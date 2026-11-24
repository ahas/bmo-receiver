//! The agency's turn routes served on a real loopback port, which a test stops and starts again in the middle of a
//! turn as a deploy does: a stop closes the listener and cuts every connection it holds, the send waiting on its
//! turn and the event stream alike, while what the agency keeps (its turns, the turn each key names, its events)
//! stands across it as its store does. It takes a turn under the caller's key and attaches a send made again under
//! the same key to the turn standing, as the agency's re-attach does.

use std::collections::{BTreeMap, HashMap};
use std::convert::Infallible;
use std::net::{Shutdown, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::sse::{Event as SseEvent, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use futures_util::StreamExt;
use futures_util::stream;
use receiver_ports::Peer;
use receiver_ports::agency::HttpAgency;
use receiver_ports::http::{Caller, Patience, RealClock};
use serde_json::{Value, json};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, watch};
use tokio::task::JoinHandle;

/// One event the agency keeps and publishes.
#[derive(Clone, Debug)]
struct Kept {
  id: i64,
  kind: String,
  data: Value,
}

#[derive(Default)]
struct Book {
  /// Every send of a turn taken, its body as it came.
  sends: Vec<Value>,
  events: Vec<Kept>,
  /// Each turn's session and its answer once it has ended.
  turns: BTreeMap<i64, (String, Option<String>)>,
  keys: HashMap<String, i64>,
  /// The turns that ended failed, each with why.
  failed: HashMap<i64, String>,
  /// Whether a send names its turn now; one taken while it does not waits, unnamed, until it is cut.
  silent: bool,
}

struct Shared {
  book: Mutex<Book>,
  live: broadcast::Sender<Kept>,
  /// Moves on every turn's end.
  ends: watch::Sender<u64>,
  /// Moves on every stop.
  stops: watch::Sender<u64>,
}

impl Shared {
  fn publish(&self, kind: &str, data: Value) {
    let kept = {
      let mut book = self.book.lock().unwrap();
      let kept = Kept {
        id: book.events.len() as i64 + 1,
        kind: kind.to_owned(),
        data,
      };

      book.events.push(kept.clone());
      kept
    };

    let _ = self.live.send(kept);
  }

  fn answer_of(&self, turn: i64) -> Option<String> {
    self.book.lock().unwrap().turns.get(&turn).and_then(|(_, answer)| answer.clone())
  }

  // Waits until the stand-in is stopped next.
  async fn stopped(&self) {
    let mut stops = self.stops.subscribe();

    let _ = stops.changed().await;
  }
}

// The listener whose every accepted connection is held, so a stop cuts them all.
struct Severable {
  listener: TcpListener,
  held: Arc<Mutex<Vec<std::net::TcpStream>>>,
}

impl axum::serve::Listener for Severable {
  type Io = TcpStream;
  type Addr = SocketAddr;

  async fn accept(&mut self) -> (Self::Io, Self::Addr) {
    loop {
      let Ok((stream, addr)) = self.listener.accept().await else {
        continue;
      };
      let Ok(socket) = stream.into_std() else { continue };
      let Ok(kept) = socket.try_clone() else { continue };
      let Ok(stream) = TcpStream::from_std(socket) else {
        continue;
      };

      self.held.lock().unwrap().push(kept);

      return (stream, addr);
    }
  }

  fn local_addr(&self) -> std::io::Result<Self::Addr> {
    self.listener.local_addr()
  }
}

struct Serving {
  server: JoinHandle<()>,
  held: Arc<Mutex<Vec<std::net::TcpStream>>>,
}

/// The agency's turn routes on a loopback port, until it is dropped.
pub struct ServedAgency {
  shared: Arc<Shared>,
  addr: SocketAddr,
  serving: Mutex<Option<Serving>>,
}

#[derive(serde::Deserialize)]
struct Page {
  #[serde(default)]
  after: Option<i64>,
  #[serde(default)]
  limit: Option<usize>,
}

async fn send(
  State(shared): State<Arc<Shared>>,
  Path(session): Path<String>,
  Json(body): Json<Value>,
) -> Response {
  let key = body.get("key").and_then(Value::as_str).map(str::to_owned);
  let named = {
    let mut book = shared.book.lock().unwrap();

    book.sends.push(body.clone());

    if book.silent {
      None
    } else if let Some(turn) = key.as_ref().and_then(|key| book.keys.get(key).copied()) {
      Some((turn, false))
    } else {
      let turn = book.turns.keys().last().copied().unwrap_or(0) + 1;

      book.turns.insert(turn, (session.clone(), None));

      if let Some(key) = key {
        book.keys.insert(key, turn);
      }

      Some((turn, true))
    }
  };

  let Some((turn, fresh)) = named else {
    shared.stopped().await;

    return StatusCode::SERVICE_UNAVAILABLE.into_response();
  };

  if fresh {
    shared.publish(
      "turn.created",
      json!({ "session": session, "turnId": turn, "run": format!("run-{turn}") }),
    );
  }

  let mut ends = shared.ends.subscribe();

  // The send answers the turn's end once it came, as the agency's does.
  tokio::select! {
    _ = ends.wait_for(|_| shared.answer_of(turn).is_some()) => {
      let answer = shared.answer_of(turn);
      let failed = shared.book.lock().unwrap().failed.get(&turn).cloned();
      let outcome = if failed.is_some() { "failed" } else { "answered" };

      Json(json!({ "turn": turn, "delivery": "delivered", "outcome": outcome, "answer": answer, "reason": failed }))
        .into_response()
    }
    () = shared.stopped() => StatusCode::SERVICE_UNAVAILABLE.into_response(),
  }
}

// The kept page where the ask takes JSON, as the agency answers it; its stream otherwise.
async fn events(
  State(shared): State<Arc<Shared>>,
  Query(page): Query<Page>,
  headers: HeaderMap,
) -> Response {
  let polling = headers
    .get(header::ACCEPT)
    .and_then(|accept| accept.to_str().ok())
    .is_some_and(|accept| accept.contains("application/json"));

  if polling {
    let limit = page.limit.unwrap_or(200).min(1000);

    let items: Vec<Value> = shared
      .book
      .lock()
      .unwrap()
      .events
      .iter()
      .filter(|kept| page.after.is_none_or(|after| kept.id > after))
      .take(limit)
      .map(|kept| json!({ "id": kept.id, "kind": kept.kind, "at": kept.id, "data": kept.data }))
      .collect();

    return Json(json!({ "items": items, "more": false })).into_response();
  }

  let live = shared.live.subscribe();
  let stopped = Box::pin({
    let shared = shared.clone();

    async move { shared.stopped().await }
  });
  let heard = stream::unfold(live, |mut live| async move {
    loop {
      match live.recv().await {
        Ok(kept) => return Some((kept, live)),
        Err(broadcast::error::RecvError::Lagged(_)) => continue,
        Err(broadcast::error::RecvError::Closed) => return None,
      }
    }
  })
  .map(|kept| {
    let envelope = json!({ "id": kept.id, "kind": kept.kind, "at": kept.id, "data": kept.data });

    Ok::<_, Infallible>(
      SseEvent::default().event(kept.kind).id(kept.id.to_string()).data(envelope.to_string()),
    )
  })
  .take_until(stopped);

  Sse::new(heard).into_response()
}

async fn read(State(shared): State<Arc<Shared>>, Path((_session, turn)): Path<(String, i64)>) -> Response {
  let standing = shared.book.lock().unwrap().turns.get(&turn).cloned();

  match standing {
    None => (
      StatusCode::NOT_FOUND,
      Json(json!({ "code": "unknown-turn", "error": "The session holds no such turn." })),
    )
      .into_response(),
    // The agency's read names whether the turn ended and carries nothing of its end.
    Some((_, answer)) => {
      let state = if answer.is_some() { "ended" } else { "running" };

      Json(json!({ "id": turn, "state": state, "outcome": null, "answer": null, "delivery": null }))
        .into_response()
    }
  }
}

impl ServedAgency {
  /// The stand-in serving on a fresh loopback port.
  pub async fn start() -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let served = ServedAgency {
      shared: Arc::new(Shared {
        book: Mutex::default(),
        live: broadcast::channel(256).0,
        ends: watch::channel(0).0,
        stops: watch::channel(0).0,
      }),
      addr,
      serving: Mutex::default(),
    };

    served.serve(listener);
    served
  }

  fn serve(&self, listener: TcpListener) {
    let router = Router::new()
      .route("/sessions/{session}/turns", post(send))
      .route("/sessions/{session}/turns/{turn}", get(read))
      .route("/events", get(events))
      .with_state(self.shared.clone());
    let held = Arc::new(Mutex::new(Vec::new()));
    let listener = Severable {
      listener,
      held: held.clone(),
    };
    let server = tokio::spawn(async move {
      let _ = axum::serve(listener, router).await;
    });

    *self.serving.lock().unwrap() = Some(Serving { server, held });
  }

  /// The receiver's port onto it, offering a refused call again every millisecond.
  pub fn port(&self) -> HttpAgency {
    let peer = Peer {
      base: format!("http://{}", self.addr),
      token: "token-a".to_owned(),
    };

    HttpAgency::with_caller(Caller::new(&peer).with_patience(Patience {
      every: Duration::from_millis(1),
      waiter: Arc::new(RealClock),
    }))
  }

  // Every connection it held cut and its listener set to close; the server's task, to be waited out.
  fn cut(&self) -> Option<JoinHandle<()>> {
    let serving = self.serving.lock().unwrap().take()?;

    serving.server.abort();

    for socket in serving.held.lock().unwrap().drain(..) {
      let _ = socket.shutdown(Shutdown::Both);
    }

    self.shared.stops.send_modify(|stops| *stops += 1);

    Some(serving.server)
  }

  /// Stopped: every connection it held cut, and the port closed once this returns.
  pub async fn stop(&self) {
    if let Some(server) = self.cut() {
      let _ = server.await;
    }
  }

  /// Started again on the same port, keeping all it kept.
  pub async fn restart(&self) {
    let listener = TcpListener::bind(self.addr).await.unwrap();

    self.serve(listener);
  }

  /// Whether a send names its turn from now on.
  pub fn naming(&self, names: bool) {
    self.shared.book.lock().unwrap().silent = !names;
  }

  /// The turn ended answered with `answer`, its end published whether or not anyone listens.
  pub fn end(&self, turn: i64, answer: &str) {
    let session = {
      let mut book = self.shared.book.lock().unwrap();
      let standing = book.turns.get_mut(&turn).expect("the turn stands");

      standing.1 = Some(answer.to_owned());
      standing.0.clone()
    };

    // The end as the agency publishes it: its turn under `turn`, and the words taken.
    self.shared.publish(
      "turn.ended",
      json!({
        "session": session,
        "turn": turn,
        "run": format!("run-{turn}"),
        "outcome": "answered",
        "answer": answer,
        "delivery": "taken",
      }),
    );
    self.shared.ends.send_modify(|ends| *ends += 1);
  }

  /// The turn ended failed for `reason`, its end published whether or not anyone listens, as a turn the agency's
  /// next server could not take up again ends.
  pub fn fail(&self, turn: i64, reason: &str) {
    let session = {
      let mut book = self.shared.book.lock().unwrap();
      let standing = book.turns.get_mut(&turn).expect("the turn stands");

      standing.1 = Some(String::new());

      let session = standing.0.clone();

      book.failed.insert(turn, reason.to_owned());
      session
    };

    self.shared.publish(
      "turn.ended",
      json!({
        "session": session,
        "turn": turn,
        "run": format!("run-{turn}"),
        "outcome": "failed",
        "reason": reason,
        "answer": null,
        "delivery": "taken",
      }),
    );
    self.shared.ends.send_modify(|ends| *ends += 1);
  }

  /// The turn ended answered with `answer` as the stream never told it: the send waiting on it is answered and
  /// the turn read says `ended`, but no end event is kept or published, as one dropped between the agency and a
  /// listening stream is.
  pub fn end_unheard(&self, turn: i64, answer: &str) {
    let mut book = self.shared.book.lock().unwrap();
    let standing = book.turns.get_mut(&turn).expect("the turn stands");

    standing.1 = Some(answer.to_owned());
    drop(book);
    self.shared.ends.send_modify(|ends| *ends += 1);
  }

  /// All it kept forgotten, as an agency whose store was lost.
  pub fn forget(&self) {
    let mut book = self.shared.book.lock().unwrap();

    book.turns.clear();
    book.keys.clear();
    book.events.clear();
  }

  /// Every send taken, its body as it came.
  pub fn sends(&self) -> Vec<Value> {
    self.shared.book.lock().unwrap().sends.clone()
  }

  /// The turns it named, in order.
  pub fn turns(&self) -> Vec<i64> {
    self.shared.book.lock().unwrap().turns.keys().copied().collect()
  }
}

impl Drop for ServedAgency {
  fn drop(&mut self) {
    let _ = self.cut();
  }
}

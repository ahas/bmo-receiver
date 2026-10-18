//! A stand-in for the agency's sessions (docs/boundary-api.md, 「The agency」): a session started for every asking,
//! its state, the turn-started and turn-ended events of every turn it carries on its event stream, and an empty
//! run log. A turn it is told to hold stands working after it started until the test lets it go, so words can
//! come into the room while it works.

use std::sync::Mutex;

use futures_util::future::BoxFuture;
use receiver_ports::agency::{Delivery, Event, Events, RunLine, Scope, Session, SessionState, Turn};
use receiver_ports::http::Answer;
use serde_json::json;
use tokio::sync::{Notify, broadcast};

/// The session every turn goes on: every start and turn it was asked, the answer each turn ends with, and the
/// turns it holds working.
pub struct StandInSession {
  pub id: String,
  pub answer: String,
  pub starts: Mutex<Vec<Scope>>,
  pub turns: Mutex<Vec<Turn>>,
  events: broadcast::Sender<Event>,
  // How many turns, from the first, stand working until let go; and the letting go.
  holding: Mutex<usize>,
  let_go: Notify,
  working: Mutex<bool>,
}

impl StandInSession {
  pub fn new(id: &str, answer: &str) -> Self {
    StandInSession {
      id: id.to_owned(),
      answer: answer.to_owned(),
      starts: Mutex::default(),
      turns: Mutex::default(),
      events: broadcast::channel(64).0,
      holding: Mutex::new(0),
      let_go: Notify::new(),
      working: Mutex::new(false),
    }
  }

  /// The first turn stands working once it started, until `let_go`.
  pub fn holding_the_first(self) -> Self {
    *self.holding.lock().unwrap() = 1;
    self
  }

  /// The turn standing working ends.
  pub fn let_go(&self) {
    self.let_go.notify_one();
  }

  /// Whether a turn has started and not yet ended.
  pub fn working(&self) -> bool {
    *self.working.lock().unwrap()
  }

  /// Every turn's prompt, in the order they were carried.
  pub fn prompts(&self) -> Vec<String> {
    self.turns.lock().unwrap().iter().map(|turn| turn.prompt.clone()).collect()
  }

  fn told(&self, kind: &str, data: serde_json::Value) {
    let _ = self.events.send(Event {
      kind: kind.to_owned(),
      data,
    });
  }

  pub fn start(&self, agent: &str, scope: Scope) -> BoxFuture<'_, Answer<Session>> {
    self.starts.lock().unwrap().push(scope);

    let session = Session {
      id: self.id.clone(),
      agent: agent.to_owned(),
      scope,
    };

    Box::pin(async move { Ok(session) })
  }

  pub fn state(&self) -> BoxFuture<'_, Answer<SessionState>> {
    let state = SessionState {
      session: self.id.clone(),
      provider: Some("provider-a".to_owned()),
      model: Some("model-a".to_owned()),
      place: json!(null),
      generation: json!(null),
      context: json!({ "carried": 1000, "window": 200000 }),
    };

    Box::pin(async move { Ok(state) })
  }

  pub fn events(&self) -> BoxFuture<'_, Answer<Events>> {
    let heard = self.events.subscribe();
    let stream = futures_util::stream::unfold(heard, |mut heard| async move {
      loop {
        match heard.recv().await {
          Ok(event) => return Some((Ok(event), heard)),
          Err(broadcast::error::RecvError::Lagged(_)) => continue,
          Err(broadcast::error::RecvError::Closed) => return None,
        }
      }
    });

    Box::pin(async move { Ok(Box::pin(stream) as Events) })
  }

  pub fn turn<'a>(&'a self, turn: &'a Turn) -> BoxFuture<'a, Answer<Delivery>> {
    Box::pin(async move {
      let number = {
        let mut turns = self.turns.lock().unwrap();

        turns.push(turn.clone());
        turns.len()
      };
      let holds = {
        let mut holding = self.holding.lock().unwrap();
        let holds = *holding > 0;

        *holding = holding.saturating_sub(1);
        holds
      };

      *self.working.lock().unwrap() = true;
      self.told("turn-started", json!({ "session": self.id, "run": format!("run-{number}") }));

      if holds {
        self.let_go.notified().await;
      }

      *self.working.lock().unwrap() = false;
      self.told(
        "turn-ended",
        json!({ "session": self.id, "outcome": "answered", "answer": self.answer }),
      );

      Ok(Delivery::Taken)
    })
  }

  pub fn run_log(&self) -> BoxFuture<'_, Answer<Vec<RunLine>>> {
    Box::pin(async { Ok(vec![]) })
  }
}

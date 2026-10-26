//! The tool host, as the receiver reaches it on its control socket (`docs/boundary-api.md`, 「The tool host」;
//! bmo-pocket's `docs/api.md`, 「The control socket」): the catalogue's changes on its event stream, resumed after
//! the last one read, and the tool sessions standing, each with its holder and its namespaces. Every call carries
//! the issuer token, read afresh from its file at each call.

use std::path::PathBuf;
use std::time::Duration;

use futures_util::future::BoxFuture;
use futures_util::stream::BoxStream;
use futures_util::{StreamExt, TryStreamExt};
use serde::Deserialize;
use sse_stream::SseStream;

use crate::http::{Answer, Caller, Peer, PortFailure};

/// The kinds of event the receiver hears of the tool host.
pub const KINDS: [&str; 2] = ["tool.placed", "tool.removed"];

/// One entry a tool serves a session: its full name, how it is reached and its description's first sentence.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
  pub tool: String,
  pub reached_by: String,
  #[serde(default)]
  pub summary: String,
}

/// The tool a change is of: its name, the namespaces its entries are shown under, the entries, and whether a
/// placing took the place of another version.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Change {
  pub tool: String,
  #[serde(default)]
  pub namespaces: Vec<String>,
  #[serde(default)]
  pub entries: Vec<Entry>,
  #[serde(default)]
  pub replaced: bool,
}

/// One event of the tool host's stream: its number, its kind, when the host read the change, and the change.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ToolEvent {
  pub id: i64,
  pub kind: String,
  #[serde(default)]
  pub at: i64,
  pub data: Change,
}

/// A tool session standing: who holds it and the namespaces it may use.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StandingSession {
  pub holder_name: String,
  #[serde(default)]
  pub namespaces: Vec<String>,
  #[serde(default)]
  pub created_at: i64,
}

#[derive(Deserialize)]
struct SessionPage {
  items: Vec<StandingSession>,
  #[serde(default)]
  more: bool,
}

/// The events as they come; a stream that drops, or stays silent past its bound, ends with the failure, and is
/// opened again by its reader.
pub type ToolEvents = BoxStream<'static, Answer<ToolEvent>>;

/// Every call the receiver makes on the tool host.
pub trait ToolHost: Send + Sync {
  /// The catalogue's changes after the event numbered `after`, or from now where none is named. A cursor the host
  /// no longer holds is answered `410 gone`.
  fn events(&self, after: Option<i64>) -> BoxFuture<'_, Answer<ToolEvents>>;

  /// Every tool session standing.
  fn sessions(&self) -> BoxFuture<'_, Answer<Vec<StandingSession>>>;
}

/// The tool host on its control socket, the issuer token read from its file.
pub struct HttpToolHost {
  socket: PathBuf,
  token_file: PathBuf,
  silent: Duration,
}

// The base a request names on the socket, whose host the socket stands for.
const BASE: &str = "http://tool-host";

impl HttpToolHost {
  /// The host on `socket`, called with the issuer token in `token_file`, its stream read as dropped once it says
  /// nothing, not even a ping, for `silent`.
  pub fn new(socket: PathBuf, token_file: PathBuf, silent: Duration) -> Self {
    Self {
      socket,
      token_file,
      silent,
    }
  }

  fn caller(&self) -> Caller {
    let token = std::fs::read_to_string(&self.token_file).unwrap_or_default().trim().to_owned();

    Caller::over_socket(
      &self.socket,
      &Peer {
        base: BASE.to_owned(),
        token,
      },
    )
  }
}

impl ToolHost for HttpToolHost {
  fn events(&self, after: Option<i64>) -> BoxFuture<'_, Answer<ToolEvents>> {
    Box::pin(async move {
      let caller = self.caller();
      let mut query = vec![("kinds", KINDS.join(","))];

      if let Some(after) = after {
        query.push(("after", after.to_string()));
      }

      let url = caller.url(&["events"])?.to_string();
      let opened = caller.open(&["events"], &query).await?;
      let silent = self.silent;
      // A frame or a ping is heard within the bound, or the stream ends as dropped.
      let bytes = opened.bytes_stream().map_err(std::io::Error::other);
      let heard = futures_util::stream::unfold(Some(bytes), move |bytes| async move {
        let mut bytes = bytes?;

        match tokio::time::timeout(silent, bytes.next()).await {
          Ok(Some(chunk)) => Some((chunk, Some(bytes))),
          Ok(None) => None,
          Err(_) => Some((
            Err(std::io::Error::new(
              std::io::ErrorKind::TimedOut,
              "the stream heard nothing, not even a ping",
            )),
            None,
          )),
        }
      });
      let events = SseStream::from_bytes_stream(Box::pin(heard)).filter_map(move |read| {
        let url = url.clone();

        async move {
          match read {
            Err(why) => Some(Err(PortFailure::Transport {
              url,
              why: why.to_string(),
            })),
            Ok(sse) => {
              let data = sse.data?;

              Some(serde_json::from_str::<ToolEvent>(&data).map_err(|why| PortFailure::Undecodable {
                url,
                why: format!("an event's data: {why}"),
              }))
            }
          }
        }
      });

      Ok(events.boxed())
    })
  }

  fn sessions(&self) -> BoxFuture<'_, Answer<Vec<StandingSession>>> {
    Box::pin(async move {
      let caller = self.caller();
      let mut sessions: Vec<StandingSession> = vec![];
      let mut after: Option<i64> = None;

      loop {
        let query: Vec<(&str, String)> =
          after.map(|after| vec![("after", after.to_string())]).unwrap_or_default();
        let page: SessionPage = caller.get(&["tool-sessions"], &query).await?;
        let last = page.items.last().map(|session| session.created_at);

        sessions.extend(page.items);

        match (page.more, last) {
          (true, Some(last)) if Some(last) != after => after = Some(last),
          _ => return Ok(sessions),
        }
      }
    })
  }
}

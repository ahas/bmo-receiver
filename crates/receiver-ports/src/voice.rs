//! The voice engine's port: a briefing taken in, the briefings a line holds looked up, and every room's briefings
//! of one kind stopped (`docs/boundary-api.md`,
//! 「The voice engine」; the voice's `docs/boundary-api.md`, 「Briefings」). Bodies are camelCase, behind the bearer
//! token the configuration holds for it.

use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};

use crate::http::{Answer, Caller, Peer};

/// A line to speak for a room, as the engine takes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewBriefing {
  pub voice_name: String,
  pub text: String,
  pub chat_id: i64,
  pub message_id: Option<i64>,
  /// `summary` or `full`.
  pub kind: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub title: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub emotion: Option<String>,
  pub plays: bool,
}

/// A briefing as the receiver reads it: its id, its kind and where it stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Briefing {
  pub id: i64,
  pub kind: String,
  /// `waiting`, `speaking`, `done`, `failed` or `stopped`.
  pub state: String,
}

#[derive(Deserialize)]
struct Page {
  items: Vec<Briefing>,
}

// Which briefings a stop ends: those of one kind, in every room, as the body names none.
#[derive(Serialize)]
struct EveryRoom<'a> {
  kind: &'a str,
}

/// Every call the receiver makes on the voice engine.
pub trait VoiceEngine: Send + Sync {
  /// A briefing taken in, answered at once.
  fn brief<'a>(&'a self, briefing: &'a NewBriefing) -> BoxFuture<'a, Answer<Briefing>>;

  /// The briefings of one kind a line holds, newest first.
  fn of_line<'a>(
    &'a self,
    chat_id: i64,
    message_id: i64,
    kind: &'a str,
  ) -> BoxFuture<'a, Answer<Vec<Briefing>>>;

  /// Every room's briefings of one kind that have not ended stopped at once, answered with the briefings it
  /// stopped.
  fn stop_kind<'a>(&'a self, kind: &'a str) -> BoxFuture<'a, Answer<Vec<Briefing>>>;
}

/// The voice engine over HTTP.
#[derive(Debug, Clone)]
pub struct HttpVoice {
  caller: Caller,
}

impl HttpVoice {
  pub fn new(peer: &Peer) -> Self {
    Self::with_caller(Caller::new(peer))
  }

  pub fn with_caller(caller: Caller) -> Self {
    Self { caller }
  }
}

impl VoiceEngine for HttpVoice {
  fn brief<'a>(&'a self, briefing: &'a NewBriefing) -> BoxFuture<'a, Answer<Briefing>> {
    Box::pin(async move { self.caller.post(&["briefings"], briefing).await })
  }

  fn of_line<'a>(
    &'a self,
    chat_id: i64,
    message_id: i64,
    kind: &'a str,
  ) -> BoxFuture<'a, Answer<Vec<Briefing>>> {
    Box::pin(async move {
      let query = [
        ("chatId", chat_id.to_string()),
        ("messageId", message_id.to_string()),
        ("kind", kind.to_owned()),
        ("limit", "10".to_owned()),
      ];
      let page: Page = self.caller.get(&["briefings"], &query).await?;

      Ok(page.items)
    })
  }

  fn stop_kind<'a>(&'a self, kind: &'a str) -> BoxFuture<'a, Answer<Vec<Briefing>>> {
    Box::pin(async move {
      let page: Page = self.caller.post(&["briefings", "stop"], &EveryRoom { kind }).await?;

      Ok(page.items)
    })
  }
}

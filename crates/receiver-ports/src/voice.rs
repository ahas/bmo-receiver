//! The voice engine's port: a briefing taken in and read, its sound fetched and its line named, the briefings a line
//! holds looked up, and the briefings of one kind stopped, in every room or in one (`docs/boundary-api.md`,
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
  /// `summary`, `full` or `speech`.
  pub kind: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub title: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub emotion: Option<String>,
  pub plays: bool,
  /// The caller's own name for the line, so a second ask of the same line answers the first briefing.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub key: Option<String>,
}

/// A briefing as the receiver reads it: its id, its kind and where it stands, and the synthesis that speaks it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Briefing {
  pub id: i64,
  pub kind: String,
  /// `waiting`, `speaking`, `done`, `failed` or `stopped`.
  pub state: String,
  #[serde(default)]
  pub synthesis_id: Option<i64>,
}

/// A briefing's line, named or let go.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Line {
  message_id: Option<i64>,
}

#[derive(Deserialize)]
struct Page {
  items: Vec<Briefing>,
}

// Which briefings a stop ends: those of one kind, in one room or in every room where it names none.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Stop<'a> {
  #[serde(skip_serializing_if = "Option::is_none")]
  chat_id: Option<i64>,
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

  /// One room's briefings of one kind that have not ended stopped at once, answered with the briefings it stopped.
  fn stop_in<'a>(&'a self, chat_id: i64, kind: &'a str) -> BoxFuture<'a, Answer<Vec<Briefing>>>;

  /// One briefing as it stands.
  fn briefing(&self, briefing_id: i64) -> BoxFuture<'_, Answer<Briefing>>;

  /// A done briefing's sound, OGG/Opus.
  fn sound(&self, briefing_id: i64) -> BoxFuture<'_, Answer<Vec<u8>>>;

  /// A briefing's line named, the message it went out as.
  fn name_line(&self, briefing_id: i64, message_id: i64) -> BoxFuture<'_, Answer<Briefing>>;
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
      let page: Page = self
        .caller
        .post(
          &["briefings", "stop"],
          &Stop {
            chat_id: None,
            kind,
          },
        )
        .await?;

      Ok(page.items)
    })
  }

  fn stop_in<'a>(&'a self, chat_id: i64, kind: &'a str) -> BoxFuture<'a, Answer<Vec<Briefing>>> {
    Box::pin(async move {
      let stop = Stop {
        chat_id: Some(chat_id),
        kind,
      };
      let page: Page = self.caller.post(&["briefings", "stop"], &stop).await?;

      Ok(page.items)
    })
  }

  fn briefing(&self, briefing_id: i64) -> BoxFuture<'_, Answer<Briefing>> {
    Box::pin(async move { self.caller.get(&["briefings", &briefing_id.to_string()], &[]).await })
  }

  fn sound(&self, briefing_id: i64) -> BoxFuture<'_, Answer<Vec<u8>>> {
    Box::pin(async move {
      let id = briefing_id.to_string();
      let url = self.caller.url(&["briefings", &id, "sound"])?.to_string();
      let answer = self.caller.open(&["briefings", &id, "sound"], &[]).await?;
      let bytes = answer.bytes().await.map_err(|why| crate::http::PortFailure::Transport {
        url,
        why: why.to_string(),
      })?;

      Ok(bytes.to_vec())
    })
  }

  fn name_line(&self, briefing_id: i64, message_id: i64) -> BoxFuture<'_, Answer<Briefing>> {
    Box::pin(async move {
      let line = Line {
        message_id: Some(message_id),
      };

      self.caller.post(&["briefings", &briefing_id.to_string(), "line"], &line).await
    })
  }
}

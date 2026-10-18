//! The voice unit's port: a voice briefing launched, a room's briefings read, and the line a briefing awaits
//! handed over (`docs/boundary-api.md`, 「The voice unit」). The voice unit states no boundary yet, so these calls
//! are the receiver's statement for it to match; bodies are snake_case.

use futures_util::future::BoxFuture;
use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::http::{Answer, Caller, Peer};

/// What a briefing speaks: a notice, or a summary of a room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BriefingKind {
  Notice,
  Summary,
}

/// A briefing asked for a room.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Briefing {
  pub chat: i64,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub message: Option<i64>,
  pub kind: BriefingKind,
  pub title: String,
  /// Whether the briefing waits for the room's line to stand under before it is sent.
  pub awaits_line: bool,
  pub text: String,
}

/// A briefing of a room as the voice unit keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Briefed {
  pub run: String,
  pub chat: i64,
  /// The line the reading was asked for, where it was asked for one.
  #[serde(default)]
  pub message: Option<i64>,
  #[serde(default)]
  pub voice_message: Option<i64>,
  pub kind: BriefingKind,
  pub title: String,
  pub finished: bool,
}

/// Every call the receiver makes on the voice unit.
pub trait Voice: Send + Sync {
  /// A briefing launched; the run it goes out as.
  fn brief<'a>(&'a self, briefing: &'a Briefing) -> BoxFuture<'a, Answer<String>>;

  /// A room's briefings, only those awaiting their line where `awaiting`.
  fn briefings(&self, chat: i64, awaiting: bool) -> BoxFuture<'_, Answer<Vec<Briefed>>>;

  /// The line a briefing awaits, by its message.
  fn line<'a>(&'a self, run: &'a str, message: i64) -> BoxFuture<'a, Answer<()>>;
}

/// The voice unit over HTTP.
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

#[derive(Deserialize)]
struct Launched {
  run: String,
}

#[derive(Deserialize)]
struct Briefings {
  briefings: Vec<Briefed>,
}

#[derive(Serialize)]
struct Line {
  message: i64,
}

impl Voice for HttpVoice {
  fn brief<'a>(&'a self, briefing: &'a Briefing) -> BoxFuture<'a, Answer<String>> {
    Box::pin(async move {
      let launched: Launched = self.caller.post(&["briefings"], briefing).await?;

      Ok(launched.run)
    })
  }

  fn briefings(&self, chat: i64, awaiting: bool) -> BoxFuture<'_, Answer<Vec<Briefed>>> {
    Box::pin(async move {
      let query = [("chat", chat.to_string()), ("awaiting", awaiting.to_string())];
      let briefings: Briefings = self.caller.get(&["briefings"], &query).await?;

      Ok(briefings.briefings)
    })
  }

  fn line<'a>(&'a self, run: &'a str, message: i64) -> BoxFuture<'a, Answer<()>> {
    Box::pin(
      async move { self.caller.done(Method::POST, &["briefings", run, "line"], &Line { message }).await },
    )
  }
}

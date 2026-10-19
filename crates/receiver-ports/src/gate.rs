//! The gate port: a turn's envelope handed over for its reference, words through the sanitiser on their way in
//! and out, a room's judgements written and a new companion's gate options copied (`docs/boundary-api.md`, 「The
//! gate」). The gate states no boundary
//! yet, so these calls are the receiver's statement for it to match; bodies are snake_case, and the gate reads
//! the rest of its material back from the brain and from the receiver's own routes.

use futures_util::future::BoxFuture;
use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::http::{Answer, Caller, Peer};

/// What kind of room a turn is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RoomKind {
  Owner,
  Group,
}

/// An event a turn carries: its kind, and the item it is about where it is about one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnEvent {
  pub kind: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub item: Option<i64>,
}

/// The situation a turn is read in: the agent whose turn it is, the room's kind, the events it carries and the
/// moment it is read at, in Unix seconds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Situation {
  pub agent: String,
  pub room: RoomKind,
  pub events: Vec<TurnEvent>,
  pub now: i64,
}

/// A turn's envelope, as the gate chooses its reference by it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Envelope {
  pub chat: i64,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub message: Option<i64>,
  /// The room's recent messages in the room shape, with every line the turn brought.
  pub lines: Vec<String>,
  /// The people whose words the turn answers.
  pub speakers: Vec<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub situation: Option<Situation>,
  /// The holds the turn stands under, where it names any.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub holds: Option<Vec<String>>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub round: Option<String>,
  /// Whether anyone but the agents spoke in the turn's words.
  pub people_spoke: bool,
  /// What the turn answered already, where it is asked again.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub answered: Option<String>,
  /// The gates switched off for the agent.
  pub off: Vec<String>,
  /// Whether the turn's road records its gates.
  pub records: bool,
}

/// Which sort of thing a piece of the reference is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
  Document,
  Person,
  Rule,
  Situation,
  Knowledge,
  Memory,
  Room,
  Archive,
}

/// One piece a turn is handed: where it was read from, its text, its sort, and the fragment row it was offered
/// from where it was offered from one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
  pub path: String,
  pub text: String,
  pub source: Source,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub fragment: Option<i64>,
}

/// Words asked of the sanitiser.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sanitise {
  pub text: String,
  /// What the words are, for the sanitiser's instructions.
  pub label: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub chat: Option<i64>,
  /// The message a verdict on the words is kept under.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub message: Option<i64>,
}

/// What the sanitiser answered: the words as they are to be handed on, and whether anything was defused. A run
/// somebody stopped is answered 409 with the code `stopped`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sanitised {
  pub text: String,
  pub changed: bool,
}

/// A decision written into a room's log: a round's skip, a re-ask, an answer sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Judgement {
  pub chat: i64,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub message: Option<i64>,
  pub stage: String,
  pub verdict: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub detail: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub ms: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub round: Option<String>,
}

/// Every call the receiver makes on the gate.
pub trait Gate: Send + Sync {
  /// The pieces the turn's envelope calls for, in the order chosen.
  fn reference<'a>(&'a self, envelope: &'a Envelope) -> BoxFuture<'a, Answer<Vec<Entry>>>;

  fn sanitise<'a>(&'a self, asked: &'a Sanitise) -> BoxFuture<'a, Answer<Sanitised>>;

  fn judge<'a>(&'a self, judgement: &'a Judgement) -> BoxFuture<'a, Answer<()>>;

  /// Every judgement of the room written at or after `since`, in Unix seconds, and naming no message yet, filed
  /// under `message`: a round's judgements under the message its answer went out as.
  fn attach<'a>(&'a self, chat: i64, message: i64, since: i64) -> BoxFuture<'a, Answer<()>>;

  /// The gate options of the agent `from` copied, as they are now, for the agent that has just stood up.
  fn copy_options<'a>(&'a self, agent: &'a str, from: &'a str) -> BoxFuture<'a, Answer<()>>;
}

/// The gate over HTTP.
#[derive(Debug, Clone)]
pub struct HttpGate {
  caller: Caller,
}

impl HttpGate {
  pub fn new(peer: &Peer) -> Self {
    Self::with_caller(Caller::new(peer))
  }

  pub fn with_caller(caller: Caller) -> Self {
    Self { caller }
  }
}

#[derive(Deserialize)]
struct Entries {
  entries: Vec<Entry>,
}

#[derive(Serialize)]
struct Attached {
  chat: i64,
  message: i64,
  since: i64,
}

#[derive(Serialize)]
struct Copied<'a> {
  agent: &'a str,
  from: &'a str,
}

impl Gate for HttpGate {
  fn reference<'a>(&'a self, envelope: &'a Envelope) -> BoxFuture<'a, Answer<Vec<Entry>>> {
    Box::pin(async move {
      let entries: Entries = self.caller.post(&["reference", "selected"], envelope).await?;

      Ok(entries.entries)
    })
  }

  fn sanitise<'a>(&'a self, asked: &'a Sanitise) -> BoxFuture<'a, Answer<Sanitised>> {
    Box::pin(self.caller.post(&["sanitised"], asked))
  }

  fn judge<'a>(&'a self, judgement: &'a Judgement) -> BoxFuture<'a, Answer<()>> {
    Box::pin(self.caller.done(Method::POST, &["judgements"], judgement))
  }

  fn attach<'a>(&'a self, chat: i64, message: i64, since: i64) -> BoxFuture<'a, Answer<()>> {
    Box::pin(async move {
      self
        .caller
        .done(
          Method::POST,
          &["judgements", "attached"],
          &Attached {
            chat,
            message,
            since,
          },
        )
        .await
    })
  }

  fn copy_options<'a>(&'a self, agent: &'a str, from: &'a str) -> BoxFuture<'a, Answer<()>> {
    Box::pin(async move {
      self.caller.done(Method::POST, &["options", "copied"], &Copied { agent, from }).await
    })
  }
}

//! The brain port: a room's rules, the reminders the send tool sets, a session's loaded documents, a chat
//! session's handoff note, a room's person book before a fold, the rows a turn wrote, and a room forgotten or
//! removed (`docs/boundary-api.md`, 「The brain」; the brain's `docs/boundary-api.md`). Every call is a camelCase
//! `POST` but the memory rows' `GET`, and a request the brain understood and will not carry out is answered with
//! its refusal.

use futures_util::future::BoxFuture;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::http::{Answer, Caller, Done, Peer, PortFailure};

/// A request the brain refused: its code, its English sentence and the code's own parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Refusal {
  pub code: String,
  pub message: String,
  #[serde(flatten)]
  pub params: Map<String, Value>,
}

/// What the brain answered a request it understood with: the call's answer, or its refusal.
pub type Refused<T> = Result<T, Refusal>;

/// A reading: the lines a session reads back.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reading {
  #[serde(default)]
  pub lines: Vec<String>,
}

/// One of a room's rules: its id, the user it is about and its words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomRule {
  pub id: String,
  pub subject: i64,
  pub text: String,
}

/// A rule's words, as it is set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleText {
  pub subject: i64,
  pub text: String,
}

/// A reminder set by an agent in a room. It is due at one moment or by a rule it repeats by; the brain refuses one
/// that names both or neither, so both are handed on as the caller gave them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reminder {
  pub agent: String,
  pub chat_id: i64,
  /// An ISO moment, or `YYYY-MM-DD HH:MM` on the brain's clock.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub at: Option<String>,
  /// `daily HH:MM`, `weekly <mon…sun> HH:MM` or `monthly <1-31> HH:MM`.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub every: Option<String>,
  pub reason: String,
  /// A memory row the reminder carries, by its id or its slug.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub memory: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub user: Option<i64>,
}

/// A room's reminders, soonest first, and the same as a session reads them back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reminders {
  pub reminders: Vec<Map<String, Value>>,
  #[serde(default)]
  pub lines: Vec<String>,
}

/// A document a session has loaded, at the version it read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Load {
  pub path: String,
  pub version: String,
}

/// What the brain did of a room's person book before a fold: `launched` or `running` with its unit, `skipped`, or
/// `refused` with why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BeforeFold {
  pub outcome: String,
  #[serde(flatten)]
  pub detail: Map<String, Value>,
}

/// A row the brain keeps, read for a turn's answer checks as the brain gives it.
pub type Row = Map<String, Value>;

/// What a room's removal takes, or would take in a preview: the rows per table, the settings, the folders, and
/// what could not be removed.
pub type Took = Map<String, Value>;

/// Every call the receiver makes on the brain.
pub trait Brain: Send + Sync {
  fn room_rules(&self, chat_id: i64) -> BoxFuture<'_, Answer<Vec<RoomRule>>>;

  /// A rule set, or revoked where `rule` is none, on the owner's message; the room's rules as they now stand.
  fn set_room_rule<'a>(
    &'a self,
    chat_id: i64,
    message_id: i64,
    id: &'a str,
    rule: Option<&'a RuleText>,
  ) -> BoxFuture<'a, Answer<Refused<Vec<RoomRule>>>>;

  fn set_reminder<'a>(&'a self, reminder: &'a Reminder) -> BoxFuture<'a, Answer<Refused<Reading>>>;

  fn cancel_reminder(&self, id: i64) -> BoxFuture<'_, Answer<Refused<Reading>>>;

  /// A room's reminders still to come, or every one where `all`; one agent's where it is named.
  fn reminders<'a>(
    &'a self,
    chat_id: i64,
    agent: Option<&'a str>,
    all: bool,
  ) -> BoxFuture<'a, Answer<Reminders>>;

  fn loads<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<Vec<Load>>>;

  fn keep_loads<'a>(&'a self, session: &'a str, loads: &'a [Load], at: i64) -> BoxFuture<'a, Answer<()>>;

  fn forget_loads<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<()>>;

  /// The handoff note an agent's chat session left, where it left one.
  fn session_summary<'a>(&'a self, agent: &'a str) -> BoxFuture<'a, Answer<Option<String>>>;

  fn clear_session_summary<'a>(&'a self, agent: &'a str) -> BoxFuture<'a, Answer<()>>;

  fn people_before_fold<'a>(&'a self, chat_id: i64, agent: &'a str) -> BoxFuture<'a, Answer<BeforeFold>>;

  /// The memory rows, an agent's or a room's where either is named, newest first.
  fn memories<'a>(
    &'a self,
    agent: Option<&'a str>,
    chat_id: Option<i64>,
  ) -> BoxFuture<'a, Answer<Vec<Row>>>;

  /// The notes still waiting for the digest, an agent's, a room's where one is named.
  fn notes<'a>(&'a self, agent: &'a str, chat_id: Option<i64>) -> BoxFuture<'a, Answer<Vec<Row>>>;

  /// The watches a room set, or every room's, an agent's where one is named; every one where `all`.
  fn watches<'a>(
    &'a self,
    by: Option<i64>,
    agent: Option<&'a str>,
    all: bool,
  ) -> BoxFuture<'a, Answer<Vec<Row>>>;

  fn room_forgotten(&self, chat_id: i64) -> BoxFuture<'_, Answer<()>>;

  /// A room's rows, settings and folders removed, or counted where `preview`.
  fn room_removed(&self, chat_id: i64, preview: bool) -> BoxFuture<'_, Answer<Took>>;
}

/// The brain over HTTP.
#[derive(Debug, Clone)]
pub struct HttpBrain {
  caller: Caller,
}

impl HttpBrain {
  pub fn new(peer: &Peer) -> Self {
    Self::with_caller(Caller::new(peer))
  }

  pub fn with_caller(caller: Caller) -> Self {
    Self { caller }
  }

  // A call whose answer is the call's own where the brain carried it out, and its refusal where it did not.
  async fn answered<T: DeserializeOwned>(
    &self,
    segments: &[&str],
    body: &impl Serialize,
  ) -> Answer<Refused<T>> {
    let answer: Value = self.caller.post(segments, body).await?;
    let undecodable = |why: String| PortFailure::Undecodable {
      url: segments.join("/"),
      why,
    };

    if answer.get("answered").and_then(Value::as_bool) == Some(false) {
      let refusal = answer.get("refusal").cloned().unwrap_or(Value::Null);

      return serde_json::from_value(refusal)
        .map(Err)
        .map_err(|why| undecodable(format!("its refusal: {why}")));
    }

    serde_json::from_value(answer).map(Ok).map_err(|why| undecodable(why.to_string()))
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Room {
  chat_id: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SetRule<'a> {
  chat_id: i64,
  message_id: i64,
  id: &'a str,
  rule: Option<&'a RuleText>,
}

#[derive(Deserialize)]
struct Rules {
  #[serde(default)]
  rules: Vec<RoomRule>,
}

#[derive(Serialize)]
struct Id {
  id: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RoomOf<'a> {
  chat_id: i64,
  agent: Option<&'a str>,
  all: bool,
}

#[derive(Serialize)]
struct Session<'a> {
  session: &'a str,
}

#[derive(Serialize)]
struct Keep<'a> {
  session: &'a str,
  loads: &'a [Load],
  at: i64,
}

#[derive(Deserialize)]
struct Loads {
  loads: Vec<Load>,
}

#[derive(Serialize)]
struct Agent<'a> {
  agent: &'a str,
}

#[derive(Deserialize)]
struct Note {
  note: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Fold<'a> {
  chat_id: i64,
  agent: &'a str,
}

#[derive(Deserialize)]
struct Memories {
  memories: Vec<Row>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NotesOf<'a> {
  agent: &'a str,
  chat_id: Option<i64>,
}

#[derive(Deserialize)]
struct Notes {
  notes: Vec<Row>,
}

#[derive(Serialize)]
struct WatchesOf<'a> {
  by: Option<i64>,
  agent: Option<&'a str>,
  all: bool,
}

#[derive(Deserialize)]
struct Watches {
  watches: Vec<Row>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Removed {
  chat_id: i64,
  preview: bool,
}

#[derive(Deserialize)]
struct Taken {
  took: Took,
}

impl Brain for HttpBrain {
  fn room_rules(&self, chat_id: i64) -> BoxFuture<'_, Answer<Vec<RoomRule>>> {
    Box::pin(async move {
      let rules: Rules = self.caller.post(&["room-rules", "read"], &Room { chat_id }).await?;

      Ok(rules.rules)
    })
  }

  fn set_room_rule<'a>(
    &'a self,
    chat_id: i64,
    message_id: i64,
    id: &'a str,
    rule: Option<&'a RuleText>,
  ) -> BoxFuture<'a, Answer<Refused<Vec<RoomRule>>>> {
    Box::pin(async move {
      let set = SetRule {
        chat_id,
        message_id,
        id,
        rule,
      };
      let answered: Refused<Rules> = self.answered(&["room-rules", "set"], &set).await?;

      Ok(answered.map(|rules| rules.rules))
    })
  }

  fn set_reminder<'a>(&'a self, reminder: &'a Reminder) -> BoxFuture<'a, Answer<Refused<Reading>>> {
    Box::pin(self.answered(&["reminders", "set"], reminder))
  }

  fn cancel_reminder(&self, id: i64) -> BoxFuture<'_, Answer<Refused<Reading>>> {
    Box::pin(async move { self.answered(&["reminders", "cancel"], &Id { id }).await })
  }

  fn reminders<'a>(
    &'a self,
    chat_id: i64,
    agent: Option<&'a str>,
    all: bool,
  ) -> BoxFuture<'a, Answer<Reminders>> {
    Box::pin(async move {
      self
        .caller
        .post(
          &["reminders", "list"],
          &RoomOf {
            chat_id,
            agent,
            all,
          },
        )
        .await
    })
  }

  fn loads<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<Vec<Load>>> {
    Box::pin(async move {
      let loads: Loads = self.caller.post(&["sessions", "loads"], &Session { session }).await?;

      Ok(loads.loads)
    })
  }

  fn keep_loads<'a>(&'a self, session: &'a str, loads: &'a [Load], at: i64) -> BoxFuture<'a, Answer<()>> {
    Box::pin(async move {
      let _: Done = self.caller.post(&["sessions", "loads", "keep"], &Keep { session, loads, at }).await?;

      Ok(())
    })
  }

  fn forget_loads<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<()>> {
    Box::pin(async move {
      let _: Done = self.caller.post(&["sessions", "loads", "forget"], &Session { session }).await?;

      Ok(())
    })
  }

  fn session_summary<'a>(&'a self, agent: &'a str) -> BoxFuture<'a, Answer<Option<String>>> {
    Box::pin(async move {
      let note: Note = self.caller.post(&["session-summary", "read"], &Agent { agent }).await?;

      Ok(note.note)
    })
  }

  fn clear_session_summary<'a>(&'a self, agent: &'a str) -> BoxFuture<'a, Answer<()>> {
    Box::pin(async move {
      let _: Done = self.caller.post(&["session-summary", "clear"], &Agent { agent }).await?;

      Ok(())
    })
  }

  fn people_before_fold<'a>(&'a self, chat_id: i64, agent: &'a str) -> BoxFuture<'a, Answer<BeforeFold>> {
    Box::pin(async move { self.caller.post(&["people", "before-fold"], &Fold { chat_id, agent }).await })
  }

  fn memories<'a>(
    &'a self,
    agent: Option<&'a str>,
    chat_id: Option<i64>,
  ) -> BoxFuture<'a, Answer<Vec<Row>>> {
    Box::pin(async move {
      let mut query = vec![];

      if let Some(agent) = agent {
        query.push(("agent", agent.to_owned()));
      }

      if let Some(chat_id) = chat_id {
        query.push(("chat", chat_id.to_string()));
      }

      let memories: Memories = self.caller.get(&["memories"], &query).await?;

      Ok(memories.memories)
    })
  }

  fn notes<'a>(&'a self, agent: &'a str, chat_id: Option<i64>) -> BoxFuture<'a, Answer<Vec<Row>>> {
    Box::pin(async move {
      let notes: Notes = self.caller.post(&["notes", "list"], &NotesOf { agent, chat_id }).await?;

      Ok(notes.notes)
    })
  }

  fn watches<'a>(
    &'a self,
    by: Option<i64>,
    agent: Option<&'a str>,
    all: bool,
  ) -> BoxFuture<'a, Answer<Vec<Row>>> {
    Box::pin(async move {
      let watches: Watches = self.caller.post(&["watches", "list"], &WatchesOf { by, agent, all }).await?;

      Ok(watches.watches)
    })
  }

  fn room_forgotten(&self, chat_id: i64) -> BoxFuture<'_, Answer<()>> {
    Box::pin(async move {
      let _: Done = self.caller.post(&["rooms", "forgotten"], &Room { chat_id }).await?;

      Ok(())
    })
  }

  fn room_removed(&self, chat_id: i64, preview: bool) -> BoxFuture<'_, Answer<Took>> {
    Box::pin(async move {
      let taken: Taken = self.caller.post(&["rooms", "removed"], &Removed { chat_id, preview }).await?;

      Ok(taken.took)
    })
  }
}

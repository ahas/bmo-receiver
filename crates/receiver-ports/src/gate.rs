//! The gate port: a turn's envelope handed over for its reference, the brain's index read, a room's judgements
//! written and an added agent's gate options copied (`docs/boundary-api.md`, 「The gate」). The bodies are the gate's
//! published shapes (`docs/api.md`): camelCase keys, epoch milliseconds, and the gate's own names — `roomId`,
//! `messageId`, `speakerIds`, `peopleSpoke`, `gateName`, `agentName` and `fromAgentName`, with the situation road
//! carrying `agentName`, `roomKind`, its events and `nowAt`.

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
#[serde(rename_all = "camelCase")]
pub struct TurnEvent {
  pub kind: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub item_id: Option<i64>,
}

/// The situation a turn is read in: the agent whose turn it is, the room's kind, the events it carries and the
/// moment it is read at, in epoch milliseconds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Situation {
  pub agent_name: String,
  pub room_kind: RoomKind,
  pub events: Vec<TurnEvent>,
  pub now_at: i64,
}

/// A turn's envelope, as the gate's reference road reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
  pub room_id: i64,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub message_id: Option<i64>,
  /// The room's recent messages in the room shape, with every line the turn brought.
  pub lines: Vec<String>,
  /// The people whose words the turn answers.
  pub speaker_ids: Vec<i64>,
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
#[serde(rename_all = "camelCase")]
pub struct Entry {
  pub path: String,
  pub text: String,
  pub source: Source,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub fragment: Option<i64>,
}

/// A decision written into a room's log: a round's skip, a re-ask, an answer sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Judgement {
  pub room_id: i64,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub message_id: Option<i64>,
  pub gate_name: String,
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

  fn judge<'a>(&'a self, judgement: &'a Judgement) -> BoxFuture<'a, Answer<()>>;

  /// Every judgement of the room written at or after `since`, in epoch milliseconds, and naming no message yet,
  /// filed under `message_id`: a round's judgements under the message its answer went out as.
  fn attach<'a>(&'a self, room_id: i64, message_id: i64, since: i64) -> BoxFuture<'a, Answer<()>>;

  /// The gate options of `from_agent_name` copied, as they are now, for the agent that has just stood up.
  fn copy_options<'a>(&'a self, agent_name: &'a str, from_agent_name: &'a str)
  -> BoxFuture<'a, Answer<()>>;

  /// The brain's index for an agent in a room, its unread counted against the session's loads; none where the gate
  /// serves no index.
  fn index<'a>(
    &'a self,
    _room: i64,
    _session: Option<&'a str>,
    _agent: Option<&'a str>,
  ) -> BoxFuture<'a, Answer<Option<Vec<IndexGroup>>>> {
    Box::pin(async { Ok(None) })
  }
}

/// One group of the brain's index as the gate answers it: its name, how many of its entries the session has not
/// read, and what changed lately, where anything did.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct IndexGroup {
  pub group: String,
  pub unread: u64,
  #[serde(default)]
  pub hint: Option<String>,
}

#[derive(Deserialize)]
struct Index {
  groups: Vec<IndexGroup>,
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
#[serde(rename_all = "camelCase")]
struct Attached {
  room_id: i64,
  message_id: i64,
  since: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Copied<'a> {
  agent_name: &'a str,
  from_agent_name: &'a str,
}

impl Gate for HttpGate {
  fn reference<'a>(&'a self, envelope: &'a Envelope) -> BoxFuture<'a, Answer<Vec<Entry>>> {
    Box::pin(async move {
      let entries: Entries = self.caller.post(&["reference", "selected"], envelope).await?;

      Ok(entries.entries)
    })
  }

  fn index<'a>(
    &'a self,
    room: i64,
    session: Option<&'a str>,
    agent: Option<&'a str>,
  ) -> BoxFuture<'a, Answer<Option<Vec<IndexGroup>>>> {
    Box::pin(async move {
      let mut query = vec![("room", room.to_string())];

      query.extend(session.map(|session| ("session", session.to_owned())));
      query.extend(agent.map(|agent| ("agent", agent.to_owned())));

      let index: Index = self.caller.get(&["reference", "index"], &query).await?;

      Ok(Some(index.groups))
    })
  }

  fn judge<'a>(&'a self, judgement: &'a Judgement) -> BoxFuture<'a, Answer<()>> {
    Box::pin(self.caller.done(Method::POST, &["judgements"], judgement))
  }

  fn attach<'a>(&'a self, room_id: i64, message_id: i64, since: i64) -> BoxFuture<'a, Answer<()>> {
    Box::pin(async move {
      self
        .caller
        .done(
          Method::POST,
          &["judgements", "attached"],
          &Attached {
            room_id,
            message_id,
            since,
          },
        )
        .await
    })
  }

  fn copy_options<'a>(
    &'a self,
    agent_name: &'a str,
    from_agent_name: &'a str,
  ) -> BoxFuture<'a, Answer<()>> {
    Box::pin(async move {
      self
        .caller
        .done(
          Method::POST,
          &["options", "copied"],
          &Copied {
            agent_name,
            from_agent_name,
          },
        )
        .await
    })
  }
}

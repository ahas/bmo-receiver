//! What a road declares of itself, as a turn is told of it and an answer goes out on it (docs/receiver.md, 「The one
//! hard rule」): a chat service's own declaration (`GET /declaration`), read alike for every service, and the room's
//! record, declared by the receiver's own policy.

use bmo_chat_client::{ChatCapability, ChatDeclaration};
use receiver_policy::roads::ROADS;

/// One road's declaration, as the receiver reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Facts {
  /// The road's key: written onto every message it takes in, never compared to a literal.
  pub key: String,
  /// What the road is called where a person is told of it.
  pub label: String,
  /// The chat verbs it performs, by the protocol's names for them.
  pub can: Vec<String>,
  /// The read modes whose messages an answer cannot be pinned to.
  pub unpinned: Vec<String>,
  /// What a writer in its rooms is told its marks become.
  pub writing: String,
  /// What its screen shows beyond its own sends, where it says.
  pub shows: Option<String>,
  /// Whether a send in an agent's own name goes out under that agent.
  pub speakers: bool,
}

impl Facts {
  /// Whether the road performs a verb.
  pub fn can(&self, verb: &str) -> bool {
    self.can.iter().any(|one| one == verb)
  }

  /// The room's own record: what it keeps, its markdown standing as written, and who reads what it keeps. It files
  /// a send in an agent's own name under that agent.
  pub fn record() -> Self {
    let record = &ROADS.record;

    Self {
      key: record.key.clone(),
      label: record.label.clone(),
      can: record.can.clone(),
      unpinned: vec![],
      writing: record.dialect.writing.clone(),
      shows: Some(record.shows.clone()),
      speakers: true,
    }
  }

  /// A chat service's road, as its declaration says it.
  pub fn of(declaration: &ChatDeclaration) -> Self {
    let named = |value: serde_json::Value| value.as_str().map(str::to_owned);

    Self {
      key: declaration.key.clone(),
      label: declaration.label.clone(),
      can: declaration
        .can
        .iter()
        .filter_map(|capability: &ChatCapability| serde_json::to_value(capability).ok().and_then(named))
        .collect(),
      unpinned: declaration
        .modes
        .iter()
        .filter(|mode| !mode.can_reply_to)
        .filter_map(|mode| serde_json::to_value(mode.voice).ok().and_then(named))
        .collect(),
      writing: declaration.dialect.writing.clone(),
      shows: declaration.shows.clone(),
      speakers: declaration.speakers,
    }
  }
}

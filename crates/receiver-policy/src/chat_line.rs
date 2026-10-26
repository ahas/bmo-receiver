//! The room's block a chat turn carries ahead of its prompt (docs/unified-bmo.md, 「The system prompt and the room
//! switch」): its fixed lines, what the turn is told of what its road can send, and the notices of a downgrade and
//! a fallback.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Persona {
  pub tone_header: String,
  pub room_header: String,
}

/// The line for each kind of agent the roster of the owner's private chat names.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RosterKinds {
  pub companion: String,
  pub gate: String,
  pub runner: String,
  pub recollector: String,
  pub worker: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Room {
  pub head: String,
  pub name: String,
  pub replaces: String,
  /// The rule of answering each person in the language they write.
  pub languages: String,
  pub group_document: String,
  pub owner_document: String,
  pub writing: String,
  pub shows: String,
  pub road: String,
  pub end: String,
  pub mark: String,
  pub roster: String,
  pub roster_owner_room: String,
  pub kinds: RosterKinds,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
  pub document: String,
  pub withheld: String,
  pub handed: String,
  pub sending: String,
  pub sending_none: String,
  pub sending_handed: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
  pub room_rules: String,
  pub grants_down: String,
  pub grants_head: String,
  pub grant: String,
  pub members: String,
  pub user: String,
  pub no_senders: String,
  pub until: String,
  pub words: String,
  pub nothing: String,
  pub grants_end: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Downgrade {
  pub fact: String,
  pub intent: String,
  pub follow_up: String,
  pub owner_room: String,
  pub room: String,
  pub ttl_minutes: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fallback {
  pub taken: String,
  pub moved: String,
  pub intent: String,
  pub ttl_minutes: i64,
  pub lifted: String,
  pub lifted_intent: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatLine {
  pub persona: Persona,
  pub room: Room,
  pub capabilities: Capabilities,
  pub downgrade: Downgrade,
  pub fallback: Fallback,
  pub rules: Rules,
}

crate::part!(pub CHAT_LINE: ChatLine = "chat-line.toml");

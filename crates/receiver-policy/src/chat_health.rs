//! What a room's session is told of its health (docs/voiced-notices.md): the notices a room or the owner hears
//! in the agent's own voice, what a failure order names a broken chat turn as, the fold and the touch.

use serde::Deserialize;

/// A notice's fact, what it is for, its kind and how long it stays worth saying.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Notice {
  pub fact: String,
  pub intent: String,
  #[serde(default)]
  pub follow_up: Option<String>,
  #[serde(default)]
  pub run_log: Option<String>,
  pub kind: String,
  pub ttl_minutes: i64,
}

/// What a failure order names a broken chat turn as.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Order {
  pub what: String,
  pub check_seconds: u64,
}

/// A full session folded: the session's line before it, and the notice after it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fold {
  pub timeout_minutes: u64,
  pub prompt: String,
  pub fact: String,
  pub intent: String,
  pub kind: String,
  pub ttl_minutes: i64,
}

/// The cache warmer's touch of a quiet room's session.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Touch {
  pub note: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatHealth {
  pub outage_lifted: Notice,
  pub rest_lifted: Notice,
  pub stopped: Notice,
  pub failed: Notice,
  pub order: Order,
  pub fold: Fold,
  pub touch: Touch,
}

crate::part!(pub CHAT_HEALTH: ChatHealth = "chat-health.toml");

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
  pub prompt: String,
  pub fact: String,
  pub intent: String,
  pub kind: String,
}

/// The cache warmer's touch of a quiet room's session: its note, and the chat touch target as the receiver
/// declares it to the warmer (docs/boundary-api.md, 「The cache warmer」).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Touch {
  pub note: String,
  pub label: String,
  pub rank: i64,
  pub idle_ms: i64,
  pub lifetime_ms: i64,
  pub margin_ms: i64,
  pub ping_bound_ms: i64,
  pub warm_by_default: bool,
  pub failure_touches: bool,
  pub honours_idle_override: bool,
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

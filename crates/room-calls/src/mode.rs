//! When a call's result comes back (docs/chat-mcp.md, 「The two modes」), whichever folder its tool stands in.

use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The longest a launching tool's call asked in sync waits for its run's result before it goes on as an async
/// call does: past it, the result reaches the next turn as a quiet instruction.
pub const SYNC_WAIT: Duration = Duration::from_secs(120);

/// When a call's result comes back: in the call's own answer, for this turn to go on with, or to the next turn
/// as a quiet instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
#[schemars(inline)]
pub enum Mode {
  Sync,
  Async,
}

impl Mode {
  /// The mode as a command's argument names it.
  pub fn word(self) -> &'static str {
    match self {
      Self::Sync => "sync",
      Self::Async => "async",
    }
  }
}

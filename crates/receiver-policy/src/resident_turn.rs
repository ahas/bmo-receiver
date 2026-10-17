//! One turn put into an agent's standing program, as the receiver composes it: the lead a round goes in behind,
//! and how full a session may be before it is folded.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResidentTurn {
  /// What the typed lead says about a room's round pasted after it.
  pub round_lead: String,
  /// How full a session's context may be before it is folded once its turn is over.
  pub fold_ratio: f64,
}

crate::part!(pub RESIDENT_TURN: ResidentTurn = "resident-turn.toml");

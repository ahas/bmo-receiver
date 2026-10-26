//! The health of a room's session as the owner is told it (docs/voiced-notices.md, 「What each path says」).

use serde::{Deserialize, Serialize};
use strum::{IntoStaticStr, VariantNames};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, IntoStaticStr, VariantNames)]
#[serde(tag = "code", rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum HealthFact {
  /// A room's fold did not happen; the room itself heard the fixed line, and the owner hears why.
  FoldFailed { room: String, reason: String },
  /// An agent's session was replaced and neither its handoff nor a transcript of it reached the new one; the
  /// owner hears it once, `reason` the replacement's.
  HandoffLost { agent: String, reason: String },
}

coded!(HealthFact);

impl std::fmt::Display for HealthFact {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::FoldFailed { room, reason } => {
        write!(f, "The fold of the session's context for the room {room} failed: {reason}")
      }
      Self::HandoffLost { agent, reason } => write!(
        f,
        "The session of {agent} was replaced ({reason}), and neither its handoff nor a transcript of it could be \
         carried to the new session, which starts remembering none of the rooms' recent conversation"
      ),
    }
  }
}

//! The few lines a room hears that no session says (docs/voiced-notices.md, 「Lines that stay fixed」): each a code
//! with its fields, rendered in the room's language by the web unit's language packs. Where the web unit cannot
//! be reached, the line goes as its code and fields, so a room is never silent about a rest or a stop.

use serde::{Deserialize, Serialize};
use strum::{IntoStaticStr, VariantNames};

use crate::draws::DrawLine;
use crate::kinds::EngineError;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, IntoStaticStr, VariantNames)]
#[serde(tag = "code", rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum FixedLine {
  /// A usage limit begins: no model can answer while it holds. `until` is epoch seconds where it is known.
  RestBegins { until: Option<i64> },
  /// The provider stopped answering.
  ProviderDown,
  /// A room's session is being folded, where the session could not say so itself.
  Compacting,
  /// A room's fold did not happen.
  CompactFailed,
  /// A run broke and no session can answer the order about it: the last resort, out of what the run left.
  RunBroken {
    agent: String,
    cause: Option<String>,
    record: Option<String>,
  },
  /// The owner shut the room: the receiver no longer listens to it nor keeps its words.
  ChatDenied,
  /// The service stops for a deployment carrying these items, none where the deployment named none: a system
  /// notice.
  GoingDownForUpdate { items: Vec<i64> },
  /// The service restarts with no deployment behind it: a system notice.
  GoingDown,
  /// A companion's engine ended its turn with an error: the one line the turn leaves in the room, under the
  /// companion's own name.
  CompanionEngineError { kind: EngineError },
  /// A companion's turn its guard cut short: the one line the room hears under the companion's own number, with
  /// the guard's reason in English.
  CompanionCut { agent: String, reason: String },
  /// A fair draw's room line.
  Draw(DrawLine),
}

coded!(FixedLine);

impl FixedLine {
  /// Whether the line is a system notice, said as the service rather than as an agent, which the room's record
  /// files as the service's own line with the system mark.
  pub fn is_system_notice(&self) -> bool {
    matches!(self, Self::GoingDownForUpdate { .. } | Self::GoingDown)
  }

  /// The line as it goes where nothing renders it: its code and fields as one JSON object.
  pub fn unrendered(&self) -> String {
    serde_json::to_string(self).unwrap_or_else(|_| <&'static str>::from(self).to_owned())
  }
}

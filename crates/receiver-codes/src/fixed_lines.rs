//! The few lines a room hears that no session says (docs/voiced-notices.md, 「Lines that stay fixed」): each a code
//! with its fields, rendered in the room's language by the web unit's language packs, or by the receiver's policy
//! sentence where they cannot. A person never reads the code or its fields.

use serde::{Deserialize, Serialize};
use strum::{IntoStaticStr, VariantNames};

use crate::draws::DrawLine;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, IntoStaticStr, VariantNames)]
#[serde(tag = "code", rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum FixedLine {
  /// A usage limit begins: no model can answer while it holds. `until` is epoch seconds where it is known.
  RestBegins { until: Option<i64> },
  /// The provider stopped answering.
  ProviderDown,
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
}

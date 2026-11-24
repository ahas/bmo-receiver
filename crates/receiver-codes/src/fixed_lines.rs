//! The few lines a room hears that no session says (docs/voiced-notices.md, 「Lines that stay fixed」): each a code
//! with its fields, rendered in the room's language by the web unit's language packs, or by the receiver's policy
//! sentence where they cannot. A person never reads the code or its fields.

use serde::{Deserialize, Serialize};
use strum::{IntoStaticStr, VariantNames};

use crate::draws::DrawLine;
use crate::kinds::HandoverCause;

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
  /// The agent's one session is being replaced and its memory tidied, so a room's words wait for the fresh
  /// session: said once to a room whose words arrive while it goes on.
  MemoryTidying,
  /// A fair draw's room line.
  Draw(DrawLine),
  /// An account let into a room in the console by another, both by their keys.
  AccountLetIn { account: i64, by: i64 },
  /// An account that went out of a room in the console by itself.
  AccountLeft { account: i64 },
  /// An account taken out of a room in the console by another, both by their keys.
  AccountTakenOut { account: i64, by: i64 },
  /// Several accounts let into a room in the console by another in one call, all by their keys: those it let in.
  AccountsLetIn { accounts: Vec<i64>, by: i64 },
  /// The members of an organisation unit, as they stood at that moment, let into a room in the console by another:
  /// the unit by its number and its name then, and those it let in and who did by their keys.
  UnitLetIn {
    unit: i64,
    #[serde(rename = "unitName")]
    unit_name: String,
    accounts: Vec<i64>,
    by: i64,
  },
  /// An agent's session handed over to a fresh one, or started afresh: kept in the room's record alone, never sent
  /// to a chat service. `agent` is the agent's registered name; `reason` the asker's words where an asker gave any;
  /// `by` the account key of the person who asked, where the receiver knows one.
  SessionHandedOver {
    agent: String,
    cause: HandoverCause,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    by: Option<i64>,
  },
  /// A person's run found no supply source left, said straight to the room the run answers so the person hears why
  /// while no turn can go out there: `why` names the reason as the agency gives it, a kebab-case word the web unit's
  /// packs carry a sentence for, and `until` the moment the source reopens in epoch seconds, where one is named.
  SupplyClosed { why: String, until: Option<i64> },
  /// An agent's brain slept the night through: its one pass over its brain outside every room, told in the agent's
  /// owner's private room and kept in its record alone, never sent to a chat service. `agent` is the agent's
  /// registered name, `date` the day it slept for (`YYYY-MM-DD`), and the counts what the sleep learned, merged and
  /// closed.
  Slept {
    agent: String,
    date: String,
    learned: i64,
    merged: i64,
    closed: i64,
  },
}

coded!(FixedLine);

impl FixedLine {
  /// Whether the line is a system notice, said as the service rather than as an agent; the room's record keeps it
  /// as the fixed line it is said from, as every fixed line the receiver says itself.
  pub fn is_system_notice(&self) -> bool {
    matches!(self, Self::GoingDownForUpdate { .. } | Self::GoingDown)
  }

  /// Whether another unit may ask the receiver to say the line into a room, beside an instruction it queues there
  /// (docs/api.md, 「Instructions」): the lines a unit composes and no session can say, and none of the receiver's
  /// own.
  pub fn asked_by_unit(&self) -> bool {
    matches!(self, Self::SupplyClosed { .. })
  }

  /// Whether another unit may ask the receiver to keep the line in a room's record alone, sent to no chat service
  /// (docs/api.md, 「Kept lines」): the record-only lines a unit composes.
  pub fn kept_by_unit(&self) -> bool {
    matches!(self, Self::Slept { .. })
  }
}

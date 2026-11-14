//! A room's members, asked of its own chat service (docs/api.md, 「Rooms」).

use http::StatusCode;
use strum::{IntoStaticStr, VariantNames};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, IntoStaticStr, VariantNames)]
#[strum(serialize_all = "kebab-case")]
pub enum RoomMembersRefusal {
  /// The room stands on the record alone, so no chat service lists its members.
  #[error("room {room} stands on the record alone, so no chat service lists its members")]
  RecordOnlyRoom { room: i64 },
  /// The room's chat service does not declare that it lists the members of a room.
  #[error("{label} does not list the members of a room")]
  MembersUnlisted { label: String },
  /// The room's chat service holds no members of the room, or refused them, for its own reason.
  #[error("{label} gave no members of the room: {detail}")]
  MembersUnheld { label: String, detail: String },
}

impl RoomMembersRefusal {
  fn answers(&self) -> StatusCode {
    match self {
      Self::RecordOnlyRoom { .. } | Self::MembersUnlisted { .. } | Self::MembersUnheld { .. } => {
        StatusCode::CONFLICT
      }
    }
  }
}

coded!(RoomMembersRefusal);
refused!(RoomMembersRefusal);

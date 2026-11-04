//! A supervisor's read of a group room's messages (docs/api.md, 「Messages」).

use http::StatusCode;
use strum::{IntoStaticStr, VariantNames};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, IntoStaticStr, VariantNames)]
#[strum(serialize_all = "kebab-case")]
pub enum RoomReadRefusal {
  /// The room is no group room, so only its own answerers read it and no other supervisor does.
  #[error("room {room} is no group room, so only its own answerers read it")]
  NotAGroup { room: i64 },
  /// The supervisor's account is no member of the room, so it reads nothing there.
  #[error("{agent} is no member's supervisor in room {room}, so it reads nothing there")]
  NotAMember { agent: String, room: i64 },
}

impl RoomReadRefusal {
  fn answers(&self) -> StatusCode {
    match self {
      Self::NotAGroup { .. } | Self::NotAMember { .. } => StatusCode::FORBIDDEN,
    }
  }
}

coded!(RoomReadRefusal);
refused!(RoomReadRefusal);

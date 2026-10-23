//! A room added on a chat service in a mode (docs/receiver.md, 「Adding a room」).

use http::StatusCode;
use strum::{IntoStaticStr, VariantNames};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, IntoStaticStr, VariantNames)]
#[strum(serialize_all = "kebab-case")]
pub enum RoomAdditionRefusal {
  /// The provider does not declare that it lists the rooms of its account.
  #[error("{label} does not list the rooms of its account")]
  RoomsUnlisted { label: String },
  /// The provider does not say the room is its own.
  #[error("{label} does not say room {room} is its own")]
  RoomUnknown { label: String, room: i64 },
  /// The room is to be answered as the user where the provider's `human-avatar` mode does not stand active.
  #[error("{label} does not answer in the account of the person it reads")]
  AvatarUnsupported { label: String },
  /// The room was already added.
  #[error("room {room} was added already")]
  RoomAdded { room: i64 },
}

impl RoomAdditionRefusal {
  fn answers(&self) -> StatusCode {
    match self {
      Self::RoomsUnlisted { .. }
      | Self::RoomUnknown { .. }
      | Self::AvatarUnsupported { .. }
      | Self::RoomAdded { .. } => StatusCode::CONFLICT,
    }
  }
}

coded!(RoomAdditionRefusal);
refused!(RoomAdditionRefusal);

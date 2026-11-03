//! A room's mode read and changed on its registration (docs/receiver.md, 「Reading and changing a room's mode」).

use http::StatusCode;
use strum::{IntoStaticStr, VariantNames};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, IntoStaticStr, VariantNames)]
#[strum(serialize_all = "kebab-case")]
pub enum RoomModeRefusal {
  /// The room was never added, so it has no registration and no mode to change.
  #[error("room {room} was never added, so it has no mode to change")]
  RoomNotAdded { room: i64 },
  /// The room's list names no chat service, so no account stands there to answer as the person.
  #[error("the room is on no chat service, so nothing there would answer as the person")]
  NoChatService,
  /// A chat service on the room's list does not declare its `human-avatar` mode active.
  #[error("{label} does not offer the avatar mode")]
  AvatarUnoffered { label: String },
  /// A chat service's answer for the room states modes without `human-avatar`, so the room is never answered in.
  #[error("{label} does not answer room {room} in the account of the person it reads")]
  RoomTakesNoAvatar { label: String, room: i64 },
}

impl RoomModeRefusal {
  fn answers(&self) -> StatusCode {
    match self {
      Self::RoomNotAdded { .. }
      | Self::NoChatService
      | Self::AvatarUnoffered { .. }
      | Self::RoomTakesNoAvatar { .. } => StatusCode::CONFLICT,
    }
  }
}

coded!(RoomModeRefusal);
refused!(RoomModeRefusal);

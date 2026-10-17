//! A room's registration removed (docs/receiver.md, 「Removing a room's registration」): the rooms that are refused,
//! and what the confirmation warns of beside what the holders answer.

use http::StatusCode;
use serde::{Deserialize, Serialize};
use strum::{IntoStaticStr, VariantNames};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, IntoStaticStr, VariantNames)]
#[strum(serialize_all = "kebab-case")]
pub enum RoomRemovalRefusal {
  #[error("the room is declared by the companion seeds, so the next start would make it again")]
  SeededRoom,
  #[error("the room is the infrastructure room the configuration names")]
  Infrastructure,
  #[error(
    "the room is the administrator's own private chat, where reports and questions reach the administrator"
  )]
  OwnersChat,
  #[error("only the administrator removes a room's registration")]
  NotAdministrator,
  #[error("the room's number is not a room's number")]
  Chat,
}

impl RoomRemovalRefusal {
  fn answers(&self) -> StatusCode {
    match self {
      Self::SeededRoom | Self::Infrastructure | Self::OwnersChat => StatusCode::CONFLICT,
      Self::NotAdministrator => StatusCode::FORBIDDEN,
      Self::Chat => StatusCode::BAD_REQUEST,
    }
  }
}

coded!(RoomRemovalRefusal);
refused!(RoomRemovalRefusal);

/// What the confirmation of a removal warns of, from the receiver's own record; what another unit holds is
/// that unit's answer, shown as it came.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, IntoStaticStr, VariantNames)]
#[serde(tag = "code", rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum RoomRemovalWarning {
  /// Companions answer the room: they stay, and no longer answer in it.
  Answerers { names: Vec<String> },
  /// A road can still reach the room, where a later message would register it anew.
  Road { road: String },
  /// The room is another user's private chat.
  Private { user: String },
}

coded!(RoomRemovalWarning);

impl std::fmt::Display for RoomRemovalWarning {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Answerers { names } => write!(
        f,
        "companions answer this room: {}; they stay, and no longer answer in it",
        crate::listed(names)
      ),
      Self::Road { road } => {
        write!(f, "the road {road} can still reach this room; a later message registers it anew")
      }
      Self::Private { user } => write!(f, "this is the private chat of the user {user}"),
    }
  }
}

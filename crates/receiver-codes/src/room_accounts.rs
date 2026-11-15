//! A room's members by account and an account's words in the console refused (docs/receiver.md, 「Console rooms
//! and accounts」).

use http::StatusCode;
use strum::{IntoStaticStr, VariantNames};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, IntoStaticStr, VariantNames)]
#[strum(serialize_all = "kebab-case")]
pub enum RoomAccountRefusal {
  /// The room is one account's private room with its BMO: no other account speaks in it, and no account is let
  /// into it or taken out of it.
  #[error(
    "room {room} is one account's private room, so account {account} cannot speak in it or change its members"
  )]
  PrivateRoom { room: i64, account: i64 },
}

impl RoomAccountRefusal {
  fn answers(&self) -> StatusCode {
    match self {
      Self::PrivateRoom { .. } => StatusCode::FORBIDDEN,
    }
  }
}

coded!(RoomAccountRefusal);
refused!(RoomAccountRefusal);

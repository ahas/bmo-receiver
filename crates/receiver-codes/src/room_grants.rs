//! A room's delegations changed from a shell (docs/rooms.md, 「Delegations」).

use http::StatusCode;
use strum::{IntoStaticStr, VariantNames};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, IntoStaticStr, VariantNames)]
#[strum(serialize_all = "kebab-case")]
pub enum RoomGrantRefusal {
  /// A room's rules change only on a message the room's owner sent there.
  #[error("a room's rules change only on a message its owner sent in the room")]
  NotOwnersMessage,
  /// A grant of one relation asked where the account model is not in force.
  #[error("a grant of one relation needs the account model, which is not in force")]
  NoAccountModel,
}

impl RoomGrantRefusal {
  fn answers(&self) -> StatusCode {
    match self {
      Self::NotOwnersMessage => StatusCode::FORBIDDEN,
      Self::NoAccountModel => StatusCode::CONFLICT,
    }
  }
}

coded!(RoomGrantRefusal);
refused!(RoomGrantRefusal);

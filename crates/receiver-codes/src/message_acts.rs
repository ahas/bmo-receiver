//! A signed-in person's act on a message refused (docs/api.md, 「Messages」).

use http::StatusCode;
use strum::{IntoStaticStr, VariantNames};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, IntoStaticStr, VariantNames)]
#[strum(serialize_all = "kebab-case")]
pub enum MessageActRefusal {
  #[error("the console names no signed-in person who holds a sender")]
  NoCaller,
  #[error("room {room} keeps no message {message}")]
  MessageUnknown { room: i64, message: i64 },
  #[error("message {message} of room {room} was deleted")]
  MessageDeleted { room: i64, message: i64 },
  #[error("message {message} of room {room} is another person's")]
  MessageForbidden { room: i64, message: i64 },
  #[error("the record's sticker set holds no {emoji}")]
  UnknownSticker { emoji: String },
}

impl MessageActRefusal {
  fn answers(&self) -> StatusCode {
    match self {
      Self::NoCaller | Self::MessageForbidden { .. } => StatusCode::FORBIDDEN,
      Self::MessageUnknown { .. } => StatusCode::NOT_FOUND,
      Self::MessageDeleted { .. } => StatusCode::CONFLICT,
      Self::UnknownSticker { .. } => StatusCode::BAD_REQUEST,
    }
  }
}

coded!(MessageActRefusal);
refused!(MessageActRefusal);

//! An act on a message refused: a signed-in person's, or the record's own correction of who said it (docs/api.md,
//! 「Messages」).

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
  #[error("message {message} of room {room} does not stand under {agent}'s own number there")]
  MessageNotAgents {
    room: i64,
    message: i64,
    agent: String,
  },
  #[error("the record's sticker set holds no {emoji}")]
  UnknownSticker { emoji: String },
}

impl MessageActRefusal {
  fn answers(&self) -> StatusCode {
    match self {
      Self::NoCaller | Self::MessageForbidden { .. } => StatusCode::FORBIDDEN,
      Self::MessageUnknown { .. } => StatusCode::NOT_FOUND,
      Self::MessageDeleted { .. } | Self::MessageNotAgents { .. } => StatusCode::CONFLICT,
      Self::UnknownSticker { .. } => StatusCode::BAD_REQUEST,
    }
  }
}

coded!(MessageActRefusal);
refused!(MessageActRefusal);

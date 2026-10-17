//! A line in the consult room refused (docs/consult.md, 「The consult tool」).

use http::StatusCode;
use strum::{IntoStaticStr, VariantNames};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, IntoStaticStr, VariantNames)]
#[strum(serialize_all = "kebab-case")]
pub enum ConsultRefusal {
  #[error("the words for the consult room are empty")]
  Empty,
  #[error("no consult room is set (bmo consult room <room>)")]
  NoRoom,
  #[error("{agent} does not answer in the consult room {room}")]
  NotAnswerer { agent: String, room: String },
  #[error("this call was made outside any agent's turn, so there is no agent to speak as")]
  NoAgent,
}

impl ConsultRefusal {
  fn answers(&self) -> StatusCode {
    match self {
      Self::Empty | Self::NoAgent => StatusCode::BAD_REQUEST,
      Self::NoRoom | Self::NotAnswerer { .. } => StatusCode::CONFLICT,
    }
  }
}

coded!(ConsultRefusal);
refused!(ConsultRefusal);

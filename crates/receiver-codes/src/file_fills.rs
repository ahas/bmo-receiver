//! A room's placeholder rows filled later from its chat service (docs/room-files.md, 「Files filled in later」).

use http::StatusCode;
use strum::{IntoStaticStr, VariantNames};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, IntoStaticStr, VariantNames)]
#[strum(serialize_all = "kebab-case")]
pub enum FileFillRefusal {
  /// No road holds the room, or its road declares no `fetchMedia`, so no row of it can be filled.
  #[error("no road of room {room} fetches a message's files again, so none of its rows can be filled")]
  RoomFetchesNoFiles { room: i64 },
  /// A pass already fills the room's rows.
  #[error("a pass already fills the rows of room {room}")]
  RoomFilesFilling { room: i64 },
}

impl FileFillRefusal {
  fn answers(&self) -> StatusCode {
    match self {
      Self::RoomFetchesNoFiles { .. } | Self::RoomFilesFilling { .. } => StatusCode::CONFLICT,
    }
  }
}

coded!(FileFillRefusal);
refused!(FileFillRefusal);

//! A picture set from the console refused (docs/pictures.md, 「Profile pictures」).

use http::StatusCode;
use strum::{IntoStaticStr, VariantNames};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, IntoStaticStr, VariantNames)]
#[strum(serialize_all = "kebab-case")]
pub enum AvatarRefusal {
  #[error("the caller may not set or let go this {kind}'s picture")]
  PictureForbidden { kind: String },
  #[error("{kind} is none of the kinds a picture is set on: room, person and agent")]
  UnknownKind { kind: String },
  #[error("the form holds no picture")]
  NoPictureGiven,
  #[error("the picture cannot be read as one")]
  NotAPicture,
}

impl AvatarRefusal {
  fn answers(&self) -> StatusCode {
    match self {
      Self::PictureForbidden { .. } => StatusCode::FORBIDDEN,
      Self::UnknownKind { .. } | Self::NoPictureGiven | Self::NotAPicture => StatusCode::BAD_REQUEST,
    }
  }
}

coded!(AvatarRefusal);
refused!(AvatarRefusal);

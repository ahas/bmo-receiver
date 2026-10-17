//! The owner's words and files from the console refused (docs/receiver.md, 「The owner's words」).

use http::StatusCode;
use serde::{Deserialize, Serialize};
use strum::{IntoStaticStr, VariantNames};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, IntoStaticStr, VariantNames)]
#[strum(serialize_all = "kebab-case")]
pub enum OwnerSendRefusal {
  #[error("the message to reply to is not in the record")]
  ReplyMissing,
  #[error("this room takes words only, so no file can be sent")]
  WordsOnly,
  #[error("{name} is {size_bytes} bytes; one file may be at most {largest_bytes} bytes")]
  TooLarge {
    name: String,
    size_bytes: u64,
    largest_bytes: u64,
  },
  #[error("no file went: {}", crate::listed(.unsent))]
  NoneSent { unsent: Vec<FileUnsent> },
}

impl OwnerSendRefusal {
  fn answers(&self) -> StatusCode {
    match self {
      Self::ReplyMissing | Self::WordsOnly | Self::NoneSent { .. } => StatusCode::CONFLICT,
      Self::TooLarge { .. } => StatusCode::PAYLOAD_TOO_LARGE,
    }
  }
}

coded!(OwnerSendRefusal);
refused!(OwnerSendRefusal);

/// Why one file of a send did not go, beside the files that did.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, IntoStaticStr, VariantNames)]
#[serde(tag = "kind", rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum FileUnsent {
  /// The file holds no bytes.
  Empty { name: String },
  /// The file is past the ceiling one file may be.
  TooLarge {
    name: String,
    size_bytes: u64,
    largest_bytes: u64,
  },
  /// The road refused it, in its own words.
  Refused { name: String, reason: String },
}

impl std::fmt::Display for FileUnsent {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Empty { name } => write!(f, "{name}: the file is empty"),
      Self::TooLarge {
        name,
        size_bytes,
        largest_bytes,
      } => {
        write!(f, "{name}: {size_bytes} bytes is past the {largest_bytes} bytes one file may be")
      }
      Self::Refused { name, reason } => write!(f, "{name}: {reason}"),
    }
  }
}

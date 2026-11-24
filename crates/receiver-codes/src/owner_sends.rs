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
  #[error("{name} is {size_bytes} bytes, past the {largest_bytes} bytes {provider} takes a file at")]
  TooLarge {
    name: String,
    size_bytes: u64,
    largest_bytes: u64,
    provider: String,
  },
  #[error("no file went: {}", crate::listed(.unsent))]
  NoneSent { unsent: Vec<FileUnsent> },
  #[error("the room {room} is archived and locked, so nothing is sent into it")]
  RoomArchived { room: i64 },
}

impl OwnerSendRefusal {
  fn answers(&self) -> StatusCode {
    match self {
      Self::ReplyMissing | Self::WordsOnly | Self::NoneSent { .. } | Self::RoomArchived { .. } => {
        StatusCode::CONFLICT
      }
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
  /// The file is past the largest the room takes, which `provider` declares.
  TooLarge {
    name: String,
    size_bytes: u64,
    largest_bytes: u64,
    provider: String,
  },
  /// The file was not kept: `code` is the road's refusal code where a chat road refused it, and otherwise the
  /// receiver's own; `reason` is the English sentence for the log, never shown on a screen.
  Refused {
    name: String,
    code: String,
    reason: String,
  },
}

impl std::fmt::Display for FileUnsent {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Empty { name } => write!(f, "{name}: the file is empty"),
      Self::TooLarge {
        name,
        size_bytes,
        largest_bytes,
        provider,
      } => {
        write!(
          f,
          "{name}: {size_bytes} bytes is past the {largest_bytes} bytes {provider} takes a file at"
        )
      }
      Self::Refused { name, reason, .. } => write!(f, "{name}: {reason}"),
    }
  }
}

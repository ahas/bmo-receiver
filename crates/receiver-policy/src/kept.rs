//! How long the receiver keeps what it reads of another unit before it asks again (docs/receiver.md, 「What is
//! kept for a moment」).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kept {
  /// How long a chat service's answer about a room is kept, in seconds.
  pub room_seconds: u64,
  /// How long the agency's list of agents is kept, in seconds.
  pub agents_seconds: u64,
  /// How long the account a chat service's person is linked to is kept, in seconds.
  pub link_seconds: u64,
}

crate::part!(pub KEPT: Kept = "kept.toml");

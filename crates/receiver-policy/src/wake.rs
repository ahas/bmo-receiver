//! The wake's declared lengths (docs/room-files.md, 「Who a file wakes」).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WakeLengths {
  /// How far back a person's file is read as leading up to their words, in seconds.
  pub file_lead_seconds: i64,
  /// How many of the room's latest messages are looked through for those files.
  pub file_lead_messages: u64,
}

crate::part!(pub WAKE: WakeLengths = "wake.toml");

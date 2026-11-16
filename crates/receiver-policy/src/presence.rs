//! Typing marks and presence held from the chat services (docs/receiver.md, 「Typing and presence」).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Presence {
  /// How often a chat service whose marks or presence stand is asked whether it is ready, in seconds.
  pub provider_look_seconds: u64,
}

crate::part!(pub PRESENCE: Presence = "presence.toml");

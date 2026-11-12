//! The voice briefing's whole switch (docs/voiced-notices.md, 「The switches」): how a stop the engine did not take
//! is asked again.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoiceBriefing {
  /// How long the receiver waits before asking the engine to stop again, in milliseconds.
  pub stop_again_ms: u64,
}

crate::part!(pub VOICE_BRIEFING: VoiceBriefing = "voice-briefing.toml");

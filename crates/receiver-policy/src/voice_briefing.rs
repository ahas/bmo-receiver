//! The voice briefing's whole switch (docs/voiced-notices.md, 「The switches」): how a stop the engine did not take
//! is asked again; and how an answer said aloud is waited on (docs/voiced-notices.md, 「An answer said aloud」).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoiceBriefing {
  /// How long the receiver waits before asking the engine to stop again, in milliseconds.
  pub stop_again_ms: u64,
  /// How often an answer's speech is read while the engine speaks it, in milliseconds.
  pub speech_read_every_ms: u64,
  /// How long an answer's speech is waited for before the answer goes out written, in milliseconds.
  pub speech_within_ms: u64,
}

crate::part!(pub VOICE_BRIEFING: VoiceBriefing = "voice-briefing.toml");

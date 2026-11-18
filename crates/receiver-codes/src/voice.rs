//! The voice briefing (docs/voiced-notices.md): a briefing the receiver will not launch, and the fact the owner's
//! session is told when the briefing's switch moves.

use http::StatusCode;
use serde::{Deserialize, Serialize};
use strum::{IntoStaticStr, VariantNames};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, IntoStaticStr, VariantNames)]
#[strum(serialize_all = "kebab-case")]
pub enum VoiceRefusal {
  #[error(
    "{label} has no voice, so nothing is spoken. Say this in words only, and do not try to send it as a voice again"
  )]
  Voiceless { label: String },
  #[error(
    "a voice briefing has to say who launched it: --by chat:<chatId> from a session, or the launcher's own name"
  )]
  Unsigned,
  #[error(
    "a voice briefing has to have something to say: the words are read from stdin, or given with --text"
  )]
  Wordless,
  #[error("the voice briefing's unit {unit} did not come up; nothing was said")]
  NotUp { unit: String },
  #[error("say which switch is moved, on or autoplay, and whether it is true or false")]
  OneSwitch,
  #[error("the voice engine did not take the briefing: {reason}")]
  Unreachable { reason: String },
}

impl VoiceRefusal {
  fn answers(&self) -> StatusCode {
    match self {
      Self::Voiceless { .. } => StatusCode::CONFLICT,
      Self::Unsigned | Self::Wordless | Self::OneSwitch => StatusCode::BAD_REQUEST,
      Self::NotUp { .. } | Self::Unreachable { .. } => StatusCode::BAD_GATEWAY,
    }
  }
}

coded!(VoiceRefusal);
refused!(VoiceRefusal);

/// What the owner's session is told of the voice briefing, to say in its own words.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, IntoStaticStr, VariantNames)]
#[serde(tag = "code", rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum VoiceFact {
  /// The owner switched the whole briefing on from the console.
  BriefingOn,
  /// The owner switched the whole briefing off from the console.
  BriefingOff,
}

coded!(VoiceFact);

impl std::fmt::Display for VoiceFact {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::BriefingOn => f.write_str(
        "The owner switched the voice briefing on from the console. The notice turns of the owner's private chat are \
         summarised aloud again, unasked.",
      ),
      Self::BriefingOff => f.write_str(
        "The owner switched the voice briefing off from the console. Nothing is summarised aloud unasked while it is \
         off; a line's full reading is still made when somebody asks for it from the line.",
      ),
    }
  }
}

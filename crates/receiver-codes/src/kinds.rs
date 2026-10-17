//! The kinds a field takes where it names a sort of thing a screen or a record shows: the title a run stands
//! under, why a session was replaced, what opened a turn that no person's words opened, why a road was passed
//! over, and how a companion's engine failed. Each is a word the console's language pack renders.

use serde::{Deserialize, Serialize};
use strum::{IntoStaticStr, VariantNames};

/// The title a run of the receiver's stands under in the run log and on the live board.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, IntoStaticStr, VariantNames)]
#[serde(tag = "kind", rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum RunTitle {
  /// A round of a room's conversation.
  Chat,
  /// A room's session folded.
  Fold,
  /// A voice briefing for a room.
  VoiceBriefing { room: String },
  /// A line said into a room by a run of its own.
  Say { room: String },
}

/// Why a room's session was replaced rather than resumed.
#[derive(
  Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, strum::Display, IntoStaticStr, VariantNames,
)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum SessionReason {
  /// The provider or the model the turn runs on changed.
  Route,
  /// The account changed.
  Account,
  /// The node the session ran on is gone.
  Placement,
  /// The system prompt or the head of the run changed.
  Prefix,
  /// The cache's lifetime passed.
  Expired,
  /// The session holds no record of the prefix it was opened against.
  NoPrefix,
  /// A downgrade was noticed, and the session replaced.
  Downgrade,
  /// The model was replaced on its own, and the session after it.
  DowngradeForced,
}

/// What opened a turn of a room that no person's words opened, as the console's verdicts show it.
#[derive(
  Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, strum::Display, IntoStaticStr, VariantNames,
)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum TurnOpening {
  /// The words before a room's fold.
  Fold,
  /// A fold put off until the turn running ends.
  FoldDeferred,
}

/// Why a road was passed over for a turn.
#[derive(
  Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, strum::Display, IntoStaticStr, VariantNames,
)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum SkipReason {
  /// The provider is switched off.
  ProviderOff,
}

/// How a companion's engine ended its turn with an error.
#[derive(
  Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, strum::Display, IntoStaticStr, VariantNames,
)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum EngineError {
  /// The engine was busy.
  Busy,
  /// The engine refused the request.
  Refused,
  /// The answer was longer than the engine allows.
  TooLong,
  /// Any other failure of the engine.
  Failed,
}

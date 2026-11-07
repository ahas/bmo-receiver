//! The kinds a field takes where it names a sort of thing a screen or a record shows: the title a run stands
//! under, why a session was replaced, what opened a turn that no person's words opened, why a road was passed
//! over. Each is a word the console's language pack renders.

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
  /// The session filled its window, and its provider hands it over by its own note rather than a fold.
  Full,
  /// A handover was asked for the session.
  Asked,
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

/// Why a road was passed over for a turn, or a keepalive touch passed over for a session.
#[derive(
  Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, strum::Display, IntoStaticStr, VariantNames,
)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum SkipReason {
  /// The provider is switched off.
  ProviderOff,
  /// The agent keeps a session in each room, so it has no one session to keep warm.
  PerRoom,
  /// The session never stood in a room.
  NoSession,
  /// The session was used not long enough ago for a touch.
  UsedRecently,
  /// No conversations stand to be touched.
  NoConversation,
  /// The ping names a target other than the chat touch.
  NotChatTouch,
  /// The session's last turn is older than its provider's cache lifetime, so a touch would pay its whole prefix
  /// again.
  Expired,
}

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
  /// The pipeline's texts the session was handed changed: the chain or a stage's text moved.
  Process,
}

/// Why an agent's session was handed over to a fresh one, as the room's `session-handed-over` line names it.
#[derive(
  Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, strum::Display, IntoStaticStr, VariantNames,
)]
#[serde(rename_all = "kebab-case")]
#[strum(serialize_all = "kebab-case")]
pub enum HandoverCause {
  /// A person or the agent itself asked a handover.
  Asked,
  /// A reset of the session was asked through the agency.
  Reset,
  /// The session filled its window.
  Full,
  /// The road the session stood on moved: another provider or model.
  Road,
  /// The documents or the instructions the session was handed moved.
  Documents,
  /// Anything else, or a cause nothing names.
  Other,
}

impl HandoverCause {
  /// The cause a replacement the receiver makes for a reason goes by.
  pub fn of(reason: SessionReason) -> Self {
    match reason {
      SessionReason::Asked => Self::Asked,
      SessionReason::Full => Self::Full,
      SessionReason::Route | SessionReason::Downgrade | SessionReason::DowngradeForced => Self::Road,
      SessionReason::Prefix | SessionReason::Process => Self::Documents,
      SessionReason::Account
      | SessionReason::Placement
      | SessionReason::Expired
      | SessionReason::NoPrefix => Self::Other,
    }
  }
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

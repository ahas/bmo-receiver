//! What passes from a closed session to the one that replaces it (docs/unified-bmo.md, 「Replacing the
//! session」): the instruction the closing session writes its handoff file by, how much of the old conversation a
//! transcript keeps, the blocks that are the conversation, and the notes the new session's first turn opens with.

use serde::Deserialize;

/// One block of a round's prompt that is the conversation: the line it opens on and the line it closes on.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Block {
  pub open: String,
  pub close: String,
}

/// The line an answer is kept in a transcript by.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transcript {
  pub answer: String,
}

/// The lines of a replacement: the instruction asking the closing session for its handoff file, the one asking a
/// session about to be folded to consolidate its brain first, and the three notes a new session is handed: the
/// handoff, the transcript under its heading, and the word that nothing was recovered.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Notes {
  pub ask: Vec<String>,
  pub consolidate: Vec<String>,
  pub handed: Vec<String>,
  pub carried: Vec<String>,
  pub lost: Vec<String>,
}

/// The notice the owner is queued where nothing of a replaced session could be carried over.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LostNotice {
  pub intent: String,
  pub kind: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Handover {
  /// How much of an old conversation a transcript keeps, in UTF-16 units.
  pub transcript_tail_chars: usize,
  /// What a room's own speaker is named in a transcript.
  pub room_speaker: String,
  /// The span within which a handoff asked again for the same agent is not asked of its session, in minutes.
  pub again_minutes: i64,
  /// The folder under the data root an agent's handoffs are kept in, `{agent}` its name.
  pub folder: String,
  /// The name the newest handoff is copied to.
  pub latest: String,
  pub transcript: Transcript,
  pub conversation: Vec<Block>,
  pub chat: Notes,
  pub lost_notice: LostNotice,
}

crate::part!(pub HANDOVER: Handover = "handover.toml");

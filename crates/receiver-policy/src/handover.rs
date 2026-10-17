//! What passes from a room's closed session to the one that replaces it (docs/unified-bmo.md, 「Replacing the
//! session」): how much of the old conversation a transcript keeps, the blocks that are the conversation, and the
//! note the new session's first turn opens with.

use serde::Deserialize;

/// One block of a round's prompt that is the conversation: the line it opens on and the line it closes on.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Block {
  pub open: String,
  pub close: String,
}

/// The lines a transcript is written in.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transcript {
  pub room: String,
  pub answer: String,
}

/// The two notes a room's session is handed: the summary under its heading, and the word that nothing was
/// recovered.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Notes {
  pub carried: Vec<String>,
  pub lost: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Handover {
  /// How much of an old conversation a transcript keeps, in UTF-16 units.
  pub transcript_tail_chars: usize,
  /// What a room's own speaker is named in a transcript.
  pub room_speaker: String,
  pub transcript: Transcript,
  pub conversation: Vec<Block>,
  pub chat: Notes,
}

crate::part!(pub HANDOVER: Handover = "handover.toml");

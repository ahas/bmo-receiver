//! The words a turn of the chat agent's session answers with when it is not answering the room: wait for more, or
//! say nothing this time; and the one a turn of the draft process opens on to make its answer the message now.

use crate::js::trim;

pub const WAIT: &str = "<<WAIT>>";
pub const SKIP: &str = "<<SKIP>>";
pub const SEND: &str = "<<SEND>>";

/// Whether an answer is nothing but waits, or nothing at all.
pub fn is_wait(text: &str) -> bool {
  trim(&text.replace(WAIT, "")).is_empty()
}

/// Whether an answer is nothing but skips, or nothing at all.
pub fn is_skip(text: &str) -> bool {
  trim(&text.replace(SKIP, "")).is_empty()
}

/// The words after the send marker, where an answer opens on it: the message the answer makes now
/// (docs/drafting.md, 「What it is」).
pub fn sent_now(text: &str) -> Option<&str> {
  trim(text).strip_prefix(SEND).map(trim)
}

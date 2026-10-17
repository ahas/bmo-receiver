//! The two words a turn of the chat agent's session answers with when it is not answering the room: wait for
//! more, or say nothing this time.

use crate::js::trim;

pub const WAIT: &str = "<<WAIT>>";
pub const SKIP: &str = "<<SKIP>>";

/// Whether an answer is nothing but waits, or nothing at all.
pub fn is_wait(text: &str) -> bool {
  trim(&text.replace(WAIT, "")).is_empty()
}

/// Whether an answer is nothing but skips, or nothing at all.
pub fn is_skip(text: &str) -> bool {
  trim(&text.replace(SKIP, "")).is_empty()
}

//! A line a person typed into the chat agent's program, as the round that composes it reads it (docs/bmo-window.md,
//! 「A line typed into the program」).
//!
//! The line is the owner's — it carries no sender, and only he reaches the program — and it is read in his
//! private chat as one new message of his. It is no message of the archive, so it stands under the number no
//! archived message has, and the composition is headed by what makes this turn unlike a round: the answer stays
//! on the program's screen.

use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::fill_marks;

use super::conversation::{BoxFuture, Conversations, Typing};
use crate::Fault;
use crate::receiver::{Inbound, Receiver};

/// The line as one message of the owner's in his private chat, sent at `at_ms`.
pub fn message(text: &str, owner: i64, name: &str, at_ms: i64) -> Inbound {
  Inbound {
    chat_id: owner,
    message_id: CHAT_ROUND.typed.number,
    sender_id: Some(owner),
    sender_name: name.to_owned(),
    sent_at: at_ms,
    text: text.to_owned(),
    image_path: None,
    reply_to_message_id: None,
    via: String::new(),
    mode: None,
    via_reader: None,
    album: None,
  }
}

/// What the program is handed beside the line: the lead, then the round composed for it.
pub fn context(composed: &str) -> String {
  let lead = fill_marks(&CHAT_ROUND.typed.lead, &[("number", &CHAT_ROUND.typed.number.to_string())]);

  format!("{lead}\n\n{composed}")
}

// The chat agent's conversations compose a line typed into its program.
impl Typing for Conversations {
  fn typed_line<'a>(
    &'a self,
    _receiver: &'a Receiver,
    _agent: &'a str,
    text: &'a str,
    session_id: &'a str,
    at_ms: i64,
  ) -> BoxFuture<'a, Result<String, Fault>> {
    Box::pin(Conversations::typed_line(self, text, session_id, at_ms))
  }
}

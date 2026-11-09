//! A working supervisor's thinking, sent into the owner's private chat as its own messages while the turn works
//! (docs/receiver.md, 「The event stream」): each block as the run wrote it, the record marking it with the policy's
//! kind, cut past its ceiling, and never the turn's answer. Only the owner's private chat is sent to, since
//! thinking can quote private or other-room material and a group room holds other people.

use receiver_policy::chat_round::CHAT_ROUND;
use tokio::sync::mpsc;

use super::reply;
use super::talk::clip;
use crate::receiver::Receiver;

/// A working turn's thinking being sent, in the order it came, off the turn's own path so nothing waits on the
/// chat service.
pub struct Thoughts {
  tx: mpsc::UnboundedSender<String>,
}

impl Thoughts {
  /// The sending begun for a turn: `receiver` and its room, the road the turn answers on and the agent whose
  /// thinking it is.
  pub fn start(receiver: Receiver, chat_id: i64, via: Option<String>, agent: String) -> Self {
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    tokio::spawn(async move {
      while let Some(text) = rx.recv().await {
        let said = cut(&text);
        let kind = &CHAT_ROUND.thinking.kind;

        if reply::send_thinking(&receiver, chat_id, via.as_deref(), &agent, &said, kind)
          .await
          .is_none()
        {
          eprintln!("chat {chat_id}: a thinking block was not sent");
        }
      }
    });

    Thoughts { tx }
  }

  /// One thinking block to send, as the run wrote it; an empty one says nothing.
  pub fn say(&self, text: &str) {
    let text = text.trim();

    if !text.is_empty() {
      let _ = self.tx.send(text.to_owned());
    }
  }
}

/// A thinking block as the room reads it: the block as it stands with its secret-shaped runs taken out, cut at the
/// ceiling with the cut marked. The record marks the message itself; no label is written into the words.
fn cut(text: &str) -> String {
  let text = crate::secrets::redacted(text);

  clip(&text, CHAT_ROUND.thinking.ceiling_chars)
}

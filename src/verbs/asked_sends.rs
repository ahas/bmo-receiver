//! A send a `bmo` command asks the running receiver for (docs/chat-mcp.md, 「How a call runs」): a shell holds
//! none of the receiver's lines, so a line said, a picture or an album, a reaction, a pin, a sticker or a fair
//! draw is carried by the receiver, as the agent whose turn the shell works in, through the asking part of
//! `room-calls` the sending tools ask it by too.

use std::path::Path;

use room_calls::bot::Service;
use room_calls::sends::{Act, Asked, Asker, Unsent};

use crate::verbs::{VerbError, turn};

/// Who a shell's send is asked by: the room, the road the shell names, the chat that set it off, and the agent
/// whose turn the shell works in, with whether a job runs it.
pub fn shell_asker(chat: i64, via: Option<String>, by: Option<String>) -> Asker {
  Asker {
    room: Some(chat),
    via,
    agent_name: turn::agent(),
    by,
    job: turn::inside_job(),
  }
}

/// One send asked of the receiver the data root belongs to, answering the lines it said of it.
pub async fn ask(data_root: &Path, asker: Asker, act: Act) -> Result<Vec<String>, VerbError> {
  said(room_calls::sends::ask(&Service::of(data_root), &Asked { asker, act }).await)
}

/// What the receiver said of a send, as a verb answers it: the lines it said before a refusal are printed with it.
pub fn said(answer: Result<Vec<String>, Unsent>) -> Result<Vec<String>, VerbError> {
  answer.map_err(|unsent| match unsent {
    Unsent::Unreached(why) => VerbError::Refused(why),
    untaken @ Unsent::Untaken(_) => VerbError::Refused(untaken.lines().join("\n")),
    Unsent::Refused { lines, refusal } if lines.is_empty() => VerbError::Refused(refusal),
    Unsent::Refused { lines, refusal } => VerbError::Partly { lines, refusal },
  })
}

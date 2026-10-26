//! Words that arrive while a round's turn is working, put into that turn at its next look rather than held for
//! the next round (the agency's `docs/agent-control.md`, 「Words that arrive while a turn is working」): the one paragraph they
//! go in as — what arrived, that this same answer takes them in, the messages and instructions formatted and
//! cleaned exactly as a round's own are, the answer they are owed — behind whatever the window selects for them
//! that the round does not already hold.

use std::collections::HashSet;

use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::fill_marks;

use super::shape::{Reading, sends};
use crate::Fault;
use crate::receiver::Receiver;
use crate::receiver::{Inbound, Room};
use crate::roads::Roads;
use receiver_ports::gate::TurnEvent;

/// What a push carries into the working turn.
pub struct Push<'a> {
  pub receiver: &'a Receiver,
  /// The agent whose turn it goes into.
  pub agent: &'a str,
  pub owner: i64,
  pub chat_id: i64,
  pub room: Room,
  pub round: &'a str,
  pub messages: &'a [Inbound],
  pub instructions: &'a [String],
  pub events: &'a [TurnEvent],
  /// What the round already holds, by path.
  pub holds: &'a HashSet<String>,
  /// The message the round has read up to before the push.
  pub consumed: Option<i64>,
  pub tools_allowed: bool,
  pub reader: Option<&'a Roads>,
}

/// The paragraph a push goes in as, the instructions as the session reads them, and the paths of what the
/// window selected for it, which the round holds from the moment the push is taken.
pub struct Paragraph {
  pub text: String,
  pub instructions: String,
  pub reference: Vec<String>,
}

// What arrived, as the paragraph's opening says it.
pub fn note(messages: usize, instructions: usize) -> String {
  let push = &CHAT_ROUND.push;
  let mut what = vec![];

  if messages > 0 {
    what.push(fill_marks(&push.messages, &[("count", &messages.to_string())]));
  }

  if instructions > 0 {
    what.push(fill_marks(&push.instructions, &[("count", &instructions.to_string())]));
  }

  fill_marks(&push.note, &[("what", &what.join(&push.joined))])
}

/// The paragraph the push's words go into the working turn as.
pub async fn paragraph(push: &Push<'_>) -> Result<Paragraph, Fault> {
  let receiver = push.receiver;
  let chat_id = push.chat_id;
  let blocks = &CHAT_ROUND.blocks;
  let shown = HashSet::new();
  let reading = |fetch: bool| Reading {
    receiver,
    owner: push.owner,
    fetch_on_demand: fetch,
    shown: &shown,
  };
  let mut parts = vec![note(sends(push.messages).len(), push.instructions.len())];

  if !push.messages.is_empty() {
    let lines = reading(push.tools_allowed).lines(push.messages).await?;
    let ids: Vec<i64> = push.messages.iter().map(|message| message.message_id).collect();

    parts.push(String::new());
    parts.push(fill_marks(&blocks.fresh, &[("count", &lines.len().to_string())]));
    parts.extend(lines);
    parts.push(blocks.fresh_end.clone());
    parts.push(fill_marks(
      &CHAT_ROUND.push.scope,
      &[("ids", &serde_json::to_string(&ids).unwrap_or_default())],
    ));
  }

  // A push carries no piece: the round's index went in with its turn (docs/bmo-window.md, 「The openings」).
  let instructions = push.instructions.join("\n\n");

  if !push.instructions.is_empty() {
    parts.push(String::new());
    parts.push(fill_marks(&blocks.quiet, &[("count", &push.instructions.len().to_string())]));
    parts.push(instructions.clone());
    parts.push(blocks.quiet_end.clone());
  }

  let owed = if chat_id == push.owner {
    push.messages.iter().filter(|message| message.sender_id == Some(push.owner)).count()
  } else {
    0
  };

  if owed > 0 {
    parts.push(String::new());
    parts.push(fill_marks(&CHAT_ROUND.closing.owed, &[("count", &owed.to_string())]));
  }

  Ok(Paragraph {
    text: parts.join("\n"),
    instructions,
    reference: vec![],
  })
}

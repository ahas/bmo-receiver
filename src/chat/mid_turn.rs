//! Words that arrive while a round's turn is working, put into that turn at its next look rather than held for
//! the next round (the agency's `docs/agent-control.md`, 「Words that arrive while a turn is working」;
//! docs/bmo-window.md, 「The openings」, `push`): the one paragraph they go in as — what arrived, that only what is
//! written after it goes out, the rule each message is judged by and how the instructions are taken, the messages
//! and instructions formatted and cleaned exactly as a round's own are. The same paragraph, without the rule the
//! room's block already states, stands under a question of the draft process for the words no push put in
//! (docs/drafting.md, 「What came in meanwhile」).

use std::collections::HashSet;

use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::fill_marks;

use super::shape::{Reading, sends};
use crate::Fault;
use crate::receiver::Inbound;
use crate::receiver::Receiver;
use crate::roads::Roads;
use receiver_ports::gate::TurnEvent;

/// What a push carries into the working turn.
pub struct Push<'a> {
  pub receiver: &'a Receiver,
  /// The agent whose turn it goes into.
  pub agent: &'a str,
  pub owner: i64,
  pub chat_id: i64,
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
  /// Whether the paragraph carries the rule each message is judged by: a push into a working turn does, and a
  /// block under a question leans on the rules the room's block states.
  pub judged: bool,
  /// Whether the paragraph opens on the note saying what arrived. A push into a working turn does; a block the
  /// draft process shows carries the messages as data, its rule standing in the room's block.
  pub announced: bool,
}

/// The paragraph a push goes in as, the instructions as the session reads them, and the paths of what the
/// window selected for it, which the round holds from the moment the push is taken.
pub struct Paragraph {
  pub text: String,
  pub instructions: String,
  pub reference: Vec<String>,
}

/// The paragraph's opening: what arrived and that only what is written after it goes out; then, where messages
/// came and `judged` asks for it, the rule each is judged by, and where instructions came, how they are taken.
pub fn note(messages: usize, instructions: usize, judged: bool) -> String {
  let push = &CHAT_ROUND.push;
  let mut what = vec![];

  if messages > 0 {
    what.push(fill_marks(&push.messages, &[("count", &messages.to_string())]));
  }

  if instructions > 0 {
    what.push(fill_marks(&push.instructions, &[("count", &instructions.to_string())]));
  }

  let mut note = vec![fill_marks(&push.note, &[("what", &what.join(&push.joined))])];

  if judged && messages > 0 {
    note.push(push.judge.clone());
  }

  if instructions > 0 {
    note.push(push.heed.clone());
  }

  note.join(" ")
}

/// The paragraph the push's words go into the working turn as.
pub async fn paragraph(push: &Push<'_>) -> Result<Paragraph, Fault> {
  let receiver = push.receiver;
  let blocks = &CHAT_ROUND.blocks;
  let shown = HashSet::new();
  let reading = |fetch: bool| Reading {
    receiver,
    owner: push.owner,
    fetch_on_demand: fetch,
    shown: &shown,
  };
  let mut parts = if push.announced {
    vec![note(sends(push.messages).len(), push.instructions.len(), push.judged)]
  } else {
    vec![]
  };

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

  Ok(Paragraph {
    text: parts.join("\n"),
    instructions,
    reference: vec![],
  })
}

//! What a companion's turn is given and what it answers, as words (the agency's `docs/companions.md`, 「The session」,
//! 「What a turn shows」): the room's lines under each speaker's name with the owner's
//! mark on his alone, the parts one turn's words fall into, the blocks its reference and its handoff note stand in,
//! an answer that repeats what it was given, and the lines a cut is told in.

use std::collections::HashSet;

use receiver_codes::{EngineError, FixedLine};
use receiver_policy::companion_runner::COMPANION_RUNNER;
use receiver_policy::fill_marks;
use receiver_policy::roads::ROADS;

use crate::js::{is_space, trim, units};
use crate::receiver::Inbound;

// Every case of the owner's mark taken out of a text, so nobody else can wear it.
fn without_mark(text: &str) -> String {
  let mark = &COMPANION_RUNNER.words.owner_mark;
  let lowered = text.to_ascii_lowercase();
  let wanted = mark.to_ascii_lowercase();
  let mut kept = String::new();
  let mut from = 0;

  while let Some(at) = lowered[from..].find(&wanted) {
    kept.push_str(&text[from..from + at]);
    from += at + wanted.len();
  }

  kept + &text[from..]
}

/// A word of the room as a turn reads it: under its speaker's name, a picture as its mark, and the owner's mark
/// ahead of his own; nothing where the words hold nothing.
pub fn line_of(message: &Inbound, owner: Option<i64>) -> Option<String> {
  let words = &COMPANION_RUNNER.words;
  let trimmed = trim(&message.text);
  let shown = if trimmed.is_empty() && message.image_path.is_some() {
    ROADS.marks.picture.as_str()
  } else {
    trimmed
  };
  let his = owner.is_some() && message.sender_id == owner;
  let said = if his { shown.to_owned() } else { trim(&without_mark(shown)).to_owned() };
  let name = trim(&without_mark(&message.sender_name)).to_owned();

  if said.is_empty() {
    return None;
  }

  Some(if his {
    format!("{} {name}: {said}", words.owner_mark)
  } else {
    format!("{name}: {said}")
  })
}

/// An instruction as a turn reads it: under the opening every quiet instruction opens with.
pub fn quiet_opening(text: &str) -> String {
  let head = &COMPANION_RUNNER.words.quiet_head;

  if text.starts_with(head.as_str()) { text.to_owned() } else { format!("{head} {text}") }
}

/// The parts one turn's words fall into: what came in since the agent's last turn — the room's other words first,
/// then the words that woke it, a part of their own and last — and each quiet instruction it carries after them.
pub fn new_turns(
  since: &[Inbound],
  batch: &[Inbound],
  quiet: &[String],
  owner: Option<i64>,
) -> Vec<String> {
  let answering: HashSet<i64> = batch.iter().map(|message| message.message_id).collect();
  let other: Vec<String> = since
    .iter()
    .filter(|message| !answering.contains(&message.message_id))
    .filter_map(|message| line_of(message, owner))
    .collect();
  let woke: Vec<String> = batch.iter().filter_map(|message| line_of(message, owner)).collect();
  let mut parts = Vec::new();

  if !other.is_empty() {
    parts.push(other.join("\n"));
  }

  if !woke.is_empty() {
    parts.push(woke.join("\n"));
  }

  parts.extend(quiet.iter().map(|text| quiet_opening(text)));
  parts
}

/// What the turn's gates attached, as the block that stands at the head of its words; empty where nothing was
/// attached.
pub fn reference_block(entries: &[(&'static str, String)]) -> String {
  if entries.is_empty() {
    return String::new();
  }

  let blocks = &COMPANION_RUNNER.blocks;
  let mut lines = vec![fill_marks(&blocks.reference_opening, &[("count", &entries.len().to_string())])];

  for (path, text) in entries {
    lines.push(format!("[{path}]"));
    lines.push(trim(text).to_owned());
  }

  lines.push(blocks.reference_closing.clone());
  lines.join("\n")
}

/// The note a handed-over session opens its first turn with, as the block that turn reads it in.
pub fn handoff_block(note: &str) -> String {
  let blocks = &COMPANION_RUNNER.blocks;

  [blocks.note_opening.as_str(), trim(note), blocks.note_closing.as_str()].join("\n")
}

// A text with every run of space made one, and trimmed.
fn flat(text: &str) -> String {
  let mut out = String::new();
  let mut spaced = false;

  for character in text.chars() {
    if is_space(character) {
      spaced = true;
    } else {
      if spaced && !out.is_empty() {
        out.push(' ');
      }

      spaced = false;
      out.push(character);
    }
  }

  out
}

// The first `count` UTF-16 units of a text.
fn first_units(text: &str, count: usize) -> String {
  let units: Vec<u16> = text.encode_utf16().take(count).collect();

  String::from_utf16_lossy(&units)
}

/// What an answer repeats of what its turn was given rather than the room, or nothing: a phrase the program writes
/// itself into the conversation, or a long line of the system prompt standing whole in it.
pub fn echoed_prompt(answer: &str, system: &str, own_words: &[&str]) -> Option<String> {
  let lines = &COMPANION_RUNNER.answer;
  let said = flat(answer);
  let lowered = said.to_lowercase();

  if let Some(word) = own_words.iter().find(|phrase| lowered.contains(&phrase.to_lowercase())) {
    return Some(fill_marks(&lines.echoed_words, &[("words", word)]));
  }

  system
    .split('\n')
    .map(flat)
    .find(|line| units(line) >= COMPANION_RUNNER.echoed_line_chars && said.contains(line.as_str()))
    .map(|line| {
      fill_marks(&lines.echoed_line, &[("line", &first_units(&line, COMPANION_RUNNER.echoed_line_chars))])
    })
}

/// The engine's own words for how its turn ended, as the program names them, into the kinds the fixed line is
/// told by: busy or over a limit, refused, an answer past the engine's length, and any other error.
pub fn engine_error_of(kind: Option<&str>) -> EngineError {
  match kind {
    Some("rate_limit") | Some("overloaded") => EngineError::Busy,
    Some("authentication_failed")
    | Some("oauth_org_not_allowed")
    | Some("account_on_hold")
    | Some("verification_required")
    | Some("billing_error")
    | Some("cloud_credential_error")
    | Some("model_not_found")
    | Some("invalid_request") => EngineError::Refused,
    Some("max_output_tokens") => EngineError::TooLong,
    _ => EngineError::Failed,
  }
}

/// The one line a turn its engine ended with an error leaves in the room, under the companion's own name.
pub fn engine_error_notice(kind: Option<&str>) -> FixedLine {
  FixedLine::CompanionEngineError {
    kind: engine_error_of(kind),
  }
}

/// The one line the room is told of a cut turn in, under the companion's own name and with the guard's reason.
pub fn cut_notice(agent: &str, reason: &str) -> FixedLine {
  FixedLine::CompanionCut {
    agent: agent.to_owned(),
    reason: reason.to_owned(),
  }
}

/// What a cut turn tells BMO in the owner's chat.
pub struct CutTold<'a> {
  pub name: &'a str,
  pub agent: &'a str,
  pub chat_id: i64,
  pub title: Option<&'a str>,
  pub reason: &'a str,
  pub run: &'a str,
}

/// The quiet instruction a cut turn is told to BMO in.
pub fn cut_instruction(cut: &CutTold) -> String {
  let chat = cut.chat_id.to_string();
  let values = [
    ("name", cut.name),
    ("agent", cut.agent),
    ("room", cut.title.unwrap_or(&chat)),
    ("chat", chat.as_str()),
    ("reason", cut.reason),
    ("run", cut.run),
  ];

  COMPANION_RUNNER
    .cut
    .instruction
    .iter()
    .map(|line| fill_marks(line, &values))
    .collect::<Vec<_>>()
    .join("\n")
}

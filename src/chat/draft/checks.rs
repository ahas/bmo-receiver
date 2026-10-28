//! What the draft process reads of a text with no model (docs/drafting.md, 「The checks on the final text」 and 「A
//! light turn」): the skip standing in an answer, whether a turn is light, and the checks a final text passes.

use receiver_policy::drafting::DRAFTING;
use receiver_policy::fill_marks;
use receiver_policy::second_asks::SECOND_ASKS;

use super::super::sentinels::SKIP;
use crate::form::without_control_markers;
use crate::receiver::Inbound;

// How long a fixed head of a line of the process must run before the format check reads it.
const HEAD_CHARS: usize = 12;

/// A check the final text passes, in the order the process reads them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Check {
  Length,
  Format,
}

impl Check {
  pub fn word(self) -> &'static str {
    match self {
      Self::Length => "length",
      Self::Format => "format",
    }
  }
}

/// What the checks read a text against: the process's own lines and the longest text let go. The language an
/// answer is written in is the agent's own rule and no check reads it.
pub struct Reading<'a> {
  pub own_lines: &'a [String],
  pub max_chars: usize,
}

/// Whether the skip stands in an answer outside a code span or fence, where it is not being quoted.
pub fn has_skip(text: &str) -> bool {
  text.matches(SKIP).count() > without_control_markers(text).matches(SKIP).count()
}

/// Whether a turn is light: it carried quiet instructions and none of the room's words, or room words that hold
/// no file and run together, white space aside, to at most the bound.
pub fn light(batch: &[Inbound], instructions: usize, light_chars: u32) -> bool {
  if batch.is_empty() {
    return instructions > 0;
  }

  let chars: usize = batch
    .iter()
    .map(|message| message.text.chars().filter(|one| !one.is_whitespace()).count())
    .sum();

  batch.iter().all(|message| message.image_path.is_none()) && chars <= light_chars as usize
}

/// The fixed head of a line of the process: what stands before its first mark, where it runs long enough to be
/// told apart from the room's own words.
pub fn fixed_head(line: &str) -> Option<String> {
  let head = line.split('{').next().unwrap_or_default().trim();

  (head.chars().count() >= HEAD_CHARS).then(|| head.to_owned())
}

/// The process's own lines a final text must not carry: the questions asked and the fixed heads of its lines.
pub fn own_lines(asked: &[String]) -> Vec<String> {
  let heads = [
    &DRAFTING.opening,
    &DRAFTING.lead,
    &DRAFTING.last_lead,
    &DRAFTING.waiting,
    &DRAFTING.arrived,
  ]
  .into_iter()
  .filter_map(|line| fixed_head(line));

  asked
    .iter()
    .map(|text| text.trim().to_owned())
    .filter(|text| !text.is_empty())
    .chain(heads)
    .collect()
}

/// The checks a text fails, in their order.
pub fn failing(text: &str, reading: &Reading) -> Vec<Check> {
  let mut failed = vec![];

  if text.chars().count() > reading.max_chars {
    failed.push(Check::Length);
  }

  if reading.own_lines.iter().any(|line| text.contains(line.as_str())) {
    failed.push(Check::Format);
  }

  failed
}

/// The errand a failed check asks the session with, naming what is wrong in the text.
pub fn errand(check: Check, text: &str, reading: &Reading) -> String {
  let errands = &SECOND_ASKS.errands;

  match check {
    Check::Length => fill_marks(
      &errands.length,
      &[("count", &text.chars().count().to_string()), ("limit", &reading.max_chars.to_string())],
    ),
    Check::Format => errands.format.clone(),
  }
}

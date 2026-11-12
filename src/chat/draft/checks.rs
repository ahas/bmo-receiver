//! What the draft process reads of a text with no model (docs/drafting.md, 「The checks on the final text」): the
//! skip standing in an answer and the checks a final text passes.

use receiver_policy::drafting::DRAFTING;

use super::super::sentinels::skip;

// How long a fixed head of a line of the process must run before the format check reads it.
const HEAD_CHARS: usize = 12;

/// A check the final text passes, in the order the process reads them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Check {
  Format,
}

impl Check {
  pub fn word(self) -> &'static str {
    match self {
      Self::Format => "format",
    }
  }
}

/// What the check reads a text against: the process's own lines. The language an answer is written in is the
/// agent's own rule and no check reads it.
pub struct Reading<'a> {
  pub own_lines: &'a [String],
}

/// Whether a skip of either shape stands in an answer outside a code span or fence, where it is not being quoted.
/// What it names, where it names a reason, is read by `super::super::sentinels::skip`.
pub fn has_skip(text: &str) -> bool {
  skip(text).is_some()
}

/// The fixed head of a line of the process: what stands before its first mark, where it runs long enough to be
/// told apart from the room's own words.
pub fn fixed_head(line: &str) -> Option<String> {
  let head = line.split('{').next().unwrap_or_default().trim();

  (head.chars().count() >= HEAD_CHARS).then(|| head.to_owned())
}

/// The process's own lines a final text must not carry: the questions asked and the fixed heads of its lines.
pub fn own_lines(asked: &[String]) -> Vec<String> {
  let heads = [&DRAFTING.round, &DRAFTING.arrived_marker]
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

  if reading.own_lines.iter().any(|line| text.contains(line.as_str())) {
    failed.push(Check::Format);
  }

  failed
}

/// The turn a failed check asks the session with: the marker that names it. What the check does with the text
/// stands in the room's block.
pub fn errand(check: Check, _text: &str, _reading: &Reading) -> String {
  match check {
    Check::Format => super::check_words("format"),
  }
}

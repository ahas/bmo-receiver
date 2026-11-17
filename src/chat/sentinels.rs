//! The words a turn of the default answerer's session answers with when it is not answering the room: wait for more, or
//! say nothing this time, a round's turn naming on one line why it says nothing; the one a turn of the draft process
//! opens on to make its answer the message now; and the marks it sets on a message that arrived while the process ran.

use std::sync::LazyLock;

use regex::Regex;

use crate::form::{LARGEST_NAMED, without_control_markers};
use crate::js::trim;

pub const WAIT: &str = "<<WAIT>>";
pub const SKIP: &str = "<<SKIP>>";
pub const SEND: &str = "<<SEND>>";
pub const FOLD: &str = "<<FOLD>>";

// The opening of any of the draft process's answer marks: two opening angle brackets and a word in
// capitals, then the closing brackets, a colon or a number's `#`. A shell here-document's opening is none.
static ANSWER_MARK: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"<<[A-Z][A-Z_]*[ \t]*(?:>>|:|#)").expect("an answer mark"));

/// Whether a text holds one of the draft process's answer marks, and so is a draft of a turn's message
/// (docs/receiver.md, 「The event stream」).
pub fn holds_answer_mark(text: &str) -> bool {
  ANSWER_MARK.is_match(text)
}

/// Whether an answer is nothing but waits, or nothing at all; a fold asked of no words is nothing.
pub fn is_wait(text: &str) -> bool {
  trim(&text.replace(FOLD, "").replace(WAIT, "")).is_empty()
}

/// What a skip in an answer is read as (docs/drafting.md, 「What the receiver reads of an answer」).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Skip {
  /// `<<SKIP: <reason>>>`: nothing is sent and the answer says why, on the one line the reason is.
  Reasoned(String),
  /// A skip naming no reason: the bare `<<SKIP>>`, or the form with nothing after its colon. No verdict for a
  /// round's turn, which is asked once more over it.
  Bare,
}

impl Skip {
  /// The reason the skip names, where it names one.
  pub fn reason(&self) -> Option<&str> {
    match self {
      Self::Reasoned(reason) => Some(reason),
      Self::Bare => None,
    }
  }
}

// The skip in either shape: the bare marker, or the form naming its reason on one line, no angle bracket in it.
static SKIP_FORM: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"<<SKIP[ \t]*(?::[ \t]*([^<\n]*?)[ \t]*)?>>").expect("a skip"));

// The skips of a text, each with the reason it names: the bare marker names none, and white space alone is no
// reason either.
fn skips_in(text: &str) -> Vec<Skip> {
  SKIP_FORM
    .captures_iter(text)
    .map(|found| {
      let named = found.get(1).map_or("", |reason| reason.as_str());
      let reason = named.split_whitespace().collect::<Vec<_>>().join(" ");

      if reason.is_empty() { Skip::Bare } else { Skip::Reasoned(reason) }
    })
    .collect()
}

/// The skip an answer carries outside a code span or fence, where it is not being quoted. Where the answer carries
/// skips naming reasons and not, the verdict is the one that names a reason.
pub fn skip(text: &str) -> Option<Skip> {
  let mut quoted = skips_in(&without_control_markers(text));
  let outside: Vec<Skip> = skips_in(text)
    .into_iter()
    .filter(|said| match quoted.iter().position(|held| held == said) {
      Some(at) => {
        quoted.remove(at);
        false
      }
      None => true,
    })
    .collect();

  outside
    .iter()
    .find_map(Skip::reason)
    .map(|reason| Skip::Reasoned(reason.to_owned()))
    .or_else(|| (!outside.is_empty()).then_some(Skip::Bare))
}

/// Whether an answer is nothing but skips, of either shape, or nothing at all; a fold asked of no words is
/// nothing. This is what a housekeeping turn answers with, and an answer of nothing but a bare marker is a skip
/// naming no reason.
pub fn is_skip(text: &str) -> bool {
  trim(&SKIP_FORM.replace_all(&text.replace(FOLD, ""), "")).is_empty()
}

/// The skip of an answer that is nothing but skips, with the reason it names where the skips name one.
pub fn skip_alone(text: &str) -> Option<Skip> {
  is_skip(text).then(|| {
    skips_in(text)
      .into_iter()
      .find(|said| matches!(said, Skip::Reasoned(_)))
      .unwrap_or(Skip::Bare)
  })
}

/// Whether a message asks to be folded (docs/wake.md, 「Folds」): the fold mark stands in it outside a code span
/// or fence, where it is not being quoted.
pub fn asks_fold(text: &str) -> bool {
  text.matches(FOLD).count() > without_control_markers(text).matches(FOLD).count()
}

/// The words after the send marker, where an answer opens on it: the message the answer makes now
/// (docs/drafting.md, 「What it is」).
pub fn sent_now(text: &str) -> Option<&str> {
  trim(text).strip_prefix(SEND).map(trim)
}

/// What an answer of the draft process says of a message of its round (docs/drafting.md, 「What came in
/// meanwhile」): that it needs no answer, that it is answered in a round of its own, or that it is the one the
/// answer answers. A reply mark that names no message, or names it by zero or by a number too large to be one,
/// carries the number zero: the newest message the round answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
  Done,
  Separate,
  Reply,
}

static ROUND_MARK: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"<<(?:(DONE|SEPARATE|REPLY) #(-?[0-9]+)|REPLY:(-?[0-9]+)|(REPLY))>>").expect("a round mark")
});

fn marks_in(text: &str) -> Vec<(Mark, i64)> {
  ROUND_MARK
    .captures_iter(text)
    .filter_map(|found| {
      let (mark, id) = match (found.get(1), found.get(2), found.get(3)) {
        _ if found.get(4).is_some() => return Some((Mark::Reply, 0)),
        (Some(mark), Some(id), _) => {
          let mark = match mark.as_str() {
            "DONE" => Mark::Done,
            "SEPARATE" => Mark::Separate,
            _ => Mark::Reply,
          };

          (mark, id.as_str())
        }
        (_, _, Some(id)) => (Mark::Reply, id.as_str()),
        _ => return None,
      };

      match (mark, id.parse::<i64>()) {
        (Mark::Reply, Ok(id)) if id == 0 || id.unsigned_abs() > LARGEST_NAMED => Some((mark, 0)),
        (_, Ok(id)) => Some((mark, id)),
        (Mark::Reply, Err(_)) => Some((mark, 0)),
        (_, Err(_)) => None,
      }
    })
    .collect()
}

/// The marks an answer sets on messages, each with the message's number: those standing outside a code span or
/// fence, where they are not being quoted.
pub fn round_marks(text: &str) -> Vec<(Mark, i64)> {
  let mut quoted = marks_in(&without_control_markers(text));

  marks_in(text)
    .into_iter()
    .filter(|mark| match quoted.iter().position(|held| held == mark) {
      Some(at) => {
        quoted.remove(at);
        false
      }
      None => true,
    })
    .collect()
}

static ROUND_MARK_SPACED: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"[ \t]?<<(?:(?:DONE|SEPARATE|REPLY) #-?[0-9]+|REPLY(?::-?[0-9]+)?)>>").expect("a round mark")
});

/// An answer with every round mark taken out, so nothing after reads them as words.
pub fn without_round_marks(text: &str) -> String {
  trim(&ROUND_MARK_SPACED.replace_all(text, "")).to_owned()
}

//! The words a turn of the chat agent's session answers with when it is not answering the room: wait for more, or
//! say nothing this time; the one a turn of the draft process opens on to make its answer the message now; and the
//! marks it sets on a message that arrived while the process ran.

use std::sync::LazyLock;

use regex::Regex;

use crate::form::without_control_markers;
use crate::js::trim;

pub const WAIT: &str = "<<WAIT>>";
pub const SKIP: &str = "<<SKIP>>";
pub const SEND: &str = "<<SEND>>";

/// Whether an answer is nothing but waits, or nothing at all.
pub fn is_wait(text: &str) -> bool {
  trim(&text.replace(WAIT, "")).is_empty()
}

/// Whether an answer is nothing but skips, or nothing at all.
pub fn is_skip(text: &str) -> bool {
  trim(&text.replace(SKIP, "")).is_empty()
}

/// The words after the send marker, where an answer opens on it: the message the answer makes now
/// (docs/drafting.md, 「What it is」).
pub fn sent_now(text: &str) -> Option<&str> {
  trim(text).strip_prefix(SEND).map(trim)
}

/// What an answer of the draft process says of a message of its round (docs/drafting.md, 「What came in
/// meanwhile」): that it needs no answer, that it is answered in a round of its own, or that it is the one the
/// answer answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
  Done,
  Separate,
  Reply,
}

static ROUND_MARK: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"<<(?:(DONE|SEPARATE|REPLY) #(-?[0-9]+)|REPLY:(-?[0-9]+))>>").expect("a round mark")
});

fn marks_in(text: &str) -> Vec<(Mark, i64)> {
  ROUND_MARK
    .captures_iter(text)
    .filter_map(|found| {
      let (mark, id) = match (found.get(1), found.get(2), found.get(3)) {
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

      id.parse().ok().map(|id| (mark, id))
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
  Regex::new(r"[ \t]?<<(?:(?:DONE|SEPARATE|REPLY) #-?[0-9]+|REPLY:-?[0-9]+)>>").expect("a round mark")
});

/// An answer with every round mark taken out, so nothing after reads them as words.
pub fn without_round_marks(text: &str) -> String {
  trim(&ROUND_MARK_SPACED.replace_all(text, "")).to_owned()
}

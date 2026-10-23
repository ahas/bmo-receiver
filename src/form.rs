//! The words' form before any road's marks: what a text is cleaned of before it is read into a room's marks.
//! The scaffolding a session's own program writes and the markers it answers with are cut out, and a text left
//! with nothing is refused; the sanitiser reads what is left unless it comes already read and whole; and a dash
//! left between words is replaced by the net under the writing rules. Every way the receiver's words leave for a
//! room passes this one place, so none of them has to remember it.

use std::sync::LazyLock;

use receiver_ports::gate::{Gate, Sanitise, Sanitised};
use receiver_ports::http::PortFailure;

use crate::dashes::without_dashes;
use crate::js;
use regex::Regex;

/// Why a text has no form to go out in.
#[derive(Debug, thiserror::Error)]
pub enum NotFormed {
  #[error("refused to send prompt scaffolding to chat {0}")]
  Scaffolding(i64),
  #[error(transparent)]
  Gate(#[from] PortFailure),
}

// Where the scaffolding begins: whatever follows the first of these is no message.
static SCAFFOLDING: LazyLock<Vec<Regex>> = LazyLock::new(|| {
  [
    r"(?:^|\n)[ \t]*(?:END OF )?BMO ROOM CONTEXT(?:[ \t]|$)",
    r"(?:^|\n)[ \t]*(?:user[ \t]*\n?[ \t]*)?Chat id: -?[0-9]+[^\n]*\n[ \t]*Mode: ",
    r"(?:^|\n)[ \t]*Chat kind: (?:the owner's private chat|a user's own private chat|a group chat)",
    r"(?:^|\n)[ \t]*Run privilege: (?:PRIVILEGED|RESTRICTED)[ \t]*(?:\n|$)",
    r"(?:^|\n)[ \t]*Archived messages since your last reply(?: \([0-9]+\))?:[ \t]*(?:\n|$)",
    r"(?:^|\n)[ \t]*New messages(?: \([0-9]+\))?:[ \t]*\n[ \t]*\[[0-9]{4}-[0-9]{2}-[0-9]{2}T",
    r"(?:^|\n)[ \t]*Messages you waited on with <<WAIT>>, still unanswered(?: \([0-9]+\))?:[ \t]*(?:\n|$)",
    r"(?:^|\n)[ \t]*END OF (?:NEW|ARCHIVED|WAITED) MESSAGES[ \t]*(?:\n|$)",
    r"(?:^|\n)[ \t]*Reference for this message(?: \([0-9]+\))?:[ \t]*(?:\n|$)",
    r"(?:^|\n)[ \t]*END OF REFERENCE[ \t]*(?:\n|$)",
    r"(?:^|\n)[ \t]*Quiet instruction from the system(?: \([0-9]+\))?:[ \t]*(?:\n|$)",
    r"(?:^|\n)[ \t]*END OF QUIET INSTRUCTION[ \t]*(?:\n|$)",
    r"(?:^|\n)[ \t]*Documents you rely on were updated(?: \([0-9]+\))?:[ \t]*(?:\n|$)",
    r"(?:^|\n)[ \t]*END OF DOCUMENTS[ \t]*(?:\n|$)",
    r"(?:^|\n)[ \t]*\[[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}[^\]]*\][^\n]*\(id [0-9]+",
    r"(?:^|\n)[ \t]*No further messages arrived\.[ \t]*(?:\n|$)",
    r"(?:^|\n)[ \t]*The waiting window is over\. Give your final answer now",
  ]
  .iter()
  .map(|pattern| Regex::new(pattern).expect("a scaffolding pattern"))
  .collect()
});

// A context summary belongs to the program that carries a turn: where a reply opens with one, only what follows
// its closing separator is the message.
static SUMMARY_OPENING: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"(?m)^[ \t]*#{1,3}[ \t]*(?:Session[ \t]+)?Context Summary[ \t]*$")
    .expect("a summary's opening")
});
static SUMMARY_SEPARATOR: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"\n[ \t]*-{3,}[ \t]*\n").expect("a summary's separator"));

fn without_context_summary(text: &str) -> &str {
  let Some(opening) = SUMMARY_OPENING.find(text) else { return text };

  if js::units(&text[..opening.start()]) > 200 {
    return text;
  }

  SUMMARY_SEPARATOR
    .find_iter(text)
    .last()
    .map_or("", |separator| &text[separator.end()..])
}

/// A reply with the scaffolding cut off where it begins, and trimmed.
pub fn strip_prompt_scaffolding(input: &str) -> String {
  let text = without_context_summary(input);
  let cut = SCAFFOLDING
    .iter()
    .filter_map(|pattern| pattern.find(text).map(|found| found.start()))
    .min()
    .unwrap_or(text.len());

  js::trim(&text[..cut]).to_owned()
}

// Every marker a session writes for the service rather than for the room, with the one space before it.
static CONTROL_MARKER: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"[ \t]?<<(?:REPLY(?::[0-9]+)?|SKIP|WAIT|SEND|DONE #[0-9]+|SEPARATE #[0-9]+)>>")
    .expect("a control marker")
});
static FENCE_LINE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^ {0,3}(`{3,}|~{3,})").expect("a fence"));

fn backtick_run(line: &[char], at: usize) -> usize {
  line[at..].iter().take_while(|character| **character == '`').count()
}

// Where a run of exactly `length` backticks starts at or after `from`.
fn find_run(line: &[char], from: usize, length: usize) -> Option<usize> {
  let mut at = from;

  while at < line.len() {
    let next = line[at..].iter().position(|character| *character == '`').map(|at| at + from)?;
    let end = next + backtick_run(line, next);

    if end - next == length {
      return Some(next);
    }

    at = end;
  }

  None
}

fn unmarked(text: &[char]) -> String {
  CONTROL_MARKER.replace_all(&js::text(text), "").into_owned()
}

// The markers taken out of one line, every code span in it left as it stands.
fn outside_code_spans(line: &str) -> String {
  let line = js::chars(line);
  let mut kept = String::new();
  let mut from = 0;

  while from < line.len() {
    let Some(open) = line[from..].iter().position(|character| *character == '`').map(|at| from + at) else {
      break;
    };
    let run = open + backtick_run(&line, open);
    let ticks = run - open;

    match find_run(&line, run, ticks) {
      None => {
        kept += &unmarked(&line[from..run]);
        from = run;
      }
      Some(close) => {
        kept += &unmarked(&line[from..open]);
        kept += &js::text(&line[open..close + ticks]);
        from = close + ticks;
      }
    }
  }

  kept + &unmarked(&line[from.min(line.len())..])
}

/// A text with every control marker standing as text taken out; one in a code span or a code fence is being
/// quoted and stays.
pub fn without_control_markers(text: &str) -> String {
  let mut fence: Option<String> = None;
  let lines: Vec<String> = text
    .split('\n')
    .map(|line| {
      let found = FENCE_LINE.captures(line).map(|found| found[1].to_owned());

      if let Some(open) = &fence {
        if let Some(run) = &found
          && run.chars().next() == open.chars().next()
          && run.chars().count() >= open.chars().count()
          && js::trim(line) == run
        {
          fence = None;
        }

        return line.to_owned();
      }

      if found.is_some() {
        fence = found;

        return line.to_owned();
      }

      outside_code_spans(line)
    })
    .collect();

  js::trim(&lines.join("\n")).to_owned()
}

/// Which message an answer is pinned to: the one the turn read last where the reply sentinel names none, the
/// one it names, or none where the answer opens with no sentinel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplyTarget {
  Plain,
  Last,
  Named(i64),
}

// The reply sentinel an answer may open with, bare or naming a message.
static REPLY_SENTINEL: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^<<REPLY(?::([0-9]+))?>>").expect("the reply sentinel"));

// The largest number a message may be named by: a JavaScript number stays whole up to it.
const LARGEST_NAMED: u64 = (1 << 53) - 1;

/// An answer's words with the reply sentinel taken off, and the message it pins them to. The sentinel is the
/// session's instruction to the send and never words, so it is taken off once, before anything reads them. A
/// named number of zero, or one too large to be a message's, falls back to the message the turn read.
pub fn read_reply_request(written: &str) -> (String, ReplyTarget) {
  let text = js::trim(written);
  let Some(found) = REPLY_SENTINEL.captures(text) else {
    return (text.to_owned(), ReplyTarget::Plain);
  };
  let rest = js::trim(&text[found[0].len()..]).to_owned();
  let target = match found.get(1).map(|named| named.as_str().parse::<u64>()) {
    None => ReplyTarget::Last,
    Some(Ok(number)) if (1..=LARGEST_NAMED).contains(&number) => ReplyTarget::Named(number as i64),
    Some(_) => ReplyTarget::Last,
  };

  (rest, target)
}

/// What the sanitiser is told a message is, where the send names nothing else.
pub fn message_label() -> String {
  "a message the chat agent is about to send to a chat room".to_owned()
}

/// The boundary a text already read above the road is said to have been read at.
pub const READ_ABOVE: &str = "the send path above the road";

/// Words on their way out of a room: read by the sanitiser where the agent's own switch leaves it on, and their
/// dashes replaced.
pub async fn outgoing(
  gate: &dyn Gate,
  chat_id: i64,
  text: &str,
  label: &str,
  on: bool,
) -> Result<String, PortFailure> {
  let words = if !on {
    text.to_owned()
  } else {
    let asked = Sanitise {
      text: text.to_owned(),
      label: label.to_owned(),
      room_id: Some(chat_id),
      message_id: None,
    };

    match gate.sanitise(&asked).await {
      Ok(Sanitised { text, changed }) => {
        if changed {
          println!("send chat {chat_id}: security-intrusion material was defused");
        }

        text
      }
      Err(why) => return Err(why),
    }
  };

  Ok(without_dashes(&words, false))
}

/// A text in the words' form: scaffolding and markers cut, read by the sanitiser unless `read` says it came
/// already read and the cut left it whole, under `label` where it is no message, and its dashes replaced.
pub async fn cleaned(
  gate: &dyn Gate,
  chat_id: i64,
  text: &str,
  read: bool,
  label: Option<&str>,
) -> Result<String, NotFormed> {
  let stripped = without_control_markers(&strip_prompt_scaffolding(text));

  if stripped.is_empty() {
    return Err(NotFormed::Scaffolding(chat_id));
  }

  // A verdict is about the words as they were handed over; where the guard cut something out of them the
  // sanitiser reads what is left.
  let words = if read && stripped == text {
    stripped
  } else {
    let label = label.map_or_else(message_label, str::to_owned);

    outgoing(gate, chat_id, &stripped, &label, true).await?
  };

  Ok(without_dashes(&words, false))
}

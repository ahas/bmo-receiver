//! The words' form before any road's marks: what a text is cleaned of before it is read into a room's marks.
//! The scaffolding a session's own program writes and the markers it answers with are cut out, and a text left
//! with nothing is refused; and a dash left between words is replaced by the net under the writing rules. Every way
//! the receiver's words leave for a room passes this one place, so none of them has to remember it.

use std::sync::LazyLock;

use crate::dashes::without_dashes;
use crate::js;
use regex::Regex;

// Where the scaffolding begins: whatever follows the first of these is no message.
static SCAFFOLDING: LazyLock<Vec<Regex>> = LazyLock::new(|| {
  [
    r"(?:^|\n)[ \t]*(?:END OF )?BMO ROOM CONTEXT(?:[ \t]|$)",
    r"(?:^|\n)[ \t]*Chat -?[0-9]+[^\n]*· (?:the owner's private chat|a user's own private chat|a group chat)",
    r"(?:^|\n)[ \t]*Archived messages since your last reply(?: \([0-9]+\))?:[ \t]*(?:\n|$)",
    r"(?:^|\n)[ \t]*New messages(?: \([0-9]+\))?:[ \t]*\n[ \t]*\[[0-9]{2}-[0-9]{2} [0-9]{2}:[0-9]{2}",
    r"(?:^|\n)[ \t]*Messages you waited on with <<WAIT>>, still unanswered(?: \([0-9]+\))?:[ \t]*(?:\n|$)",
    r"(?:^|\n)[ \t]*END OF (?:NEW|ARCHIVED|WAITED) MESSAGES[ \t]*(?:\n|$)",
    r"(?:^|\n)[ \t]*Reference for this message(?: \([0-9]+\))?:[ \t]*(?:\n|$)",
    r"(?:^|\n)[ \t]*END OF REFERENCE[ \t]*(?:\n|$)",
    r"(?:^|\n)[ \t]*Quiet instruction from the system(?: \([0-9]+\))?:[ \t]*(?:\n|$)",
    r"(?:^|\n)[ \t]*END OF QUIET INSTRUCTION[ \t]*(?:\n|$)",
    r"(?:^|\n)[ \t]*Documents you rely on were updated(?: \([0-9]+\))?:[ \t]*(?:\n|$)",
    r"(?:^|\n)[ \t]*END OF DOCUMENTS[ \t]*(?:\n|$)",
    r"(?:^|\n)[ \t]*\[[0-9]{2}-[0-9]{2} [0-9]{2}:[0-9]{2}[^\]]*\][^\n]*#[0-9]+ ",
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

/// A reply with the scaffolding cut off where it begins and the markup of a tool call a program wrote as text
/// taken out, then trimmed.
pub fn strip_prompt_scaffolding(input: &str) -> String {
  let text = without_context_summary(input);
  let cut = SCAFFOLDING
    .iter()
    .filter_map(|pattern| pattern.find(text).map(|found| found.start()))
    .min()
    .unwrap_or(text.len());

  js::trim(&without_tool_calls(&text[..cut])).to_owned()
}

// The markup a program writes when it means a call it did not make: the wrapper some models write around a
// function and its parameters. Everything inside a fenced block is left as written, so a quotation of one stays.
static TOOL_CALL: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"(?is)<tool_call\b.*?(?:</tool_call\s*>|\z)|<\|tool_call\|>.*?(?:<\|/tool_call\|>|\z)")
    .expect("a tool call pattern")
});
static FENCED: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"(?s)```.*?(?:```|\z)").expect("a fenced block"));

fn without_tool_calls(text: &str) -> String {
  let mut out = String::new();
  let mut last = 0;

  for fence in FENCED.find_iter(text) {
    out.push_str(&TOOL_CALL.replace_all(&text[last..fence.start()], ""));
    out.push_str(fence.as_str());
    last = fence.end();
  }

  out.push_str(&TOOL_CALL.replace_all(&text[last..], ""));
  out
}

// Every marker a session writes for the service rather than for the room, with the one space before it.
static CONTROL_MARKER: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(
    r"[ \t]?<<(?:REPLY(?::-?[0-9]+| #-?[0-9]+)?|SKIP|WAIT|SEND|DONE #-?[0-9]+|SEPARATE #-?[0-9]+)>>",
  )
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
  LazyLock::new(|| Regex::new(r"^<<REPLY(?::(-?[0-9]+))?>>").expect("the reply sentinel"));

// The largest number a message may be named by, either side of zero: a JavaScript number stays whole up to it.
const LARGEST_NAMED: u64 = (1 << 53) - 1;

/// An answer's words with the reply sentinel taken off, and the message it pins them to. The sentinel is the
/// session's instruction to the send and never words, so it is taken off once, before anything reads them. A
/// message's number may be negative; a named number of zero, or one too large to be a message's either side of
/// zero, falls back to the message the turn read.
pub fn read_reply_request(written: &str) -> (String, ReplyTarget) {
  let text = js::trim(written);
  let Some(found) = REPLY_SENTINEL.captures(text) else {
    return (text.to_owned(), ReplyTarget::Plain);
  };
  let rest = js::trim(&text[found[0].len()..]).to_owned();
  let target = match found.get(1).map(|named| named.as_str().parse::<i64>()) {
    None => ReplyTarget::Last,
    Some(Ok(number)) if number != 0 && number.unsigned_abs() <= LARGEST_NAMED => ReplyTarget::Named(number),
    Some(_) => ReplyTarget::Last,
  };

  (rest, target)
}

/// Words on their way out of a room, their dashes replaced.
pub fn outgoing(text: &str) -> String {
  without_dashes(text, false)
}

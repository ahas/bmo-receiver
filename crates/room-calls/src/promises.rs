//! What an answer of the chat says, read by the code that holds it (docs/answer-checks.md): the answer's own
//! sentences with code and quotations set aside, and whether a text waits for something to happen first. The words
//! each reads by stand in the receiver's policy; the sentences are cut and the quotations set aside here.

use std::sync::LazyLock;

use regex::Regex;

use crate::js::{SPACE, chars, is_space};
use receiver_policy::answer_reading::ANSWER_READING;
use receiver_policy::scripts::SCRIPTS;

// A policy pattern as this side compiles it: the white space the machine's own class where the pattern says `\s`.
fn compiled(pattern: &str) -> Regex {
  Regex::new(&pattern.replace(r"\s", &format!("[{SPACE}]"))).expect("a reading pattern of the policy")
}

struct Reading {
  awaiting: Regex,
}

static READING: LazyLock<Reading> = LazyLock::new(|| {
  let reading = &ANSWER_READING.reading;

  Reading {
    awaiting: compiled(&reading.awaiting),
  }
});

// What in an answer is not its own words: code, and anything quoted. A single-quoted span is read apart, since
// it does not count where a syllable stands right before it.
static NOT_OWN: LazyLock<Vec<Regex>> = LazyLock::new(|| {
  [
    r"(?s)```.*?(?:```|\z)",
    r"`[^`\n]*`",
    r"「[^」]*」",
    r"『[^』]*』",
    r#""[^"\n]*""#,
    r"“[^”\n]*”",
    r"‘[^’\n]*’",
  ]
  .iter()
  .map(|pattern| Regex::new(pattern).expect("a quotation pattern"))
  .collect()
});

// Where a sentence of the answer ends: after one of these, or at white space after one of the two closings the
// answer's own sentences end on.
const SENTENCE_ENDS: [char; 5] = ['.', '!', '?', '。', '\n'];

fn syllable(character: char) -> Option<u32> {
  let span = SCRIPTS.syllables;
  let code = u32::from(character).checked_sub(span.first)?;

  (code < span.count()).then_some(code)
}

fn is_syllable(character: char) -> bool {
  syllable(character).is_some()
}

// Whether a sentence ends on this syllable where white space follows it.
fn closes_sentence(character: char) -> bool {
  ANSWER_READING.reading.closings.contains(character)
}

// Single-quoted spans set aside, but for one that stands right after a syllable.
fn without_single_quotes(text: &str) -> String {
  let characters = chars(text);
  let mut kept = String::new();
  let mut at = 0;

  while at < characters.len() {
    let opens = characters[at] == '\'' && (at == 0 || !is_syllable(characters[at - 1]));
    let closes = opens
      .then(|| {
        characters[at + 1..]
          .iter()
          .position(|character| *character == '\'' || *character == '\n')
          .filter(|found| characters[at + 1 + found] == '\'')
      })
      .flatten();

    match closes {
      Some(found) => {
        kept.push(' ');
        at += found + 2;
      }
      None => {
        kept.push(characters[at]);
        at += 1;
      }
    }
  }

  kept
}

/// An answer's own words, sentence by sentence, with code and quotations set aside.
pub fn own_sentences(text: &str) -> Vec<String> {
  let bare = NOT_OWN
    .iter()
    .fold(text.to_owned(), |rest, pattern| pattern.replace_all(&rest, " ").into_owned());
  let bare = without_single_quotes(&bare);
  let mut sentences = vec![];
  let mut sentence = String::new();
  let characters = chars(&bare);

  for (at, character) in characters.iter().copied().enumerate() {
    if is_space(character) && at > 0 && closes_sentence(characters[at - 1]) {
      sentences.push(std::mem::take(&mut sentence));
    } else {
      sentence.push(character);

      // A mark at the very end splits nothing off: there is nothing after it.
      if SENTENCE_ENDS.contains(&character) && at + 1 < characters.len() {
        sentences.push(std::mem::take(&mut sentence));
      }
    }
  }

  sentences.push(sentence);

  sentences
}

/// Whether a memory's action waits for something to happen first, whoever's volition it is said in: what a row
/// filed with nothing to wake it is refused for.
pub fn waits_on_happening(action: &str) -> bool {
  own_sentences(action).iter().any(|sentence| READING.awaiting.is_match(sentence))
}

//! What an answer of the chat says, as the checks it passes read it (docs/answer-checks.md): a promise about what
//! the agent will do from now on, a promise to act once something has happened, the owner's words setting a rule
//! for later, and an answer that came out in a language other than the turn's answer language. The words each
//! reads by stand in the receiver's policy; the sentences are cut and the quotations set aside here.

use std::sync::LazyLock;

use regex::Regex;

use crate::js::{SPACE, chars, is_space, trim};
use receiver_policy::scripts::{SCRIPTS, Span, holds_any};
use receiver_policy::second_asks::SECOND_ASKS;

// A range as a regular expression character class's body.
fn class_body(span: Span) -> String {
  format!(r"\x{{{:x}}}-\x{{{:x}}}", span.first, span.last)
}

// The modern Hangul syllables, as a character class's body.
static SYLLABLE: LazyLock<String> = LazyLock::new(|| class_body(SCRIPTS.syllables));

// How a syllable's number is made: its final consonant varies fastest, its vowel next.
const FINALS: u32 = 28;
const VOWEL_SPAN: u32 = 588;

// A policy pattern as this side compiles it: the white space the machine's own class where the pattern says `\s`.
fn compiled(pattern: &str) -> Regex {
  Regex::new(&pattern.replace(r"\s", &format!("[{SPACE}]"))).expect("a reading pattern of the policy")
}

// A policy pattern followed by no syllable: the next character, consumed, is none or no syllable. A match read
// for whether it is there, or for its first group, reads the same as a lookahead would.
fn followed_by_no_syllable(pattern: &str) -> Regex {
  compiled(&format!("(?:{pattern})(?:$|[^{}])", *SYLLABLE))
}

struct Reading {
  later: Regex,
  remember: Regex,
  awaiting: Regex,
  condition: Regex,
  manner: Regex,
  volition: Regex,
  pledge: Regex,
  told: Regex,
  told_closing: Regex,
  request: Regex,
}

static READING: LazyLock<Reading> = LazyLock::new(|| {
  let reading = &SECOND_ASKS.reading;

  Reading {
    later: compiled(&reading.later),
    remember: compiled(&reading.remember),
    awaiting: compiled(&reading.awaiting),
    condition: compiled(&reading.condition),
    manner: compiled(&reading.manner),
    volition: followed_by_no_syllable(&reading.volition),
    pledge: followed_by_no_syllable(&reading.pledge),
    told: followed_by_no_syllable(&reading.told),
    told_closing: compiled(&reading.told_closing),
    request: followed_by_no_syllable(&reading.request),
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
  SECOND_ASKS.reading.closings.contains(character)
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

// The first person's volition: a formal ending, or a syllable closed on the policy's final consonant before the
// pledge.
fn volunteers(sentence: &str) -> bool {
  let reading = &*READING;

  reading.volition.is_match(sentence)
    || reading.pledge.captures_iter(sentence).any(|found| {
      found[1]
        .chars()
        .next()
        .and_then(syllable)
        .is_some_and(|code| code % FINALS == SECOND_ASKS.reading.pledge_final)
    })
}

/// Whether an answer promises something about what the agent will do from now on; a promise about this turn's own
/// work carries no word of time to come and is not one, and what the answer quotes is not read.
pub fn promises_later(answer: &str) -> bool {
  let reading = &*READING;

  own_sentences(answer).iter().any(|sentence| {
    (reading.remember.is_match(sentence) || reading.later.is_match(sentence)) && volunteers(sentence)
  })
}

/// Whether an answer promises to act once something has happened.
pub fn promises_after(answer: &str) -> bool {
  own_sentences(answer)
    .iter()
    .any(|sentence| READING.awaiting.is_match(sentence) && volunteers(sentence))
}

/// Whether a memory's action waits for something to happen first, whoever's volition it is said in: what a row
/// filed with nothing to wake it is refused for.
pub fn waits_on_happening(action: &str) -> bool {
  own_sentences(action).iter().any(|sentence| READING.awaiting.is_match(sentence))
}

// Whether this syllable is the one an imperative closes on: one of the policy's vowels, with no final consonant.
fn ends_imperative(character: char) -> bool {
  syllable(character).is_some_and(|code| {
    code % FINALS == 0 && SECOND_ASKS.reading.imperative_vowels.contains(&((code % VOWEL_SPAN) / FINALS))
  })
}

static LAST_SYLLABLE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(&format!(r"([{}])[.!。]?$", *SYLLABLE)).expect("a closing syllable"));

// Somebody told to do something: a request made of them, a plain imperative, or a standing obligation.
pub fn orders(sentence: &str) -> bool {
  let said = trim(sentence);
  let reading = &*READING;

  // A question asks rather than tells, whatever its verb looks like.
  if said.is_empty() || said.ends_with(['?', '？']) {
    return false;
  }

  if reading.told.is_match(said) || reading.told_closing.is_match(said) {
    return true;
  }

  let first_char = |found: regex::Captures<'_>| found[1].chars().next();

  if reading.request.captures(said).and_then(first_char).is_some_and(ends_imperative) {
    return true;
  }

  LAST_SYLLABLE.captures(said).and_then(first_char).is_some_and(ends_imperative)
}

/// Whether these words set what the agent is to do from a situation on: an order with the reach of a rule, a word
/// of time to come or a condition pointed at a manner.
pub fn sets_rule_later(said: &str) -> bool {
  let reading = &*READING;

  own_sentences(said).iter().any(|sentence| {
    orders(sentence)
      && (reading.later.is_match(sentence)
        || (reading.condition.is_match(sentence) && reading.manner.is_match(sentence)))
  })
}

// What an answer carries that is no language of anybody's: a code block, an inline code span, an address, a path.
static NOT_PROSE: LazyLock<Vec<Regex>> = LazyLock::new(|| {
  [
    r"(?s)```.*?(?:```|\z)".to_owned(),
    r"`[^`\n]*`".to_owned(),
    format!(r"(?-u:\b)https?://[^{SPACE}]+"),
    r"(?:~|\.{1,2})?(?:/[A-Za-z0-9_.@+-]+){2,}/?".to_owned(),
  ]
  .iter()
  .map(|pattern| Regex::new(pattern).expect("a pattern of what is not prose"))
  .collect()
});

static ENGLISH_WORD: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^[A-Za-z]+(?:['’][A-Za-z]+)?$").expect("an English word"));

/// Whether a piece of text holds any Korean at all.
pub fn holds_korean(text: &str) -> bool {
  text.chars().any(is_syllable)
}

// Whether a character is kana.
fn is_kana(character: char) -> bool {
  holds_any(&SCRIPTS.kana_letters, character)
}

// The language a word of an answer's prose is in, where it can be told: Korean where it holds a Hangul syllable,
// Japanese where it holds kana, English where it is made of Latin letters alone.
fn word_language(word: &str) -> Option<&'static str> {
  if holds_korean(word) {
    Some("ko")
  } else if word.chars().any(is_kana) {
    Some("ja")
  } else if ENGLISH_WORD.is_match(word) {
    Some("en")
  } else {
    None
  }
}

/// Whether an answer came out in none of the languages it is held to, by its prose: enough words of another
/// language that can be told, and not one word of any of those languages beside them.
pub fn answered_in_none_of(answer: &str, languages: &[&str]) -> bool {
  let prose = NOT_PROSE
    .iter()
    .fold(answer.to_owned(), |rest, pattern| pattern.replace_all(&rest, " ").into_owned());
  let letter =
    |character: char| character.is_ascii_alphabetic() || is_syllable(character) || is_kana(character);
  let told: Vec<&str> = prose
    .split(is_space)
    .filter_map(|word| word_language(word.trim_matches(|character: char| !letter(character))))
    .collect();
  let held = told.iter().filter(|word| languages.contains(word)).count();
  let other = told.len() - held;

  held == 0 && other >= SECOND_ASKS.language.judged_from
}

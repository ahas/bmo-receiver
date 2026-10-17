//! The words the owner banned, read out of the deployment's wording document and found in a text: the net every
//! writer of the system's own words is held to, outside the spans a text quotes.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::js::units;

/// A span a written text names a wrong form or quotes a command or somebody's words in: inside backticks or a
/// pair of quotation marks, on one line.
pub static QUOTED_SPAN: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r#"`[^`\n]*`|"[^"\n]*"|'[^'\n]*'|“[^”\n]*”|‘[^’\n]*’|「[^」\n]*」|『[^』\n]*』"#)
    .expect("a quoted span")
});

/// A word the owner banned, with what to write instead, when the entry applies where its line says so, and the
/// phrases in which it keeps its own sense and passes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BannedTerm {
  pub lang: String,
  pub term: String,
  pub instead: String,
  pub when: Option<String>,
  pub keeps: Vec<String>,
}

/// A use of a banned term the document allows in one file, on the line holding the excerpt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AllowedUse {
  pub lang: String,
  pub term: String,
  pub path: String,
  pub excerpt: String,
}

/// The document's lists: the banned terms and the uses it allows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Glossary {
  pub banned: Vec<BannedTerm>,
  pub allowed: Vec<AllowedUse>,
}

/// The document the banned words are read out of.
pub const WORDING_DOCUMENT: &str = "WORDING.md";

static BANNED_LINE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^- ([a-z]{2,3}) · (.+?) → (.+?)(?: — (.*))?$").unwrap());
static ALLOWED_LINE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^- ([a-z]{2,3}) · (.+?) · (\S+) · (.+) — .+$").unwrap());
static OWN_SENSE_LINE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^- ([a-z]{2,3}) · (.+?) · (.+?) — .+$").unwrap());

// The languages a line can be told to be in, by the script it is written in.
const KNOWN_LANGUAGES: [&str; 3] = ["ko", "ja", "en"];

/// The language a line is written in, told by its script: Korean where it holds Hangul, Japanese where it holds
/// kana alone, English otherwise.
pub fn line_language(line: &str) -> &'static str {
  let korean =
    |c: char| matches!(c, '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}' | '\u{AC00}'..='\u{D7AF}');
  let japanese = |c: char| matches!(c, '\u{3040}'..='\u{30FF}');

  if line.chars().any(korean) {
    "ko"
  } else if line.chars().any(japanese) {
    "ja"
  } else {
    "en"
  }
}

/// The banned terms as the document lists them. A line naming a language no line can be told in, or a phrase of
/// a word's own sense that names no banned term or does not hold it, is a fault in the document.
pub fn parse_glossary(text: &str) -> Result<Vec<BannedTerm>, String> {
  parse_wording(text).map(|glossary| glossary.banned)
}

/// The document's lists out of its own sections, held to the rules `parse_glossary` holds them to.
pub fn parse_wording(text: &str) -> Result<Glossary, String> {
  let mut banned: Vec<BannedTerm> = Vec::new();
  let mut allowed: Vec<AllowedUse> = Vec::new();
  let mut section = "";
  let checked = |lang: &str, line: &str| -> Result<String, String> {
    if KNOWN_LANGUAGES.contains(&lang) {
      Ok(lang.to_owned())
    } else {
      Err(format!("{WORDING_DOCUMENT}: unknown language code \"{lang}\" in: {line}"))
    }
  };

  for line in text.split('\n') {
    if let Some(title) = line.strip_prefix("## ") {
      section = title.trim();

      continue;
    }

    if section == "Banned terms" {
      if let Some(found) = BANNED_LINE.captures(line) {
        banned.push(BannedTerm {
          lang: checked(&found[1], line)?,
          term: found[2].to_owned(),
          instead: found[3].to_owned(),
          when: found.get(4).map(|when| when.as_str().to_owned()),
          keeps: Vec::new(),
        });
      }
    } else if section == "Allowed uses" {
      if let Some(found) = ALLOWED_LINE.captures(line) {
        allowed.push(AllowedUse {
          lang: checked(&found[1], line)?,
          term: found[2].to_owned(),
          path: found[3].to_owned(),
          excerpt: found[4].to_owned(),
        });
      }
    } else if section == "Uses in the word's own sense"
      && let Some(found) = OWN_SENSE_LINE.captures(line)
    {
      let lang = checked(&found[1], line)?;
      let (term, phrase) = (&found[2], &found[3]);
      let entry = banned.iter_mut().find(|one| one.lang == lang && one.term == term);

      match entry {
        Some(entry) if phrase.contains(term) => entry.keeps.push(phrase.to_owned()),
        _ => {
          return Err(format!(
            "{WORDING_DOCUMENT}: a phrase of a word's own sense must hold a banned term listed above it: {line}"
          ));
        }
      }
    }
  }

  Ok(Glossary { banned, allowed })
}

/// The banned terms as the document at a root lists them, or none where it cannot be read: an answer is not held
/// to a document nobody can read.
pub fn read_glossary(source: &Path) -> Vec<BannedTerm> {
  std::fs::read_to_string(source.join(WORDING_DOCUMENT))
    .ok()
    .and_then(|text| parse_glossary(&text).ok())
    .unwrap_or_default()
}

fn hangul_syllable(c: char) -> bool {
  matches!(c, '\u{AC00}'..='\u{D7A3}')
}

// Whether the term stands on its own where it was found, rather than being the tail of a longer word: a Korean
// term another syllable runs into from the left belongs to that other word.
fn stands_alone(line: &str, at: usize, term: &BannedTerm) -> bool {
  if at == 0 || !term.term.chars().next().is_some_and(hangul_syllable) {
    return true;
  }

  !line[..at].chars().next_back().is_some_and(hangul_syllable)
}

// Whether the term, where it was found, is part of a phrase in which it keeps its own sense.
fn keeps_own_sense(line: &str, at: usize, term: &BannedTerm) -> bool {
  term.keeps.iter().any(|phrase| {
    phrase
      .find(&term.term)
      .and_then(|inside| at.checked_sub(inside))
      .is_some_and(|start| line.get(start..).is_some_and(|rest| rest.starts_with(phrase.as_str())))
  })
}

// Whether a line of the term's own language uses it anywhere it neither runs into a longer word nor keeps its
// own sense.
fn used_in(line: &str, term: &BannedTerm) -> bool {
  let mut from = 0;

  while let Some(found) = line[from..].find(&term.term) {
    let at = from + found;

    if stands_alone(line, at, term) && !keeps_own_sense(line, at, term) {
      return true;
    }

    from = at + line[at..].chars().next().map_or(1, char::len_utf8);
  }

  false
}

/// The banned terms a text uses outside its quoted spans, each once and in the glossary's order.
pub fn banned_terms_in(text: &str, glossary: &[BannedTerm]) -> Vec<BannedTerm> {
  let bare = QUOTED_SPAN.replace_all(text, |found: &regex::Captures| " ".repeat(units(&found[0])));
  let lines: Vec<(&str, &str)> = bare.split('\n').map(|line| (line_language(line), line)).collect();

  glossary
    .iter()
    .filter(|term| lines.iter().any(|(lang, line)| *lang == term.lang && used_in(line, term)))
    .cloned()
    .collect()
}

/// The terms in one line, each beside what to write: `"<term>" → <what to write>; …`.
pub fn banned_terms_line(terms: &[BannedTerm]) -> String {
  terms
    .iter()
    .map(|term| format!("\"{}\" → {}", term.term, term.instead))
    .collect::<Vec<_>>()
    .join("; ")
}

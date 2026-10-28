//! The language a text is written in, told by the script of its prose outside the spans it quotes: what the
//! prompt's answer-language note reads (docs/answer-checks.md, 「The language a text is written in」).

use std::sync::LazyLock;

use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::scripts::{SCRIPTS, holds_any};
use regex::Regex;

// A span a written text names a wrong form or quotes a command or somebody's words in: inside backticks or a
// pair of quotation marks, on one line.
static QUOTED_SPAN: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r#"`[^`\n]*`|"[^"\n]*"|'[^'\n]*'|“[^”\n]*”|‘[^’\n]*’|「[^」\n]*」|『[^』\n]*』"#)
    .expect("a quoted span")
});

/// The language a text is written in, told by the letters of its prose outside its quoted spans: Hangul and kana
/// each weigh twice, standing for a syllable, and Latin letters once. None where no such letter stands or two
/// languages weigh the same.
pub fn text_language(text: &str) -> Option<&'static str> {
  let bare = QUOTED_SPAN.replace_all(text, " ");
  let mut weights = [("ko", 0usize), ("ja", 0usize), ("en", 0usize)];

  for character in bare.chars() {
    if holds_any(&SCRIPTS.hangul_letters, character) {
      weights[0].1 += 2;
    } else if holds_any(&SCRIPTS.kana_letters, character) {
      weights[1].1 += 2;
    } else if character.is_ascii_alphabetic() {
      weights[2].1 += 1;
    }
  }

  let heaviest = weights.iter().map(|(_, weight)| *weight).max().unwrap_or(0);
  let mut leading = weights.iter().filter(|(_, weight)| *weight == heaviest);

  match (heaviest, leading.next(), leading.next()) {
    (0, _, _) | (_, _, Some(_)) => None,
    (_, found, None) => found.map(|(language, _)| *language),
  }
}

// What a message holds that is no prose of its writer's: a code block, an address, a path, and a bracketed span,
// the placeholder a chat service writes for a file, a picture or a sticker it carried.
static NOT_PROSE: LazyLock<Vec<Regex>> = LazyLock::new(|| {
  [
    r"(?s)```.*?(?:```|\z)",
    r"https?://\S+",
    r"(?:~|\.{1,2})?(?:/[A-Za-z0-9_.@+-]+){2,}/?",
    r"\[[^\]\n]*\]",
  ]
  .iter()
  .map(|pattern| Regex::new(pattern).expect("a pattern of what is not prose"))
  .collect()
});

/// The language a person's message names as theirs (docs/answer-checks.md, 「The language an answer goes out
/// in」): the language of its prose, once its quoted spans, code, addresses, paths and bracketed placeholders are
/// set aside, where that prose still holds the policy's least letters; none for a message with no prose — a
/// file, a bare link, code, numbers or an emoji — which never changes its writer's language.
pub fn prose_language(text: &str) -> Option<&'static str> {
  let bare = NOT_PROSE
    .iter()
    .fold(text.to_owned(), |rest, pattern| pattern.replace_all(&rest, " ").into_owned());
  let prose = QUOTED_SPAN.replace_all(&bare, " ");
  let letters = prose
    .chars()
    .filter(|character| {
      character.is_ascii_alphabetic()
        || holds_any(&SCRIPTS.hangul_letters, *character)
        || holds_any(&SCRIPTS.kana_letters, *character)
    })
    .count();

  (letters >= CHAT_ROUND.language.prose_letters).then(|| text_language(&prose)).flatten()
}

//! The language a text is written in, told by the script of its prose outside the spans it quotes: what the
//! answer language check and the prompt's answer-language note read (docs/answer-checks.md, 「The language a
//! text is written in」).

use std::sync::LazyLock;

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

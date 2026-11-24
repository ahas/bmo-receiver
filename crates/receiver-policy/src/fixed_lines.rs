//! The plain sentence each fixed line falls back to where the web unit's language packs cannot render it
//! (docs/voiced-notices.md, 「Lines that stay fixed」): by the rooms' language, then the line's code.

use std::collections::BTreeMap;

use serde::Deserialize;

/// The sentences a line takes in place of its code's by the value of one of its fields: by the field's name, then
/// its value.
pub type Choices = BTreeMap<String, BTreeMap<String, String>>;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixedLines {
  /// Each language's sentences, by the line's code, `{name}` marking a field.
  pub sentences: BTreeMap<String, BTreeMap<String, String>>,
  /// Each language's sentences chosen by a field's value, by the line's code; a line whose field holds no value
  /// listed takes its code's sentence.
  #[serde(default)]
  pub choices: BTreeMap<String, BTreeMap<String, Choices>>,
}

impl FixedLines {
  /// The sentence a code falls back to in a language, or none where the policy holds none.
  pub fn sentence(&self, language: &str, code: &str) -> Option<&str> {
    self.sentences.get(language)?.get(code).map(String::as_str)
  }

  /// The sentence a line falls back to in a language: the one chosen by the value `value_of` gives for one of the
  /// fields the policy chooses by, where it keeps one for that value, else the code's own.
  pub fn chosen<'a>(
    &'a self,
    language: &str,
    code: &str,
    value_of: impl Fn(&str) -> Option<String>,
  ) -> Option<&'a str> {
    let chosen = self.choices.get(language).and_then(|codes| codes.get(code)).and_then(|fields| {
      fields
        .iter()
        .find_map(|(field, values)| value_of(field).and_then(|value| values.get(&value)))
    });

    chosen.map(String::as_str).or_else(|| self.sentence(language, code))
  }
}

crate::part!(pub FIXED_LINES: FixedLines = "fixed-lines.toml");

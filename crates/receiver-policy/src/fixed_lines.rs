//! The plain sentence each fixed line falls back to where the web unit's language packs cannot render it
//! (docs/voiced-notices.md, 「Lines that stay fixed」): by the rooms' language, then the line's code.

use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixedLines {
  /// Each language's sentences, by the line's code, `{name}` marking a field.
  pub sentences: BTreeMap<String, BTreeMap<String, String>>,
}

impl FixedLines {
  /// The sentence a code falls back to in a language, or none where the policy holds none.
  pub fn sentence(&self, language: &str, code: &str) -> Option<&str> {
    self.sentences.get(language)?.get(code).map(String::as_str)
  }
}

crate::part!(pub FIXED_LINES: FixedLines = "fixed-lines.toml");

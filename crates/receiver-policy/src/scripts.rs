//! The scripts and widths the receiver tells characters by (docs/policy.md): the Hangul and kana blocks, the
//! modern Hangul syllables, and the ranges a label's width counts as a whole em.

use serde::Deserialize;

/// A range of Unicode code points, first and last inclusive.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Span {
  pub first: u32,
  pub last: u32,
}

impl Span {
  /// How many code points the range holds.
  pub fn count(self) -> u32 {
    self.last - self.first + 1
  }

  /// Whether `character` stands in the range.
  pub fn holds(self, character: char) -> bool {
    (self.first..=self.last).contains(&u32::from(character))
  }
}

/// Whether `character` stands in any of `ranges`.
pub fn holds_any(ranges: &[Span], character: char) -> bool {
  ranges.iter().any(|range| range.holds(character))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scripts {
  pub hangul_letters: Vec<Span>,
  pub syllables: Span,
  pub kana_letters: Vec<Span>,
  pub wide_letters: Vec<Span>,
}

crate::part!(pub SCRIPTS: Scripts = "scripts.toml");

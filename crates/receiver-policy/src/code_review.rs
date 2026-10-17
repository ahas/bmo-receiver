//! The talk under a code review, a turn of the owner's chat with the chat agent (docs/policy.md): the place a
//! review is pinned to and the lines a turn's head is written in.

use serde::Deserialize;

/// The place a review is pinned to.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Places {
  pub whole: String,
  pub line: String,
  pub range: String,
  pub file: String,
  pub folder: String,
}

/// A turn's head.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Talk {
  pub words: String,
  pub head: Vec<String>,
  pub lines: String,
  pub folder_reading: String,
  pub file_reading: String,
  pub excerpt: String,
  pub excerpt_end: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeReview {
  pub excerpt_clip: usize,
  pub places: Places,
  pub talk: Talk,
}

crate::part!(pub CODE_REVIEW: CodeReview = "code-review.toml");

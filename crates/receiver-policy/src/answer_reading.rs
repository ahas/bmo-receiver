//! The words an answer's own sentences are read by (docs/answer-checks.md, 「Where the readings stand」): the
//! syllables a sentence ends on and the words of something that has to happen first.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reading {
  pub closings: String,
  pub awaiting: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Language {
  pub judged_from: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnswerReading {
  pub reading: Reading,
  /// Language thresholds a deployment set before the send path stopped reading language: read and ignored.
  #[serde(default)]
  pub language: Option<Language>,
}

crate::part!(pub ANSWER_READING: AnswerReading = "answer-reading.toml");

//! The checks a chat answer passes before it goes out, and the errands an item or a check of the draft process
//! sends the same session (docs/answer-checks.md, docs/drafting.md): the words each check reads by, and every
//! line it says.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Errands {
  pub owed: String,
  /// A language errand a deployment set before the send path stopped reading language: read and ignored.
  #[serde(default)]
  pub language: Option<String>,
  /// A length errand a deployment set before the length bound was removed: read and ignored.
  #[serde(default)]
  pub length: Option<String>,
  pub format: String,
  pub record: String,
  pub record_off: String,
  pub record_on: String,
  pub wake: String,
  pub wake_off: String,
  pub wake_on: String,
  pub rule: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reading {
  pub closings: String,
  pub later: String,
  pub remember: String,
  pub awaiting: String,
  pub condition: String,
  pub manner: String,
  pub volition: String,
  pub pledge: String,
  pub pledge_final: u32,
  pub told: String,
  pub told_closing: String,
  pub request: String,
  pub imperative_vowels: Vec<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Language {
  pub judged_from: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecondAsks {
  pub next_turn: String,
  pub later: String,
  pub errands: Errands,
  pub reading: Reading,
  /// Language thresholds a deployment set before the send path stopped reading language: read and ignored.
  #[serde(default)]
  pub language: Option<Language>,
}

crate::part!(pub SECOND_ASKS: SecondAsks = "second-asks.toml");

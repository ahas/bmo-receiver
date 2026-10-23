//! The checks a chat answer passes before it goes out, and the errands an item or a check of the draft process
//! sends the same session (docs/answer-checks.md, docs/drafting.md): the words each check reads by, and every
//! line it says.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Errands {
  pub owed: String,
  pub language: String,
  pub answer: String,
  pub wording: String,
  pub length: String,
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
  pub korean_share: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecondAsks {
  pub next_turn: String,
  pub later: String,
  /// The languages an answer is held to the wording document in, as a text's language is told.
  pub wording_languages: Vec<String>,
  pub errands: Errands,
  pub reading: Reading,
  pub language: Language,
}

crate::part!(pub SECOND_ASKS: SecondAsks = "second-asks.toml");

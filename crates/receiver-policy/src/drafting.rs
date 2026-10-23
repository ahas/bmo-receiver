//! The draft process (docs/drafting.md): the questions a round's turn is followed by, the default rows of the
//! table that says which, the lines a question turn is written in and the longest final text.

use std::collections::BTreeMap;

use serde::Deserialize;

/// A default row of the process table: the key it is matched by, each part a name or `*`, and the process.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Process {
  pub provider: String,
  pub tier: String,
  pub agent: String,
  pub rounds: Vec<String>,
  pub light: Vec<String>,
  pub light_chars: u32,
  pub ceiling_seconds: u32,
  pub past_ceiling: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Drafting {
  /// The longest final text the length check lets go, in characters.
  pub max_chars: usize,
  pub lead: String,
  pub last_lead: String,
  pub waiting: String,
  /// The questions, by name.
  pub questions: BTreeMap<String, String>,
  pub process: Vec<Process>,
}

crate::part!(pub DRAFTING: Drafting = "drafting.toml");

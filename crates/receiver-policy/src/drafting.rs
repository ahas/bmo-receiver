//! The draft process (docs/drafting.md): the questions a round's session is asked, the default rows of the table
//! that says which, and the lines a question turn is written in.

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
  /// The question of `rounds` from whose answer on a send is read; empty for the first.
  pub send_from: String,
  pub light: Vec<String>,
  pub light_chars: u32,
  /// A time ceiling a row set before rounds were left without one: read and ignored.
  #[serde(default)]
  pub ceiling_seconds: Option<u32>,
  /// What a row set to do past that ceiling: read and ignored.
  #[serde(default)]
  pub past_ceiling: Option<String>,
  /// A turn's time bound a row set before the bound was removed: read and ignored.
  #[serde(default)]
  pub turn_seconds: Option<u32>,
}

/// The review the gate asks before a message goes out.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
  pub head: String,
  pub draft: String,
  pub words: String,
  pub notes: String,
  pub rules: String,
  pub pass: String,
  pub reject: String,
  pub revise: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Drafting {
  /// A length bound a deployment set before the bound was removed: read and ignored.
  #[serde(default)]
  pub max_chars: Option<usize>,
  /// A turn's time bound a deployment set before the bound was removed: read and ignored.
  #[serde(default)]
  pub turn_seconds: Option<u32>,
  /// The opening of the first question, which rides the round's own turn where it is not also the last.
  pub opening: String,
  pub lead: String,
  pub last_lead: String,
  /// What the opening and the lead say of the send marker: that a send is read at this question, or from which
  /// question on it is.
  pub send_open: String,
  pub send_closed: String,
  pub waiting: String,
  /// The closing check's opening, asked once the message stands where messages arrived after the last question.
  pub arrived: String,
  /// The head the closing check shows the message as it stands under.
  pub standing: String,
  /// What a question whose answer may be the message says of its voice: the room's, never notes.
  pub voice: String,
  /// The review the gate asks before a message goes out.
  pub review: Review,
  /// The questions, by name.
  pub questions: BTreeMap<String, String>,
  pub process: Vec<Process>,
}

impl Drafting {
  /// One process for every agent: where a deployment still carries more than one row, the row with the most
  /// questions is the one and it answers for any provider, tier and agent.
  pub fn normalize(&mut self) {
    let Some(row) = self.process.iter().max_by_key(|row| row.rounds.len()).cloned() else {
      return;
    };

    let mut row = row;

    row.provider = "*".to_owned();
    row.tier = "*".to_owned();
    row.agent = "*".to_owned();
    self.process = vec![row];
  }
}

pub static DRAFTING: std::sync::LazyLock<Drafting> = std::sync::LazyLock::new(|| {
  let mut drafting: Drafting = crate::read(
    "drafting.toml",
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../policy/drafting.toml")),
  );

  drafting.normalize();

  drafting
});

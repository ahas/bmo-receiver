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
  /// The question a row once read a send from: read and ignored, since the draft turn's answer is the message.
  #[serde(default)]
  pub send_from: Option<String>,
  /// The questions of the list that are switched off: each keeps its place and is not asked.
  #[serde(default)]
  pub off: Vec<String>,
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
  /// The marker that names the review round, with no instruction.
  pub marker: String,
  /// What the review is asked, said once in the room's block; `{pass}` and `{reject}` are the verdicts below.
  pub instruction: String,
  /// What a revision round does, said once in the room's block.
  pub revise: String,
  pub draft: String,
  pub words: String,
  pub rules: String,
  /// The label the review turn shows the questions the draft turn followed under, `{names}` as its line carried
  /// them: data, with no instruction.
  pub followed: String,
  pub handed: String,
  pub room_handed: String,
  pub pass: String,
  pub reject: String,
  /// The label a revision turn carries its reason under: data, with no instruction.
  pub reason: String,
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
  /// The line the draft turn closes on, `{names}` the questions its round follows, in order and joined by `+`.
  pub round: String,
  /// The line a round turn is asked once more with where it skipped naming no reason: the form a skip goes in.
  pub skip_again: String,
  /// The marker that names the closing check's turn.
  pub arrived_marker: String,
  /// What the closing check does, said once in the room's block.
  pub arrived: String,
  /// The head the closing check shows the message as it stands under.
  pub standing: String,
  /// The voice the message is written in: the persona's, never notes.
  pub voice: String,
  /// The head the room's block carries over the stage instructions, which the session is handed as its system
  /// prompt; `{voice}` is `voice`.
  pub stages_head: String,
  /// One stage instruction in the room's block, `{name}` the question and `{text}` its catalogue text.
  pub stage: String,
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

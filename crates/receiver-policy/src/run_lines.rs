//! The lines a turn runs on, as far as the receiver reads them (docs/bmo-window.md, 「A line typed into the
//! program」).

use std::collections::BTreeMap;

use serde::Deserialize;

/// What one line declares.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunLine {
  /// Whether its turns follow one another on one session.
  pub keeps_session: bool,
  /// Whether a line a person types into its standing program is handed the composition a turn would carry.
  #[serde(default)]
  pub composes_typed_lines: bool,
}

/// Every line, by its name.
#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct RunLines(pub BTreeMap<String, RunLine>);

impl RunLines {
  pub fn line(&self, name: &str) -> Option<&RunLine> {
    self.0.get(name)
  }

  /// Whether a person's typed line in the line's standing program is composed.
  pub fn composes_typed_lines(&self, name: &str) -> bool {
    self.line(name).is_some_and(|line| line.composes_typed_lines)
  }
}

crate::part!(pub RUN_LINES: RunLines = "run-lines.toml");

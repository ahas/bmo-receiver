//! What `quiet` counts as work under way that a restart would cut (docs/boundary-api.md, 「The service and its
//! commands」).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quiet {
  /// The words a run's name carries for work that counts however long it has run.
  pub long_words: Vec<String>,
  /// The words a chat turn's run carries, which counts however long it has run.
  pub turn_words: Vec<String>,
}

crate::part!(pub QUIET: Quiet = "quiet.toml");

//! What `quiet` counts as work under way that a restart would cut (docs/boundary-api.md, 「The service and its
//! commands」).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quiet {
  /// The words a run's name carries for work that counts however long it has run.
  pub long_words: Vec<String>,
  /// The words a chat turn's run carries.
  pub turn_words: Vec<String>,
  /// The span within which an open chat turn's run counts, in minutes.
  pub recent_minutes: i64,
}

crate::part!(pub QUIET: Quiet = "quiet.toml");

//! The rest as the receiver lifts it (docs/receiver.md): when a login rests, and until when, is the agency's.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rest {
  /// How often a rest past its end is looked for and lifted, in seconds.
  pub lift_check_seconds: u64,
}

crate::part!(pub REST: Rest = "rest.toml");

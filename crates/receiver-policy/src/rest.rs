//! The rest a usage limit calls for, as the receiver closes and opens its rooms by it (docs/receiver.md).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rest {
  /// The share of the five-hour window past which a login rests.
  pub threshold: f64,
  /// The share of the weekly window past which a login rests.
  pub weekly_threshold: f64,
  /// How long a login is put aside where its limit named no reset moment, in seconds.
  pub fallback_seconds: i64,
  /// How long a rest lasts when the provider itself is down, in seconds.
  pub outage_seconds: i64,
  /// How often a rest past its end is looked for and lifted, in seconds.
  pub lift_check_seconds: u64,
}

crate::part!(pub REST: Rest = "rest.toml");

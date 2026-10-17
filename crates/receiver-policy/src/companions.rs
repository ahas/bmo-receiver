//! A companion added on the console with a room of its own (docs/boundary-api.md, 「Companions with a room of
//! their own」): what it may be named and the mark a screen puts beside it.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompanionRules {
  /// The pattern a companion added on the console is named by.
  pub name_pattern: String,
  /// The mark a screen puts beside a companion added on the console.
  pub added_icon: String,
}

crate::part!(pub COMPANIONS: CompanionRules = "companions.toml");

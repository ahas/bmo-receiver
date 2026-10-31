//! The secret shapes (docs/messages.md, 「Secrets taken out」): the marker a secret-shaped run is replaced by,
//! and the shapes a message's words are read for, in the order they are read.

use serde::Deserialize;

/// One shape: its name, and the regular expression its runs are found by.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecretShape {
  pub name: String,
  /// Where it names a part `secret`, that part alone is the run replaced.
  pub pattern: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecretShapes {
  pub marker: String,
  pub shapes: Vec<SecretShape>,
}

crate::part!(pub SECRET_SHAPES: SecretShapes = "secret-shapes.toml");

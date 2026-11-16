//! The files the console attaches and an agent sends.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileNames {
  /// The name a file goes by where the browser sent it with none.
  pub unnamed: String,
}

crate::part!(pub FILES: FileNames = "files.toml");

//! The files the console attaches and an agent sends.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileLimits {
  /// The largest one file may be, in bytes.
  pub largest_bytes: u64,
  /// The name a file goes by where the browser sent it with none.
  pub unnamed: String,
}

crate::part!(pub FILES: FileLimits = "files.toml");

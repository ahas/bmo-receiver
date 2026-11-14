//! How the files the process holds open are watched against its ceiling (docs/receiver.md, 「The open-file
//! ceiling」).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenFiles {
  /// How often the open files are counted, in seconds.
  pub check_seconds: u64,
  /// The share of the ceiling, in percent, at which the count stands near it.
  pub near_percent: u64,
}

crate::part!(pub OPEN_FILES: OpenFiles = "open-files.toml");

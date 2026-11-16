//! The files the console attaches and an agent sends.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileLimits {
  /// The largest one file may be, in bytes.
  pub largest_bytes: u64,
  /// The most files one send may carry.
  pub most_files: u64,
  /// The bytes a form's fields and its framing may take beside its files.
  pub form_bytes: u64,
  /// The name a file goes by where the browser sent it with none.
  pub unnamed: String,
}

impl FileLimits {
  /// The most bytes a request carrying `files` files may hold: each file at its largest, and the form around them.
  pub fn request_bytes(&self, files: u64) -> usize {
    let bytes = self.largest_bytes.saturating_mul(files).saturating_add(self.form_bytes);

    usize::try_from(bytes).unwrap_or(usize::MAX)
  }
}

crate::part!(pub FILES: FileLimits = "files.toml");

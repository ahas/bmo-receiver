//! The receiver's own road and the marks it reads (docs/unified-bmo.md, 「Console conversation in the private
//! chat」): the room's own record, its dialect, and the marks a picture and a voice are kept under.

use serde::Deserialize;

/// A dialect: the longest one message holds where it bounds one, and what a writer is told its marks become.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dialect {
  #[serde(default)]
  pub text_limit: Option<usize>,
  pub writing: String,
}

/// The room's own record.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
  /// The key a turn and a send name it by.
  pub key: String,
  pub label: String,
  /// The chat verbs a turn answering into it performs.
  pub can: Vec<String>,
  /// What a turn answering into it is told of who reads it.
  pub shows: String,
  pub document_kept: String,
  pub voice_kept: String,
  pub caption_kept: String,
  pub audio_kept: String,
  pub voice_source: String,
  pub audio_source: String,
  pub dialect: Dialect,
}

/// The marks the receiver matches on.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Marks {
  pub picture: String,
  pub voice: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Roads {
  pub record: Record,
  pub marks: Marks,
}

crate::part!(pub ROADS: Roads = "roads.toml");

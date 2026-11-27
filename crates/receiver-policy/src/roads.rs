//! The receiver's own road and the marks it reads (docs/unified-bmo.md, 「Console conversation in the private
//! chat」): the room's own record, its dialect, the marks a picture and a voice are kept under, how a picture a
//! message carried is filed, and what a file handed over with no name is named.

use std::collections::BTreeMap;

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
  /// The range of room numbers the record gives, from the top down.
  pub rooms_from: i64,
  pub rooms_to: i64,
  /// The stickers the record keeps, each by its emoji.
  pub stickers: Vec<String>,
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

/// How a picture a message carried is filed under its message.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pictures {
  /// Past how many bytes a picture is brought down.
  pub kept_threshold_bytes: usize,
  /// Inside how many pixels, on its longer side.
  pub kept_max_dimension: u32,
  /// The JPEG quality it is written at.
  pub kept_quality: u8,
}

/// What a file a message carried is named where it came with no name of its own.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gathering {
  pub voice_name: String,
  pub video_name: String,
  pub video_fallback_extension: String,
  /// The extension each sound's type implies.
  pub audio_extensions: BTreeMap<String, String>,
  /// The extension each video's type implies.
  pub video_extensions: BTreeMap<String, String>,
}

/// How a row a chat service handed over with its placeholder alone is filled later (docs/room-files.md, 「Files
/// filled in later」).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filling {
  /// The words a chat service writes for a file it did not carry: a row whose words are one, or open with one
  /// followed by a space or a line break, is asked for.
  pub heads: Vec<String>,
  /// How many rows a pass asks where it names none.
  pub default_rows: u32,
  /// How many rows a pass asks at most.
  pub most_rows: u32,
}

/// How another account's words said in the console go to a chat service that cannot speak as that account there.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relay {
  /// The words as they go, the shown name `{name}` before the words `{text}`.
  pub named: String,
}

/// What an agent's words carry before them where they go out in the owner's own account (docs/receiver.md, 「A
/// room's chat providers」).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NameTag {
  /// The tag of an agent whose registration names none, its label `{name}` in it.
  pub default: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Roads {
  pub record: Record,
  pub marks: Marks,
  pub relay: Relay,
  pub name_tag: NameTag,
  pub pictures: Pictures,
  pub gathering: Gathering,
  pub filling: Filling,
}

crate::part!(pub ROADS: Roads = "roads.toml");

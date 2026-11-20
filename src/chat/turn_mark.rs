//! A turn's own mark (docs/instruction.md, 「The turn's mark」): what one round of a turn has already handed the
//! session, so a later round of the same turn carries the line and not the body. It is the turn's, separate from
//! the session's mark, and is let go when the turn ends.

use std::collections::HashMap;

use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};

use crate::Fault;
use crate::js::bun_hash;
use crate::store::settings;

/// What a turn has handed: each document by its path and version, and the room lines by message number.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TurnMark {
  #[serde(default)]
  pub documents: HashMap<String, String>,
  #[serde(default)]
  pub rooms: Vec<i64>,
}

impl TurnMark {
  /// Whether the document at the path was handed in this version.
  pub fn holds(&self, path: &str, text: &str) -> bool {
    self.documents.get(path).is_some_and(|version| version == &bun_hash(text).to_string())
  }

  /// Whether the room line was handed in this turn.
  pub fn room(&self, message_id: i64) -> bool {
    self.rooms.contains(&message_id)
  }

  /// The document marked as handed, with its version.
  pub fn hand(&mut self, path: &str, text: &str) {
    self.documents.insert(path.to_owned(), bun_hash(text).to_string());
  }

  /// The room line marked as handed.
  pub fn hand_room(&mut self, message_id: i64) {
    if !self.rooms.contains(&message_id) {
      self.rooms.push(message_id);
    }
  }
}

fn key(turn: &str) -> String {
  format!("turn_mark:{turn}")
}

/// The turn's mark as it stands, none where the turn has handed nothing.
pub async fn read(db: &DatabaseConnection, turn: &str) -> TurnMark {
  settings::get(db, &key(turn))
    .await
    .ok()
    .flatten()
    .and_then(|held| serde_json::from_str(&held).ok())
    .unwrap_or_default()
}

/// The turn's mark kept.
pub async fn write(db: &DatabaseConnection, turn: &str, mark: &TurnMark) -> Result<(), Fault> {
  let text = serde_json::to_string(mark).map_err(|why| Fault::new(format!("the turn's mark: {why}")))?;

  settings::set(db, &key(turn), &text).await
}

/// The turn's mark let go as the turn ends.
pub async fn forget(db: &DatabaseConnection, turn: &str) -> Result<(), Fault> {
  settings::forget(db, &key(turn)).await
}

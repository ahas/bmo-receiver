//! A room's tone layer (docs/receiver.md, 「A room's tone layer」): the name of the tone layer a room is set to,
//! kept in the store, and the layer's text the room's block carries under its header. A room set to none, and a
//! room whose layer's file is missing or empty, carries no layer.

use receiver_policy::chat_line::CHAT_LINE;
use sea_orm::ConnectionTrait;

use super::room::Trees;
use crate::{Fault, js, one, run};

/// The tone layer a room is set to, where it is set to one.
pub async fn tone_of<C: ConnectionTrait>(db: &C, chat_id: i64) -> Result<Option<String>, Fault> {
  let row =
    one(db, r#"SELECT "tone" FROM "room_tones" WHERE "chat_id" = $1"#, vec![chat_id.into()]).await?;

  Ok(match row {
    Some(row) => Some(row.try_get("", "tone")?),
    None => None,
  })
}

/// A room set to a tone layer at a moment, or to none.
pub async fn set<C: ConnectionTrait>(
  db: &C,
  chat_id: i64,
  tone: Option<&str>,
  now: i64,
) -> Result<(), Fault> {
  match tone {
    Some(tone) => {
      run(
        db,
        r#"INSERT INTO "room_tones" ("chat_id", "tone", "set_at") VALUES ($1, $2, $3)
           ON CONFLICT ("chat_id") DO UPDATE SET "tone" = excluded."tone", "set_at" = excluded."set_at""#,
        vec![chat_id.into(), tone.into(), now.into()],
      )
      .await?;
    }
    None => {
      run(db, r#"DELETE FROM "room_tones" WHERE "chat_id" = $1"#, vec![chat_id.into()]).await?;
    }
  }

  Ok(())
}

/// The tone layer a room's block carries under its header: the named layer's file under the data root's persona
/// folder, or nothing where no layer is named or its file holds nothing.
pub fn layer(trees: &Trees, tone: Option<&str>) -> String {
  let Some(tone) = tone else {
    return String::new();
  };
  let path = trees.data.join("prompts").join("persona").join(format!("{tone}.md"));
  let text = std::fs::read_to_string(path).map(|text| js::trim(&text).to_owned()).unwrap_or_default();

  if text.is_empty() {
    String::new()
  } else {
    format!("{}\n{text}", CHAT_LINE.tone.header)
  }
}

//! A room archived and locked (`docs/receiver.md`, 「An archived room」): its record kept and read, and nothing more
//! said or woken in it until it is opened again.

use crate::{Fault, one, run};
use sea_orm::ConnectionTrait;

/// When a room was archived and by whom, where it stands archived.
pub async fn archived<C: ConnectionTrait>(db: &C, chat_id: i64) -> Result<Option<(i64, String)>, Fault> {
  let row = one(
    db,
    r#"SELECT "archived_at", "archived_by" FROM "room_archives" WHERE "chat_id" = $1"#,
    vec![chat_id.into()],
  )
  .await?;

  match row {
    Some(row) => Ok(Some((row.try_get("", "archived_at")?, row.try_get("", "archived_by")?))),
    None => Ok(None),
  }
}

/// A room archived by someone at a moment, or opened again.
pub async fn set<C: ConnectionTrait>(
  db: &C,
  chat_id: i64,
  by: &str,
  archived: bool,
  now: i64,
) -> Result<(), Fault> {
  if archived {
    run(
      db,
      r#"INSERT INTO "room_archives" ("chat_id", "archived_at", "archived_by") VALUES ($1, $2, $3)
         ON CONFLICT ("chat_id") DO NOTHING"#,
      vec![chat_id.into(), now.into(), by.into()],
    )
    .await?;
  } else {
    run(db, r#"DELETE FROM "room_archives" WHERE "chat_id" = $1"#, vec![chat_id.into()]).await?;
  }

  Ok(())
}

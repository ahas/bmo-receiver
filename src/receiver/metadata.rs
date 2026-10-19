//! What a chat service handed over beside a message as its context's `metadata` (`docs/boundary-api.md`,
//! 「Reading a room」): the platform's own payload, kept as it arrived for a message the record holds and handed back
//! unread. Nothing here reads into it; what it holds is the chat service's to state.

use sea_orm::DatabaseConnection;
use serde_json::Value;

use crate::{Fault, one, run};

/// The payload kept beside a message the record holds, replacing what was kept before; a message the record does
/// not hold keeps nothing.
pub async fn keep(
  db: &DatabaseConnection,
  chat_id: i64,
  message_id: i64,
  metadata: &Value,
) -> Result<(), Fault> {
  run(
    db,
    r#"INSERT INTO "message_metadata" ("chat_id", "message_id", "metadata")
       SELECT $1, $2, $3 WHERE EXISTS (SELECT 1 FROM "messages" WHERE "chat_id" = $1 AND "message_id" = $2)
       ON CONFLICT ("chat_id", "message_id") DO UPDATE SET "metadata" = excluded."metadata""#,
    vec![chat_id.into(), message_id.into(), metadata.to_string().into()],
  )
  .await?;

  Ok(())
}

/// The payload kept beside a message, or none where it was handed over with none.
pub async fn kept(db: &DatabaseConnection, chat_id: i64, message_id: i64) -> Result<Option<Value>, Fault> {
  let row = one(
    db,
    r#"SELECT "metadata" FROM "message_metadata" WHERE "chat_id" = $1 AND "message_id" = $2"#,
    vec![chat_id.into(), message_id.into()],
  )
  .await?;
  let Some(text) = row.map(|row| row.try_get::<String>("", "metadata")).transpose()? else {
    return Ok(None);
  };

  serde_json::from_str(&text)
    .map(Some)
    .map_err(|why| Fault::new(format!("kept metadata is no JSON: {why}")))
}

//! What a chat service says happened to a message on its platform (docs/messages.md): its reactions as the
//! platform shows them, and its deletion there, which keeps the message's words.

use sea_orm::DatabaseConnection;

use crate::{Fault, now_ms, one, run};

/// The message's reactions as the platform shows them: each reactor listed takes their emoji, or none, and where the
/// change is `whole`, every reactor not listed has none. Whether the room holds the message.
pub async fn reactions(
  db: &DatabaseConnection,
  chat_id: i64,
  message_id: i64,
  reactors: &[(String, Option<String>)],
  whole: bool,
) -> Result<bool, Fault> {
  let stands = one(
    db,
    r#"SELECT 1 AS "one" FROM "messages" WHERE "chat_id" = $1 AND "message_id" = $2"#,
    vec![chat_id.into(), message_id.into()],
  )
  .await?
  .is_some();

  if !stands {
    return Ok(false);
  }

  if whole {
    let listed: Vec<String> = reactors.iter().map(|(by, _)| by.clone()).collect();

    run(
      db,
      r#"DELETE FROM "message_reactions" WHERE "chat_id" = $1 AND "message_id" = $2
           AND NOT ("by" = ANY($3::text[]))"#,
      vec![chat_id.into(), message_id.into(), listed.into()],
    )
    .await?;
  }

  let at = now_ms();

  for (by, emoji) in reactors {
    match emoji.as_deref().map(str::trim).filter(|emoji| !emoji.is_empty()) {
      Some(emoji) => {
        run(
          db,
          r#"INSERT INTO "message_reactions" ("chat_id", "message_id", "by", "emoji", "at") VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT ("chat_id", "message_id", "by") DO UPDATE SET "emoji" = excluded."emoji",
               "at" = CASE WHEN "message_reactions"."emoji" = excluded."emoji" THEN "message_reactions"."at" ELSE excluded."at" END"#,
          vec![chat_id.into(), message_id.into(), by.clone().into(), emoji.to_owned().into(), at.into()],
        )
        .await?;
      }
      None => {
        run(
          db,
          r#"DELETE FROM "message_reactions" WHERE "chat_id" = $1 AND "message_id" = $2 AND "by" = $3"#,
          vec![chat_id.into(), message_id.into(), by.clone().into()],
        )
        .await?;
      }
    }
  }

  Ok(true)
}

/// The messages deleted on the platform, marked with the moment they went, their words kept: the ones the room holds
/// and had not marked yet.
pub async fn deleted(
  db: &DatabaseConnection,
  chat_id: i64,
  message_ids: &[i64],
) -> Result<Vec<i64>, Fault> {
  let at = now_ms();
  let mut marked = vec![];

  for message_id in message_ids {
    let changed = run(
      db,
      r#"UPDATE "messages" SET "deleted_at" = $3
         WHERE "chat_id" = $1 AND "message_id" = $2 AND "deleted_at" IS NULL"#,
      vec![chat_id.into(), (*message_id).into(), at.into()],
    )
    .await?;

    if changed > 0 {
      marked.push(*message_id);
    }
  }

  Ok(marked)
}

/// The message of the room sent by `sender_id` at `sent_at`, as the hand-over carried them, where the account that
/// names it knows it by no number the room holds; none where the room holds no such message.
pub async fn stamped(
  db: &DatabaseConnection,
  chat_id: i64,
  sender_id: i64,
  sent_at: i64,
) -> Result<Option<i64>, Fault> {
  Ok(
    one(
      db,
      r#"SELECT "message_id" FROM "messages" WHERE "chat_id" = $1 AND "sender_id" = $2 AND "sent_at" = $3
         ORDER BY "message_id" LIMIT 1"#,
      vec![chat_id.into(), sender_id.into(), sent_at.into()],
    )
    .await?
    .and_then(|row| row.try_get::<i64>("", "message_id").ok()),
  )
}

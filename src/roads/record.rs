//! The room's own record (docs/unified-bmo.md, 「The cases」): what an agent says in a room kept in the receiver's
//! database as the room's, carried to no platform, with every screen that reads the room told. It is no chat
//! service's road; it stands among the roads so a turn and a send can name it. Its numbers count down from minus
//! one, the convention for a word that never reached a platform, so they never meet a number a platform gave.

use sea_orm::{DatabaseConnection, TransactionTrait};

use crate::receiver::{Inbound, News, rows};
use crate::{Fault, now_ms, one, run};

/// The number the next word kept in a room takes: one below every number the room has held, so a number whose
/// message left the record is never handed out again.
pub async fn next_number(db: &DatabaseConnection, chat_id: i64) -> Result<i64, Fault> {
  let transaction = db.begin().await.map_err(Fault::from)?;
  let least: Option<i64> = one(
    &transaction,
    r#"SELECT MIN("message_id") AS "least" FROM "messages" WHERE "chat_id" = $1"#,
    vec![chat_id.into()],
  )
  .await?
  .map(|row| row.try_get("", "least"))
  .transpose()
  .map_err(Fault::from)?
  .flatten();
  let lowest: i64 = one(
    &transaction,
    r#"SELECT "lowest" FROM "message_number_marks" WHERE "chat_id" = $1"#,
    vec![chat_id.into()],
  )
  .await?
  .map(|row| row.try_get("", "lowest"))
  .transpose()
  .map_err(Fault::from)?
  .unwrap_or(0);
  let number = least.unwrap_or(0).min(lowest).min(0) - 1;

  run(
    &transaction,
    r#"INSERT INTO "message_number_marks" ("chat_id", "lowest") VALUES ($1, $2)
       ON CONFLICT ("chat_id") DO UPDATE SET "lowest" = LEAST("message_number_marks"."lowest", excluded."lowest")"#,
    vec![chat_id.into(), number.into()],
  )
  .await?;
  transaction.commit().await.map_err(Fault::from)?;

  Ok(number)
}

/// The number a road gave an agent as its rooms were stood up, where it gave one.
pub async fn agent_number(db: &DatabaseConnection, road: &str, agent: &str) -> Result<Option<i64>, Fault> {
  one(
    db,
    r#"SELECT "sender_id" FROM "provider_users" WHERE "provider" = $1 AND "user_id" = $2"#,
    vec![road.into(), agent.into()],
  )
  .await?
  .map(|row| row.try_get::<i64>("", "sender_id"))
  .transpose()
  .map_err(Fault::from)
}

/// What a word kept in the record is: its room, its words and what it answers, the road it is kept under, and who
/// said it, by number and name.
pub struct Kept<'a> {
  pub chat_id: i64,
  pub text: &'a str,
  pub reply_to: Option<i64>,
  pub via: &'a str,
  pub sender_id: Option<i64>,
  pub name: &'a str,
}

/// Words kept in a room's record under the next number, and the room's screen told: the number they were kept as.
pub async fn keep(db: &DatabaseConnection, news: &dyn News, kept: Kept<'_>) -> Result<i64, Fault> {
  let message = Inbound {
    chat_id: kept.chat_id,
    message_id: next_number(db, kept.chat_id).await?,
    sender_id: kept.sender_id,
    sender_name: kept.name.to_owned(),
    sent_at: now_ms(),
    text: kept.text.to_owned(),
    image_path: None,
    reply_to_message_id: kept.reply_to,
    via: kept.via.to_owned(),
    mode: None,
    via_reader: None,
    album: None,
  };

  rows::save_message(db, &message).await?;
  news.message(message.chat_id, message.message_id);

  Ok(message.message_id)
}

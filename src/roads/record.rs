//! The room's own record (docs/unified-bmo.md, 「The cases」): what an agent says in a room kept in the receiver's
//! database as the room's, carried to no platform, with every screen that reads the room told. It is no chat
//! service's road; it stands among the roads so a turn and a send can name it. Its numbers count down from minus
//! one, the convention for a word that never reached a platform, so they never meet a number a platform gave.

use sea_orm::{DatabaseConnection, TransactionTrait};

use receiver_policy::roads::ROADS;

use crate::receiver::{Inbound, News, rows};
use crate::secrets;
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
/// said it, by number and name. A send the receiver makes itself also names the agent whose words these are and
/// the voice it went out in; a word that is nobody's send leaves both unset.
pub struct Kept<'a> {
  pub chat_id: i64,
  pub text: &'a str,
  pub reply_to: Option<i64>,
  pub via: &'a str,
  pub sender_id: Option<i64>,
  pub name: &'a str,
  pub agent: Option<&'a str>,
  pub voice: Option<&'a str>,
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
    image_state: None,
    transcript: None,
    voice_seconds: None,
    voice_state: None,
    reply_to_message_id: kept.reply_to,
    via: kept.via.to_owned(),
    mode: None,
    via_reader: None,
    album: None,
  };

  rows::save_message(db, &message, kept.agent, kept.voice).await?;
  news.message(message.chat_id, message.message_id);

  Ok(message.message_id)
}

/// The settings key the lowest room number the record ever gave is kept under.
pub const ROOM_MARK: &str = "record_room_mark";

/// The number the next room the record stands up takes, a console room or the owner's private room (docs/rooms.md,
/// 「A room's number」): below every room number the store holds anywhere and below the lowest the record ever gave,
/// which it keeps, so a number once given is never given again.
pub async fn next_room(db: &DatabaseConnection) -> Result<i64, Fault> {
  let (top, bottom) = (ROADS.record.rooms_from, ROADS.record.rooms_to);
  let least = one(
    db,
    r#"SELECT MIN("least") AS "least" FROM (
         SELECT MIN("chat_id") AS "least" FROM "provider_rooms" WHERE "chat_id" BETWEEN $2 AND $1
         UNION ALL SELECT MIN("chat_id") FROM "messages" WHERE "chat_id" BETWEEN $2 AND $1
         UNION ALL SELECT MIN("chat_id") FROM "chat_titles" WHERE "chat_id" BETWEEN $2 AND $1
         UNION ALL SELECT MIN("chat_id") FROM "room_providers" WHERE "chat_id" BETWEEN $2 AND $1
         UNION ALL SELECT MIN("chat_id") FROM "bmo_rooms" WHERE "chat_id" BETWEEN $2 AND $1
       ) AS "held""#,
    vec![top.into(), bottom.into()],
  )
  .await?
  .and_then(|row| row.try_get::<Option<i64>>("", "least").ok().flatten());
  let mark = crate::store::settings::get(db, ROOM_MARK)
    .await?
    .and_then(|held| held.trim().parse::<i64>().ok());
  let next = least.into_iter().chain(mark).fold(top + 1, i64::min) - 1;

  if next < bottom {
    return Err(Fault::new(format!("the record's room numbers, {top} down to {bottom}, are spent")));
  }

  crate::store::settings::set(db, ROOM_MARK, &next.to_string()).await?;

  Ok(next)
}

/// The sender number an agent's words are filed under in a room the record stands up: below every number the road
/// has given.
pub async fn next_sender(db: &DatabaseConnection, road: &str) -> Result<i64, Fault> {
  let least = one(
    db,
    r#"SELECT MIN("sender_id") AS "least" FROM "provider_users" WHERE "provider" = $1"#,
    vec![road.into()],
  )
  .await
  .unwrap_or(None);
  let least: i64 = least.map(|row| row.try_get::<i64>("", "least").unwrap_or(0)).unwrap_or(0);

  Ok(least.min(0) - 1)
}

/// An act the record keeps on one of a room's messages (docs/messages.md, 「What an agent can send back」).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act<'a> {
  /// A reaction put on by `by`, an empty emoji taking that reactor's off.
  Reaction {
    by: &'a str,
    emoji: &'a str,
  },
  Pin {
    on: bool,
  },
  Edit {
    text: &'a str,
  },
  Delete,
}

/// The act kept on the message: whether the room's record holds the message at all.
pub async fn act(
  db: &DatabaseConnection,
  chat_id: i64,
  message_id: i64,
  act: Act<'_>,
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

  let at = now_ms();
  let (sql, values): (&str, Vec<sea_orm::Value>) = match act {
    Act::Reaction { by, emoji: "" } => (
      r#"DELETE FROM "message_reactions" WHERE "chat_id" = $1 AND "message_id" = $2 AND "by" = $3"#,
      vec![chat_id.into(), message_id.into(), by.into()],
    ),
    Act::Reaction { by, emoji } => (
      r#"INSERT INTO "message_reactions" ("chat_id", "message_id", "by", "emoji", "at") VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT ("chat_id", "message_id", "by") DO UPDATE SET "emoji" = excluded."emoji", "at" = excluded."at""#,
      vec![chat_id.into(), message_id.into(), by.into(), emoji.into(), at.into()],
    ),
    Act::Pin { on } => (
      r#"UPDATE "messages" SET "pinned_at" = $3 WHERE "chat_id" = $1 AND "message_id" = $2"#,
      vec![chat_id.into(), message_id.into(), on.then_some(at).into()],
    ),
    Act::Edit { text } => (
      r#"UPDATE "messages" SET "text" = $3, "edited_at" = $4 WHERE "chat_id" = $1 AND "message_id" = $2"#,
      vec![chat_id.into(), message_id.into(), secrets::redacted(text).into(), at.into()],
    ),
    Act::Delete => (
      r#"UPDATE "messages" SET "text" = '', "deleted_at" = $3 WHERE "chat_id" = $1 AND "message_id" = $2"#,
      vec![chat_id.into(), message_id.into(), at.into()],
    ),
  };

  run(db, sql, values).await?;

  Ok(true)
}

/// A message kept as a sticker: its kind marked once it stands.
pub async fn mark_sticker(db: &DatabaseConnection, chat_id: i64, message_id: i64) -> Result<(), Fault> {
  run(
    db,
    r#"UPDATE "messages" SET "kind" = 'sticker' WHERE "chat_id" = $1 AND "message_id" = $2"#,
    vec![chat_id.into(), message_id.into()],
  )
  .await
  .map(drop)
}

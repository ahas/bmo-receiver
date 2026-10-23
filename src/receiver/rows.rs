//! The rows the receiver reads and writes as it judges a message (docs/store.md): a room's settings, its title and
//! read position, the messages themselves, and the mode the deployment listens in.

use sea_orm::{ConnectionTrait, DatabaseConnection, DbErr, QueryResult};

use super::Inbound;
use super::commands::BotMode;
use crate::room_record::{As, RoomHolding, Rows};
use crate::{Fault, all, one, run};

// A room's picture is a row of the pictures naming the room as its subject, and its file.
crate::room_holding!(RoomHolding::Goes {
  rows: Rows {
    table: "avatars",
    condition: "kind = 'room' AND subject_id = $1",
    room: As::Number,
    reads: &[],
  },
  column: None,
  files: Some("path"),
  why: "the picture a room wears is part of its registration",
});

// The largest number a room is told apart by without losing a digit on any reader of the database.
const SAFE_INTEGER: i64 = (1 << 53) - 1;

/// Whether a room is listened to: every room the database can number is.
pub fn is_chat_allowed(chat_id: i64) -> bool {
  (-SAFE_INTEGER..=SAFE_INTEGER).contains(&chat_id)
}

/// Whether a room's registration stands: the database holds a row the receiver writes as it first sees the room — a
/// chat service's numbering of it, its title, a message kept from it, or its mode where it was added. Removing a
/// registration takes all four (docs/receiver.md, 「Removing a room's registration」).
pub async fn is_registered<C: ConnectionTrait>(db: &C, chat_id: i64) -> Result<bool, Fault> {
  Ok(
    one(
      db,
      r#"SELECT 1 AS "one" WHERE EXISTS (SELECT 1 FROM "provider_rooms" WHERE "chat_id" = $1)
           OR EXISTS (SELECT 1 FROM "chat_titles" WHERE "chat_id" = $1)
           OR EXISTS (SELECT 1 FROM "messages" WHERE "chat_id" = $1)
           OR EXISTS (SELECT 1 FROM "room_modes" WHERE "chat_id" = $1)"#,
      vec![chat_id.into()],
    )
    .await?
    .is_some(),
  )
}

/// Where work kept from a room is told: the room it was filed from, or the owner's private chat once that room's
/// registration is gone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeptWorkRoom {
  /// The chat told.
  pub chat_id: i64,
  /// The room it was filed from, where that room's registration is gone and the owner is told instead.
  pub removed: Option<i64>,
}

/// Where the news, the questions and the answers of work filed from a room go: that room while its registration
/// stands, and the owner's private chat once it is removed, never into a platform chat a send would register again
/// (docs/receiver.md, 「Removing a room's registration」).
pub async fn kept_work_room<C: ConnectionTrait>(
  db: &C,
  chat_id: i64,
  owner_id: i64,
) -> Result<KeptWorkRoom, Fault> {
  Ok(KeptWorkRoom::of(chat_id, owner_id, is_registered(db, chat_id).await?))
}

impl KeptWorkRoom {
  /// Where work filed from a room is told, for a room whose registration is known to stand or to be gone.
  pub fn of(chat_id: i64, owner_id: i64, registered: bool) -> Self {
    let removed = chat_id != owner_id && !registered;

    Self {
      chat_id: if removed { owner_id } else { chat_id },
      removed: removed.then_some(chat_id),
    }
  }
}

pub async fn allow_chat(db: &DatabaseConnection, chat_id: i64) -> Result<(), Fault> {
  run(
    db,
    r#"INSERT INTO "allowed_chats" ("chat_id") VALUES ($1) ON CONFLICT DO NOTHING"#,
    vec![chat_id.into()],
  )
  .await
  .map(drop)
}

pub async fn deny_chat(db: &DatabaseConnection, chat_id: i64) -> Result<(), Fault> {
  run(db, r#"DELETE FROM "allowed_chats" WHERE "chat_id" = $1"#, vec![chat_id.into()])
    .await
    .map(drop)
}

pub async fn set_chat_title<C: ConnectionTrait>(db: &C, chat_id: i64, title: &str) -> Result<(), Fault> {
  run(
    db,
    r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES ($1, $2)
       ON CONFLICT ("chat_id") DO UPDATE SET "title" = excluded."title""#,
    vec![chat_id.into(), title.into()],
  )
  .await
  .map(drop)
}

/// A message written down, or written over where the room already holds its number: what was measured of an
/// earlier answer under that number goes with it.
pub async fn save_message(db: &DatabaseConnection, message: &Inbound) -> Result<(), Fault> {
  run(
    db,
    r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "image_path", "reply_to_message_id", "via", "album")
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
       ON CONFLICT ("chat_id", "message_id") DO UPDATE SET "sender_id" = excluded."sender_id", "sender_name" = excluded."sender_name",
         "sent_at" = excluded."sent_at", "text" = excluded."text", "image_path" = excluded."image_path",
         "reply_to_message_id" = excluded."reply_to_message_id", "via" = excluded."via", "answer_seconds" = NULL, "round" = NULL, "album" = excluded."album""#,
    vec![
      message.chat_id.into(),
      message.message_id.into(),
      message.sender_id.into(),
      message.sender_name.clone().into(),
      message.sent_at.into(),
      message.text.clone().into(),
      message.image_path.clone().into(),
      message.reply_to_message_id.into(),
      message.via.clone().into(),
      message.album.into(),
    ],
  )
  .await
  .map(drop)
}

// A kept row as the receiver hands a message on. A row that names no road of its own is read as having come in on
// `via`.
fn inbound_of(row: &QueryResult, via: &str) -> Result<Inbound, DbErr> {
  Ok(Inbound {
    chat_id: row.try_get("", "chat_id")?,
    message_id: row.try_get("", "message_id")?,
    sender_id: row.try_get("", "sender_id")?,
    sender_name: row.try_get("", "sender_name")?,
    sent_at: row.try_get("", "sent_at")?,
    text: row.try_get("", "text")?,
    image_path: row.try_get("", "image_path")?,
    reply_to_message_id: row.try_get("", "reply_to_message_id")?,
    via: row.try_get::<Option<String>>("", "via")?.unwrap_or_else(|| via.to_owned()),
    mode: None,
    via_reader: None,
    album: row.try_get("", "album")?,
  })
}

/// One kept message, or nothing where the room holds no message under that number.
pub async fn message(
  db: &DatabaseConnection,
  chat_id: i64,
  message_id: i64,
  via: &str,
) -> Result<Option<Inbound>, Fault> {
  let row = one(
    db,
    r#"SELECT * FROM "messages" WHERE "chat_id" = $1 AND "message_id" = $2"#,
    vec![chat_id.into(), message_id.into()],
  )
  .await?;

  Ok(row.map(|row| inbound_of(&row, via)).transpose()?)
}

/// The room's latest messages, oldest first in the room's line: by when each was sent, and then by when it was
/// written down.
pub async fn recent_messages(
  db: &DatabaseConnection,
  chat_id: i64,
  count: u64,
  via: &str,
) -> Result<Vec<Inbound>, Fault> {
  let rows = all(
    db,
    r#"SELECT * FROM (SELECT * FROM "messages" WHERE "chat_id" = $1 ORDER BY "sent_at" DESC, "rowid" DESC LIMIT $2) "m"
       ORDER BY "sent_at", "rowid""#,
    vec![chat_id.into(), i64::try_from(count).unwrap_or(i64::MAX).into()],
  )
  .await?;

  Ok(rows.iter().map(|row| inbound_of(row, via)).collect::<Result<_, _>>()?)
}

/// The room's messages after one, in the room's line: after where that message stands by when it was sent and then
/// by when it was written down, or after its number where the room no longer holds it.
pub async fn messages_after(
  db: &DatabaseConnection,
  chat_id: i64,
  after: i64,
  via: &str,
) -> Result<Vec<Inbound>, Fault> {
  let rows = all(
    db,
    r#"SELECT * FROM "messages" WHERE "chat_id" = $1 AND (
         ("sent_at", "rowid") > (SELECT "c"."sent_at", "c"."rowid" FROM "messages" "c" WHERE "c"."chat_id" = $1 AND "c"."message_id" = $2)
         OR (NOT EXISTS (SELECT 1 FROM "messages" "c" WHERE "c"."chat_id" = $1 AND "c"."message_id" = $2) AND "message_id" > $2)
       ) ORDER BY "sent_at", "rowid""#,
    vec![chat_id.into(), after.into()],
  )
  .await?;

  Ok(rows.iter().map(|row| inbound_of(row, via)).collect::<Result<_, _>>()?)
}

/// The name a room's own service last showed a sender by there: their latest row that came in on any road but
/// the record's own, whose rows are the console's; none where no such row names them.
pub async fn shown_name(
  db: &DatabaseConnection,
  chat_id: i64,
  sender_id: i64,
  record: &str,
) -> Result<Option<String>, Fault> {
  let row = one(
    db,
    r#"SELECT "sender_name" FROM "messages" WHERE "chat_id" = $1 AND "sender_id" = $2
       AND ("via" IS NULL OR "via" <> $3) ORDER BY "sent_at" DESC, "rowid" DESC LIMIT 1"#,
    vec![chat_id.into(), sender_id.into(), record.into()],
  )
  .await?;

  Ok(
    row
      .map(|row| row.try_get::<String>("", "sender_name"))
      .transpose()?
      .filter(|name| !name.trim().is_empty()),
  )
}

/// The last message one sender left in the room, or nothing where they never spoke there.
pub async fn last_from(
  db: &DatabaseConnection,
  chat_id: i64,
  sender_id: i64,
  via: &str,
) -> Result<Option<Inbound>, Fault> {
  let row = one(
    db,
    r#"SELECT * FROM "messages" WHERE "chat_id" = $1 AND "sender_id" = $2 ORDER BY "sent_at" DESC, "rowid" DESC LIMIT 1"#,
    vec![chat_id.into(), sender_id.into()],
  )
  .await?;

  Ok(row.map(|row| inbound_of(&row, via)).transpose()?)
}

/// The room's read position moved to a message.
pub async fn set_last_read(db: &DatabaseConnection, chat_id: i64, message_id: i64) -> Result<(), Fault> {
  run(
    db,
    r#"INSERT INTO "chat_reads" ("chat_id", "last_read_message_id") VALUES ($1, $2)
       ON CONFLICT ("chat_id") DO UPDATE SET "last_read_message_id" = excluded."last_read_message_id""#,
    vec![chat_id.into(), message_id.into()],
  )
  .await
  .map(drop)
}

/// How long the room waited for an answer, from when it was asked to now, written onto the message it went out
/// as; nothing where the moment asked lies ahead. Both moments are epoch milliseconds and the wait is kept in
/// seconds.
pub async fn record_answer_wait(
  db: &DatabaseConnection,
  chat_id: i64,
  message_id: i64,
  asked_at: i64,
  sent_at: i64,
) -> Result<(), Fault> {
  if sent_at < asked_at {
    return Ok(());
  }

  run(
    db,
    r#"UPDATE "messages" SET "answer_seconds" = $1 WHERE "chat_id" = $2 AND "message_id" = $3"#,
    vec![((sent_at - asked_at) / 1_000).into(), chat_id.into(), message_id.into()],
  )
  .await
  .map(drop)
}

/// The round a message went out in, named on its row.
pub async fn mark_message_round(
  db: &DatabaseConnection,
  chat_id: i64,
  message_id: i64,
  round: &str,
) -> Result<(), Fault> {
  run(
    db,
    r#"UPDATE "messages" SET "round" = $1 WHERE "chat_id" = $2 AND "message_id" = $3"#,
    vec![round.into(), chat_id.into(), message_id.into()],
  )
  .await
  .map(drop)
}

/// The title the room goes by, where it has one.
pub async fn chat_title(db: &DatabaseConnection, chat_id: i64) -> Result<Option<String>, Fault> {
  let row =
    one(db, r#"SELECT "title" FROM "chat_titles" WHERE "chat_id" = $1"#, vec![chat_id.into()]).await?;

  Ok(row.map(|row| row.try_get("", "title")).transpose()?)
}

/// Whether a message carries a file a person handed over — a voice note, a sound, a video or a document — that the
/// room's store kept or tried to keep.
pub async fn carries_file(db: &DatabaseConnection, chat_id: i64, message_id: i64) -> Result<bool, Fault> {
  Ok(
    one(
      db,
      r#"SELECT 1 AS "one" FROM "collected_items"
         WHERE "chat_id" = $1 AND "message_id" = $2 AND "kind" IN ('audio', 'video', 'document') AND "source" NOT LIKE 'agent:%'
         LIMIT 1"#,
      vec![chat_id.into(), message_id.into()],
    )
    .await?
    .is_some(),
  )
}

/// The last message the room's session has read, where it has read any.
pub async fn last_read(db: &DatabaseConnection, chat_id: i64) -> Result<Option<i64>, Fault> {
  let position = one(
    db,
    r#"SELECT "last_read_message_id" FROM "chat_reads" WHERE "chat_id" = $1"#,
    vec![chat_id.into()],
  )
  .await?;

  Ok(position.map(|row| row.try_get("", "last_read_message_id")).transpose()?)
}

/// Whether the room's session has already read this message: it stands at or before the room's read position in
/// the room's line, by when it was sent and then by when it was written down.
pub async fn is_read(db: &DatabaseConnection, chat_id: i64, message_id: i64) -> Result<bool, Fault> {
  let Some(last_read) = self::last_read(db, chat_id).await? else {
    return Ok(false);
  };
  let compared = one(
    db,
    r#"SELECT CASE WHEN ("m"."sent_at", "m"."rowid") <= ("r"."sent_at", "r"."rowid") THEN 1 ELSE 0 END AS "earlier"
       FROM "messages" "m", "messages" "r"
       WHERE "m"."chat_id" = $1 AND "m"."message_id" = $2 AND "r"."chat_id" = $1 AND "r"."message_id" = $3"#,
    vec![chat_id.into(), message_id.into(), last_read.into()],
  )
  .await?;

  Ok(match compared {
    Some(row) => row.try_get::<i32>("", "earlier")? == 1,
    None => message_id <= last_read,
  })
}

pub async fn set_mode(db: &DatabaseConnection, mode: BotMode) -> Result<(), Fault> {
  crate::store::settings::set(db, "mode", mode.key()).await
}

// A room's membership of one of the tables that hold a room by being there.
async fn set_listed(db: &DatabaseConnection, table: &str, chat_id: i64, on: bool) -> Result<(), Fault> {
  let sql = if on {
    format!(r#"INSERT INTO "{table}" ("chat_id") VALUES ($1) ON CONFLICT DO NOTHING"#)
  } else {
    format!(r#"DELETE FROM "{table}" WHERE "chat_id" = $1"#)
  };

  run(db, &sql, vec![chat_id.into()]).await.map(drop)
}

pub async fn set_tools_allowed(db: &DatabaseConnection, chat_id: i64, on: bool) -> Result<(), Fault> {
  set_listed(db, "tool_chats", chat_id, on).await
}

pub async fn set_delegation(db: &DatabaseConnection, chat_id: i64, on: bool) -> Result<(), Fault> {
  set_listed(db, "delegate_chats", chat_id, on).await
}

/// A room's own word on opening pictures.
pub async fn set_photo_look(db: &DatabaseConnection, chat_id: i64, look: bool) -> Result<(), Fault> {
  run(
    db,
    r#"INSERT INTO "photo_chats" ("chat_id", "look") VALUES ($1, $2) ON CONFLICT ("chat_id") DO UPDATE SET "look" = excluded."look""#,
    vec![chat_id.into(), i64::from(look).into()],
  )
  .await
  .map(drop)
}

/// One of the operating notices a room can switch off: `rest` or `restart`.
pub async fn set_notice(db: &DatabaseConnection, chat_id: i64, kind: &str, on: bool) -> Result<(), Fault> {
  run(
    db,
    r#"INSERT INTO "chat_notices" ("chat_id", "kind", "enabled") VALUES ($1, $2, $3)
       ON CONFLICT ("chat_id", "kind") DO UPDATE SET "enabled" = excluded."enabled""#,
    vec![chat_id.into(), kind.into(), i64::from(on).into()],
  )
  .await
  .map(drop)
}

pub async fn set_muted(
  db: &DatabaseConnection,
  chat_id: i64,
  user_id: i64,
  muted: bool,
) -> Result<(), Fault> {
  let sql = if muted {
    r#"INSERT INTO "muted_members" ("chat_id", "user_id") VALUES ($1, $2) ON CONFLICT DO NOTHING"#
  } else {
    r#"DELETE FROM "muted_members" WHERE "chat_id" = $1 AND "user_id" = $2"#
  };

  run(db, sql, vec![chat_id.into(), user_id.into()]).await.map(drop)
}

/// A kept message's words written over; whether the room held the message at all. What the platform said of the
/// edit beside the words is the chat service's own, kept there.
pub async fn update_message_text(
  db: &DatabaseConnection,
  chat_id: i64,
  message_id: i64,
  text: &str,
) -> Result<bool, Fault> {
  Ok(
    run(
      db,
      r#"UPDATE "messages" SET "text" = $1 WHERE "chat_id" = $2 AND "message_id" = $3"#,
      vec![text.into(), chat_id.into(), message_id.into()],
    )
    .await?
      > 0,
  )
}

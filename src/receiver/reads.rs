//! What the receiver answers of a room: its messages, its title, the people that stood as a sender in it, and an
//! agent's read marks over it — read as another unit asks (`docs/boundary-api.md`, 「For the brain」 and 「For the
//! web unit's console」). Nothing here decides what becomes of a message; that is the receiver's own judgement
//! (`rows.rs`). A room's line is walked by when each message was sent and the order the record took it in, never
//! by its number, since a room carries two numberings and a number of one says nothing about where it stands
//! among the other's.

use sea_orm::{DatabaseConnection, DbErr, QueryResult};

use serde::Serialize;

use crate::Fault;
use crate::now_ms;

use crate::{all, one, run};

// A message of the room not in an agent's book yet, read over the room's rows as `m`.
const UNREAD: &str = r#"NOT EXISTS (SELECT 1 FROM "people_reads" "r" WHERE "r"."agent" = $1
     AND "r"."chat_id" = "m"."chat_id" AND "r"."message_id" = "m"."message_id")"#;

// Where a message stands after the cursor message the reader holds, or after its number where the room no
// longer holds that row.
const AFTER_CURSOR: &str = r#"(("m"."sent_at", "m"."rowid") > (SELECT "c"."sent_at", "c"."rowid" FROM "messages" "c"
     WHERE "c"."chat_id" = $1 AND "c"."message_id" = $2)
   OR (NOT EXISTS (SELECT 1 FROM "messages" "c" WHERE "c"."chat_id" = $1 AND "c"."message_id" = $2)
     AND "m"."message_id" > $2))"#;

// A message, read over the room's rows as `m`, that stored a picture.
const PICTURED: &str = r#""m"."image_path" IS NOT NULL AND "m"."image_path" <> ''"#;

// Where a message stands before the cursor message the reader holds.
const BEFORE_CURSOR: &str = r#"("m"."sent_at", "m"."rowid") < (SELECT "c"."sent_at", "c"."rowid" FROM "messages" "c"
     WHERE "c"."chat_id" = $1 AND "c"."message_id" = $2)"#;

/// A message as every reader of a room is handed it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
  pub chat_id: i64,
  pub message_id: i64,
  pub sender_id: Option<i64>,
  pub sender_name: String,
  pub sent_at: i64,
  pub text: String,
  pub reply_to_message_id: Option<i64>,
  pub agent_name: Option<String>,
  pub voice: Option<String>,
}

impl Message {
  fn of(row: &QueryResult) -> Result<Self, DbErr> {
    Ok(Message {
      chat_id: row.try_get("", "chat_id")?,
      message_id: row.try_get("", "message_id")?,
      sender_id: row.try_get("", "sender_id")?,
      sender_name: row.try_get("", "sender_name")?,
      sent_at: row.try_get("", "sent_at")?,
      text: row.try_get("", "text")?,
      reply_to_message_id: row.try_get("", "reply_to_message_id")?,
      agent_name: row.try_get("", "agent_name")?,
      voice: row.try_get("", "voice")?,
    })
  }
}

/// A message as the console's screen stands it: the whole row the record keeps, with the files it carried.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
  pub chat_id: i64,
  pub message_id: i64,
  pub sender_id: Option<i64>,
  pub sender_name: String,
  pub sent_at: i64,
  pub text: String,
  pub image_path: Option<String>,
  pub transcript: Option<String>,
  pub voice_seconds: Option<f64>,
  pub voice_state: Option<String>,
  pub reply_to_message_id: Option<i64>,
  pub answer_seconds: Option<i64>,
  pub round: Option<String>,
  pub album: Option<i64>,
  pub via: Option<String>,
  pub agent_name: Option<String>,
  pub voice: Option<String>,
  /// `sticker` for a sticker kept on the record, none for any other.
  pub kind: Option<String>,
  pub edited_at: Option<i64>,
  pub deleted_at: Option<i64>,
  pub pinned_at: Option<i64>,
  /// The room, the message and the name of who said it, on a copy the owner forwarded.
  pub forwarded: Option<(i64, i64, String)>,
  pub files: Vec<KeptFile>,
}

/// A file a message carried, as the room's store kept it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeptFile {
  pub kind: String,
  pub source: String,
  pub stored_path: Option<String>,
  pub summary: Option<String>,
  pub collected_at: i64,
}

impl Page {
  fn of(row: &QueryResult) -> Result<Self, DbErr> {
    Ok(Page {
      chat_id: row.try_get("", "chat_id")?,
      message_id: row.try_get("", "message_id")?,
      sender_id: row.try_get("", "sender_id")?,
      sender_name: row.try_get("", "sender_name")?,
      sent_at: row.try_get("", "sent_at")?,
      text: row.try_get("", "text")?,
      image_path: row.try_get("", "image_path")?,
      transcript: row.try_get("", "transcript")?,
      voice_seconds: row.try_get("", "voice_seconds")?,
      voice_state: row.try_get("", "voice_state")?,
      reply_to_message_id: row.try_get("", "reply_to_message_id")?,
      answer_seconds: row.try_get("", "answer_seconds")?,
      round: row.try_get("", "round")?,
      album: row.try_get("", "album")?,
      via: row.try_get::<Option<String>>("", "via")?,
      agent_name: row.try_get("", "agent_name")?,
      voice: row.try_get("", "voice")?,
      kind: row.try_get("", "kind").unwrap_or(None),
      edited_at: row.try_get("", "edited_at").unwrap_or(None),
      deleted_at: row.try_get("", "deleted_at").unwrap_or(None),
      pinned_at: row.try_get("", "pinned_at").unwrap_or(None),
      forwarded: match (
        row.try_get::<Option<i64>>("", "forwarded_room").unwrap_or(None),
        row.try_get::<Option<i64>>("", "forwarded_message").unwrap_or(None),
      ) {
        (Some(room), Some(message)) => Some((
          room,
          message,
          row.try_get::<Option<String>>("", "forwarded_name").unwrap_or(None).unwrap_or_default(),
        )),
        _ => None,
      },
      files: vec![],
    })
  }

  /// The message as every reader of a room is handed it, without the files the screen carries beside it.
  pub fn message(&self) -> Message {
    Message {
      chat_id: self.chat_id,
      message_id: self.message_id,
      sender_id: self.sender_id,
      sender_name: self.sender_name.clone(),
      sent_at: self.sent_at,
      text: self.text.clone(),
      reply_to_message_id: self.reply_to_message_id,
      agent_name: self.agent_name.clone(),
      voice: self.voice.clone(),
    }
  }
}

/// The room's messages not in an agent's book yet, oldest first, sent at or after `since` where it is given.
pub async fn unread(
  db: &DatabaseConnection,
  agent: &str,
  chat_id: i64,
  since: Option<i64>,
  limit: i64,
) -> Result<Vec<Message>, Fault> {
  let (sql, values) = match since {
    Some(since) => (
      format!(
        r#"SELECT "m".* FROM "messages" "m" WHERE "m"."chat_id" = $2 AND {UNREAD} AND "m"."sent_at" >= $3
           ORDER BY "m"."sent_at" ASC, "m"."rowid" ASC LIMIT $4"#,
      ),
      vec![agent.into(), chat_id.into(), since.into(), limit.into()],
    ),
    None => (
      format!(
        r#"SELECT "m".* FROM "messages" "m" WHERE "m"."chat_id" = $2 AND {UNREAD}
           ORDER BY "m"."sent_at" ASC, "m"."rowid" ASC LIMIT $3"#,
      ),
      vec![agent.into(), chat_id.into(), limit.into()],
    ),
  };

  Ok(all(db, &sql, values).await?.iter().map(Message::of).collect::<Result<_, _>>()?)
}

/// Every room holding messages past an agent's marks, each with how many it holds; a room named in `since`
/// counts only the messages sent at or after its moment.
pub async fn unread_counts(
  db: &DatabaseConnection,
  agent: &str,
  since: Vec<(i64, i64)>,
) -> Result<Vec<(i64, i64)>, Fault> {
  let rows = all(
    db,
    &format!(
      r#"SELECT "chat_id", COUNT(*) AS "count" FROM "messages" "m" WHERE {UNREAD} GROUP BY "chat_id""#
    ),
    vec![agent.into()],
  )
  .await?;
  let mut held: Vec<(i64, i64)> = rows
    .iter()
    .map(|row| Ok((row.try_get::<i64>("", "chat_id")?, row.try_get::<i64>("", "count")?)))
    .collect::<Result<Vec<(i64, i64)>, Fault>>()?;

  for (chat_id, moment) in since {
    let counted = one(
      db,
      &format!(
        r#"SELECT COUNT(*) AS "count" FROM "messages" "m" WHERE "m"."chat_id" = $2 AND {UNREAD}
             AND "m"."sent_at" >= $3"#,
      ),
      vec![agent.into(), chat_id.into(), moment.into()],
    )
    .await?;
    let count = counted.map(|row| row.try_get::<i64>("", "count")).transpose()?;

    held.retain(|(room, _)| *room != chat_id);

    if count.is_some_and(|count| count > 0) {
      held.push((chat_id, count.unwrap()));
    }
  }

  held.sort_unstable();

  Ok(held)
}

/// The room's messages already folded into an agent's book, oldest first, after the message `from` in the room's
/// line; where that message is gone, with a number above it.
pub async fn read_messages(
  db: &DatabaseConnection,
  agent: &str,
  chat_id: i64,
  from: i64,
  limit: i64,
) -> Result<Vec<Message>, Fault> {
  Ok(
    all(
      db,
      &format!(
        r#"SELECT "m".* FROM "messages" "m" WHERE "m"."chat_id" = $1 AND EXISTS (SELECT 1 FROM "people_reads" "r"
             WHERE "r"."agent" = $3 AND "r"."chat_id" = "m"."chat_id" AND "r"."message_id" = "m"."message_id")
           AND {AFTER_CURSOR}
           ORDER BY "m"."sent_at" ASC, "m"."rowid" ASC LIMIT $4"#,
      ),
      vec![chat_id.into(), from.into(), agent.into(), limit.into()],
    )
    .await?
    .iter()
    .map(Message::of)
    .collect::<Result<_, _>>()?,
  )
}

/// How many of a room's messages an agent has marked, of the ones that still stand.
pub async fn read_count(db: &DatabaseConnection, agent: &str, chat_id: i64) -> Result<i64, Fault> {
  Ok(
    one(
      db,
      r#"SELECT COUNT(*) AS "count" FROM "people_reads" "r" JOIN "messages" "m"
         ON "m"."chat_id" = "r"."chat_id" AND "m"."message_id" = "r"."message_id"
         WHERE "r"."agent" = $1 AND "r"."chat_id" = $2"#,
      vec![agent.into(), chat_id.into()],
    )
    .await?
    .map(|row| row.try_get::<i64>("", "count"))
    .transpose()?
    .unwrap_or(0),
  )
}

/// The room's messages marked as read into an agent's book.
pub async fn keep_marks(
  db: &DatabaseConnection,
  agent: &str,
  chat_id: i64,
  message_ids: &[i64],
) -> Result<(), Fault> {
  let now = now_ms();

  for message_id in message_ids {
    run(
      db,
      r#"INSERT INTO "people_reads" ("agent", "chat_id", "message_id", "read_at") VALUES ($1, $2, $3, $4)
         ON CONFLICT ("agent", "chat_id", "message_id") DO UPDATE SET "read_at" = excluded."read_at""#,
      vec![agent.into(), chat_id.into(), (*message_id).into(), now.into()],
    )
    .await?;
  }

  Ok(())
}

/// Every mark of an agent's book taken off a room's messages, or off every room's; how many were let go.
pub async fn forget_marks(
  db: &DatabaseConnection,
  agent: &str,
  chat_id: Option<i64>,
) -> Result<i64, Fault> {
  let (sql, values) = match chat_id {
    None => (r#"DELETE FROM "people_reads" WHERE "agent" = $1"#, vec![agent.into()]),
    Some(chat_id) => (
      r#"DELETE FROM "people_reads" WHERE "agent" = $1 AND "chat_id" = $2"#,
      vec![agent.into(), chat_id.into()],
    ),
  };

  Ok(i64::try_from(run(db, sql, values).await?).unwrap_or(0))
}

/// Every room the receiver has a record of: one it numbered, one it titled, or one a message was kept from.
pub async fn rooms(db: &DatabaseConnection) -> Result<Vec<i64>, Fault> {
  let rows = all(
    db,
    r#"SELECT "chat_id" FROM "chat_titles" "t" UNION SELECT "chat_id" FROM "messages" "m"
         UNION SELECT "chat_id" FROM "provider_rooms" "p""#,
    vec![],
  )
  .await?;

  Ok(rows.iter().map(|row| row.try_get::<i64>("", "chat_id")).collect::<Result<_, _>>()?)
}

/// Every room the record shows, with its title, how many messages it holds and when the last was sent.
pub async fn shown(db: &DatabaseConnection) -> Result<Vec<RoomLine>, Fault> {
  let rows = all(
    db,
    r#"SELECT "chat_id", "title", "message_count", "last_message_at" FROM (
           SELECT c."chat_id" AS "chat_id", t."title" AS "title",
             COALESCE(s."message_count", 0) AS "message_count", s."last_message_at" AS "last_message_at"
           FROM (SELECT "chat_id" FROM "chat_titles" "c" UNION SELECT "chat_id" FROM "messages" "m"
                 UNION SELECT "chat_id" FROM "room_modes" "a") "c"
           LEFT JOIN "chat_titles" "t" ON "t"."chat_id" = "c"."chat_id"
           LEFT JOIN (SELECT "chat_id", COUNT(*) AS "message_count", MAX("sent_at") AS "last_message_at"
                      FROM "messages" GROUP BY "chat_id") "s" ON "s"."chat_id" = "c"."chat_id"
         ) "l" ORDER BY "l"."last_message_at" IS NULL, "l"."last_message_at" DESC"#,
    vec![],
  )
  .await?;

  rows
    .iter()
    .map(|row| {
      Ok(RoomLine {
        chat_id: row.try_get("", "chat_id")?,
        title: row.try_get::<Option<String>>("", "title")?,
        message_count: row.try_get::<i64>("", "message_count")?,
        last_message_at: row.try_get::<Option<i64>>("", "last_message_at")?,
      })
    })
    .collect::<Result<Vec<RoomLine>, Fault>>()
}

/// One room as the record shows it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomLine {
  pub chat_id: i64,
  pub title: Option<String>,
  pub message_count: i64,
  pub last_message_at: Option<i64>,
}

/// A room's title, where it has one.
pub async fn title(db: &DatabaseConnection, chat_id: i64) -> Result<Option<String>, Fault> {
  Ok(
    one(db, r#"SELECT "title" FROM "chat_titles" WHERE "chat_id" = $1"#, vec![chat_id.into()])
      .await?
      .map(|row| row.try_get::<String>("", "title"))
      .transpose()?,
  )
}

/// The room's oldest message, by its number and its moment in the record's own epoch milliseconds.
pub async fn oldest(db: &DatabaseConnection, chat_id: i64) -> Result<Option<(i64, i64)>, Fault> {
  match one(
    db,
    r#"SELECT "message_id", "sent_at" FROM "messages" WHERE "chat_id" = $1
       ORDER BY "sent_at" ASC, "rowid" ASC LIMIT 1"#,
    vec![chat_id.into()],
  )
  .await?
  {
    Some(row) => Ok(Some((row.try_get::<i64>("", "message_id")?, row.try_get::<i64>("", "sent_at")?))),
    None => Ok(None),
  }
}

/// The room's latest messages, oldest first, at most `count` of them.
pub async fn latest(db: &DatabaseConnection, chat_id: i64, count: i64) -> Result<Vec<Page>, Fault> {
  Ok(
    all(
      db,
      r#"SELECT * FROM (SELECT * FROM "messages" "m" WHERE "m"."chat_id" = $1
         ORDER BY "m"."sent_at" DESC, "m"."rowid" DESC LIMIT $2) "l"
         ORDER BY "l"."sent_at", "l"."rowid""#,
      vec![chat_id.into(), count.into()],
    )
    .await?
    .iter()
    .map(Page::of)
    .collect::<Result<_, _>>()?,
  )
}

/// The messages of a room before the cursor message the reader holds, newest first, at most `count` of them.
/// The room's messages whose words carry a text, case set aside and the text read as given, before a message where
/// a cursor names one, at most `count` of the newest of them, oldest first.
pub async fn matching(
  db: &DatabaseConnection,
  chat_id: i64,
  text: &str,
  cursor: Option<i64>,
  count: i64,
) -> Result<Vec<Page>, Fault> {
  let cursor_clause = if cursor.is_some() { format!("AND {BEFORE_CURSOR}") } else { String::new() };
  let sql = format!(
    r#"SELECT * FROM (SELECT * FROM "messages" "m" WHERE "m"."chat_id" = $1 {cursor_clause}
         AND strpos(lower("m"."text"), lower($3)) > 0
         ORDER BY "m"."sent_at" DESC, "m"."rowid" DESC LIMIT $4) "l"
       ORDER BY "l"."sent_at", "l"."rowid""#
  );

  Ok(
    all(
      db,
      &sql,
      vec![chat_id.into(), cursor.unwrap_or_default().into(), text.into(), count.into()],
    )
    .await?
    .iter()
    .map(Page::of)
    .collect::<Result<_, _>>()?,
  )
}

pub async fn before(
  db: &DatabaseConnection,
  chat_id: i64,
  cursor: i64,
  count: i64,
) -> Result<Vec<Page>, Fault> {
  Ok(
    all(
      db,
      &format!(
        r#"SELECT * FROM (SELECT * FROM "messages" "m" WHERE "m"."chat_id" = $1 AND {BEFORE_CURSOR}
             ORDER BY "m"."sent_at" DESC, "m"."rowid" DESC LIMIT $3) "l"
           ORDER BY "l"."sent_at", "l"."rowid""#,
      ),
      vec![chat_id.into(), cursor.into(), count.into()],
    )
    .await?
    .iter()
    .map(Page::of)
    .collect::<Result<_, _>>()?,
  )
}

/// Which stretch of a room's pictures is asked for, by a picture the reader holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PictureWalk {
  /// The room's newest.
  Newest,
  /// That picture alone.
  At(i64),
  /// The stretch ending just before it.
  Before(i64),
  /// The stretch just past it.
  After(i64),
}

/// A stretch of the room's messages that stored a picture, oldest first, at most `count` of them.
pub async fn pictures(
  db: &DatabaseConnection,
  chat_id: i64,
  walk: PictureWalk,
  count: i64,
) -> Result<Vec<Page>, Fault> {
  let (sql, values) = match walk {
    PictureWalk::Newest => (
      format!(
        r#"SELECT * FROM (SELECT * FROM "messages" "m" WHERE "m"."chat_id" = $1 AND {PICTURED}
             ORDER BY "m"."sent_at" DESC, "m"."rowid" DESC LIMIT $2) "l"
           ORDER BY "l"."sent_at", "l"."rowid""#
      ),
      vec![chat_id.into(), count.into()],
    ),
    PictureWalk::At(message) => (
      format!(
        r#"SELECT * FROM "messages" "m" WHERE "m"."chat_id" = $1 AND {PICTURED} AND "m"."message_id" = $2"#
      ),
      vec![chat_id.into(), message.into()],
    ),
    PictureWalk::Before(message) => (
      format!(
        r#"SELECT * FROM (SELECT * FROM "messages" "m" WHERE "m"."chat_id" = $1 AND {PICTURED} AND {BEFORE_CURSOR}
             ORDER BY "m"."sent_at" DESC, "m"."rowid" DESC LIMIT $3) "l"
           ORDER BY "l"."sent_at", "l"."rowid""#
      ),
      vec![chat_id.into(), message.into(), count.into()],
    ),
    PictureWalk::After(message) => (
      format!(
        r#"SELECT "m".* FROM "messages" "m" WHERE "m"."chat_id" = $1 AND {PICTURED} AND {AFTER_CURSOR}
           ORDER BY "m"."sent_at" ASC, "m"."rowid" ASC LIMIT $3"#
      ),
      vec![chat_id.into(), message.into(), count.into()],
    ),
  };

  Ok(all(db, &sql, values).await?.iter().map(Page::of).collect::<Result<_, _>>()?)
}

/// How many of the room's pictures stand before the picture message given, and how many it holds.
pub async fn pictures_counted(
  db: &DatabaseConnection,
  chat_id: i64,
  first: Option<i64>,
) -> Result<(i64, i64), Fault> {
  let counted = |sql: String, values: Vec<sea_orm::Value>| async move {
    let row = one(db, &sql, values).await?;

    Ok::<i64, Fault>(row.map(|row| row.try_get::<i64>("", "n")).transpose()?.unwrap_or(0))
  };
  let total = counted(
    format!(r#"SELECT COUNT(*) AS "n" FROM "messages" "m" WHERE "m"."chat_id" = $1 AND {PICTURED}"#),
    vec![chat_id.into()],
  )
  .await?;
  let before = match first {
    Some(message) => {
      counted(
        format!(
          r#"SELECT COUNT(*) AS "n" FROM "messages" "m" WHERE "m"."chat_id" = $1 AND {PICTURED} AND {BEFORE_CURSOR}"#
        ),
        vec![chat_id.into(), message.into()],
      )
      .await?
    }
    None => 0,
  };

  Ok((before, total))
}

/// The messages of a room past the cursor message the reader holds, oldest first, at most `count` of them.
pub async fn after(
  db: &DatabaseConnection,
  chat_id: i64,
  cursor: i64,
  count: i64,
) -> Result<Vec<Page>, Fault> {
  Ok(
    all(
      db,
      &format!(
        r#"SELECT "m".* FROM "messages" "m" WHERE "m"."chat_id" = $1 AND {AFTER_CURSOR}
           ORDER BY "m"."sent_at" ASC, "m"."rowid" ASC LIMIT $3"#,
      ),
      vec![chat_id.into(), cursor.into(), count.into()],
    )
    .await?
    .iter()
    .map(Page::of)
    .collect::<Result<_, _>>()?,
  )
}

/// The first messages of a room sent at or after a moment, oldest first.
pub async fn from(
  db: &DatabaseConnection,
  chat_id: i64,
  since: i64,
  count: i64,
) -> Result<Vec<Page>, Fault> {
  Ok(
    all(
      db,
      r#"SELECT "m".* FROM "messages" "m" WHERE "m"."chat_id" = $1 AND "m"."sent_at" >= $2
         ORDER BY "m"."sent_at" ASC, "m"."rowid" ASC LIMIT $3"#,
      vec![chat_id.into(), since.into(), count.into()],
    )
    .await?
    .iter()
    .map(Page::of)
    .collect::<Result<_, _>>()?,
  )
}

/// The one message the room holds under that number, or nothing.
pub async fn message_of(
  db: &DatabaseConnection,
  chat_id: i64,
  message_id: i64,
) -> Result<Option<Page>, Fault> {
  Ok(
    one(
      db,
      r#"SELECT * FROM "messages" WHERE "chat_id" = $1 AND "message_id" = $2"#,
      vec![chat_id.into(), message_id.into()],
    )
    .await?
    .map(|row| Page::of(&row))
    .transpose()?,
  )
}

/// One profile picture the archive keeps: whose it is, where its file stands and when it was last taken.
#[derive(Debug, Clone, Serialize)]
pub struct Avatar {
  pub kind: String,
  pub subject_id: i64,
  pub path: Option<String>,
  pub taken_at: i64,
}

impl Avatar {
  fn of(row: &QueryResult) -> Result<Self, DbErr> {
    Ok(Self {
      kind: row.try_get("", "kind")?,
      subject_id: row.try_get("", "subject_id")?,
      path: row.try_get("", "path")?,
      taken_at: row.try_get("", "updated_at")?,
    })
  }
}

/// The pictures kept with a file, oldest taken first, from after a moment, at most `limit` of them.
pub async fn avatars(
  db: &DatabaseConnection,
  after: Option<i64>,
  limit: i64,
) -> Result<Vec<Avatar>, Fault> {
  all(
    db,
    // Each subject once, its standing picture the newer of the one handed over and the one set from the console.
    r#"SELECT "kind", "subject_id", "path", "updated_at" FROM (
         SELECT DISTINCT ON ("kind", "subject_id") "kind", "subject_id", "path", "updated_at" FROM (
           SELECT "kind", "subject_id", "path", "updated_at" FROM "avatars" WHERE "path" IS NOT NULL AND "path" <> ''
           UNION ALL SELECT "kind", "subject_id", "path", "set_at" AS "updated_at" FROM "avatar_sets"
         ) AS "kept"
         ORDER BY "kind", "subject_id", "updated_at" DESC
       ) AS "standing"
       WHERE "updated_at" > $1
       ORDER BY "updated_at", "kind", "subject_id" LIMIT $2"#,
    vec![after.unwrap_or(0).into(), limit.into()],
  )
  .await?
  .iter()
  .map(|row| Avatar::of(row).map_err(Fault::from))
  .collect()
}

/// The picture kept for one room or person, or nothing.
pub async fn avatar_of(
  db: &DatabaseConnection,
  kind: &str,
  subject_id: i64,
) -> Result<Option<Avatar>, Fault> {
  Ok(
    one(
      db,
      r#"SELECT "kind", "subject_id", "path", "updated_at" FROM (
           SELECT "kind", "subject_id", "path", "updated_at" FROM "avatars" WHERE "kind" = $1 AND "subject_id" = $2
           UNION ALL SELECT "kind", "subject_id", "path", "set_at" AS "updated_at" FROM "avatar_sets"
             WHERE "kind" = $1 AND "subject_id" = $2
         ) AS "kept" ORDER BY "updated_at" DESC LIMIT 1"#,
      vec![kind.into(), subject_id.into()],
    )
    .await?
    .map(|row| Avatar::of(&row))
    .transpose()?,
  )
}

/// A reaction kept on the record on a message: its emoji and who put it there.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Reaction {
  pub emoji: String,
  pub by: String,
}

/// The reactions the record keeps on a message, in the order they were put there.
pub async fn reactions(
  db: &DatabaseConnection,
  chat_id: i64,
  message_id: i64,
) -> Result<Vec<Reaction>, Fault> {
  all(
    db,
    r#"SELECT "emoji", "by" FROM "message_reactions" WHERE "chat_id" = $1 AND "message_id" = $2 ORDER BY "at", "by""#,
    vec![chat_id.into(), message_id.into()],
  )
  .await?
  .iter()
  .map(|row| {
    Ok(Reaction {
      emoji: row.try_get("", "emoji")?,
      by: row.try_get("", "by")?,
    })
  })
  .collect::<Result<_, DbErr>>()
  .map_err(Fault::from)
}

/// The files a message carried, as the room's store kept them.
pub async fn files(db: &DatabaseConnection, chat_id: i64, message_id: i64) -> Result<Vec<KeptFile>, Fault> {
  all(
    db,
    r#"SELECT "kind", "source", "stored_path", "summary", "collected_at" FROM "collected_items"
       WHERE "chat_id" = $1 AND "message_id" = $2 ORDER BY "collected_at", "source""#,
    vec![chat_id.into(), message_id.into()],
  )
  .await?
  .iter()
  .map(|row| {
    Ok(KeptFile {
      kind: row.try_get("", "kind")?,
      source: row.try_get("", "source")?,
      stored_path: row.try_get::<Option<String>>("", "stored_path")?,
      summary: row.try_get::<Option<String>>("", "summary")?,
      collected_at: row.try_get("", "collected_at")?,
    })
  })
  .collect::<Result<Vec<KeptFile>, Fault>>()
}

/// The people that have stood as a sender in a room.
pub async fn senders(db: &DatabaseConnection, chat_id: i64) -> Result<Vec<i64>, Fault> {
  let rows = all(
    db,
    r#"SELECT DISTINCT "sender_id" FROM "messages" "m"
       WHERE "m"."chat_id" = $1 AND "sender_id" IS NOT NULL"#,
    vec![chat_id.into()],
  )
  .await?;

  Ok(rows.iter().map(|row| row.try_get::<i64>("", "sender_id")).collect::<Result<_, _>>()?)
}

/// The people that have stood as a sender in a room, of those asked.
pub async fn spoken_in(db: &DatabaseConnection, chat_id: i64, people: &[i64]) -> Result<Vec<i64>, Fault> {
  let stood = senders(db, chat_id).await?;

  Ok(
    people
      .iter()
      .filter(|person| stood.iter().any(|sender| sender == *person))
      .cloned()
      .collect(),
  )
}

/// Whether a person sent a message in a room between two moments, both included, in epoch milliseconds.
pub async fn spoke_between(
  db: &DatabaseConnection,
  chat_id: i64,
  person: i64,
  from: i64,
  to: i64,
) -> Result<bool, Fault> {
  Ok(
    one(
      db,
      r#"SELECT 1 AS "one" FROM "messages" "m" WHERE "m"."chat_id" = $1 AND "m"."sender_id" = $2
         AND "m"."sent_at" BETWEEN $3 AND $4 LIMIT 1"#,
      vec![chat_id.into(), person.into(), from.into(), to.into()],
    )
    .await?
    .is_some(),
  )
}

/// The name a person last went by, a room where the owner goes by a name of that room alone read last; none
/// where they never spoke or spoke under no name.
pub async fn display_name(db: &DatabaseConnection, person: i64) -> Result<Option<String>, Fault> {
  let name =
    one(
      db,
      r#"SELECT "sender_name" FROM "messages" "m" WHERE "m"."sender_id" = $1
         ORDER BY "m"."chat_id" IN (SELECT "chat_id" FROM "provider_owner_users"), "m"."sent_at" DESC LIMIT 1"#,
      vec![person.into()],
    )
  .await?;

  Ok(
    name
      .map(|row| row.try_get::<String>("", "sender_name"))
      .transpose()?
      .filter(|name| !name.trim().is_empty()),
  )
}

/// Whether a sender is the owner in a room: a road gave the sender a number out of a user id that stands as
/// that room's owner.
pub async fn owner_of_sender(
  db: &DatabaseConnection,
  via: &str,
  chat_id: i64,
  sender: i64,
) -> Result<bool, Fault> {
  Ok(
    one(
      db,
      r#"SELECT 1 AS "one" WHERE EXISTS (SELECT 1 FROM "provider_users" "u" WHERE "u"."provider" = $1
         AND "u"."sender_id" = $3
         AND EXISTS (SELECT 1 FROM "provider_owner_users" "o" WHERE "o"."provider" = "u"."provider"
           AND "o"."chat_id" = $2 AND "o"."user_id" = "u"."user_id"))"#,
      vec![via.into(), chat_id.into(), sender.into()],
    )
    .await?
    .is_some(),
  )
}

/// Whether the message the room holds under that number was said by the owner.
pub async fn sender_is_owner(
  db: &DatabaseConnection,
  via: &str,
  chat_id: i64,
  message_id: i64,
) -> Result<bool, Fault> {
  let Some(message) = message_of(db, chat_id, message_id).await? else {
    return Ok(false);
  };
  let Some(sender) = message.sender_id else { return Ok(false) };

  owner_of_sender(db, via, chat_id, sender).await
}

/// The sender numbers a road gave any of the agents named: the numbers an agent's own words stand under.
pub async fn agent_numbers(db: &DatabaseConnection, agents: &[String]) -> Result<Vec<i64>, Fault> {
  if agents.is_empty() {
    return Ok(vec![]);
  }

  let named = (1..=agents.len()).map(|at| format!("${at}")).collect::<Vec<_>>();
  let sql = format!(
    r#"SELECT DISTINCT "sender_id" FROM "provider_users" "u" WHERE "u"."user_id" IN ({})"#,
    named.join(","),
  );

  Ok(
    all(db, &sql, agents.iter().map(|agent| agent.into()).collect())
      .await?
      .iter()
      .map(|row| row.try_get::<i64>("", "sender_id"))
      .collect::<Result<_, _>>()?,
  )
}

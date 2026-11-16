//! A room's placeholder rows filled later from its chat service (docs/room-files.md, 「Files filled in later」): a
//! row a road handed over with only the words it writes for a file it did not carry is asked of that road again by
//! its room, number and sent time, and what comes back is kept as a live line's files are. Each row is asked once,
//! its outcome recorded; a pass asks a bounded number of rows of one room, newest first, one at a time, and a road
//! that cannot be reached ends it with nothing recorded.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::{LazyLock, Mutex};

use bmo_chat_client::{ChatCapability, ChatMediaRequest, ClientFailure, ErrorCode};
use receiver_client::{FileFill, FileFills, FillOutcome, FilledFile, FilledRow};
use receiver_codes::FileFillRefusal;
use receiver_policy::roads::ROADS;
use sea_orm::{DatabaseConnection, Value};

use super::{Receiver, files, rows};
use crate::roads::added;
use crate::{Fault, all, now_ms, run};

/// Why a pass did not run.
#[derive(Debug)]
pub enum FillRefused {
  Refused(FileFillRefusal),
  Store(Fault),
}

impl From<Fault> for FillRefused {
  fn from(why: Fault) -> Self {
    Self::Store(why)
  }
}

impl From<FileFillRefusal> for FillRefused {
  fn from(why: FileFillRefusal) -> Self {
    Self::Refused(why)
  }
}

// The rooms a pass runs in now, so a second pass never asks a row the first is asking.
static FILLING: LazyLock<Mutex<BTreeSet<i64>>> = LazyLock::new(|| Mutex::new(BTreeSet::new()));

// A room held as filling for as long as the pass lives.
struct Filling(i64);

impl Filling {
  // The set is let go before a hold is made, since a hold dropped while the set is locked would lock it again.
  fn hold(room: i64) -> Option<Self> {
    let held = FILLING.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).insert(room);

    held.then(|| Self(room))
  }
}

impl Drop for Filling {
  fn drop(&mut self) {
    FILLING.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).remove(&self.0);
  }
}

/// How many rows a pass asks: the number asked for, the policy's default where none is, never past its ceiling.
pub fn bounded(rows: Option<u32>) -> u32 {
  let filling = &ROADS.filling;

  rows.unwrap_or(filling.default_rows).min(filling.most_rows)
}

/// One row a pass may ask for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placeholder {
  pub message_id: i64,
  pub sent_at: i64,
}

/// The room's rows a pass may ask its road for, newest first, at most `limit`: rows the road numbered whose words
/// are a placeholder head or open with one followed by a space or a line break, with no picture, no kept file and no
/// ask recorded, and not deleted.
pub async fn placeholders(
  db: &DatabaseConnection,
  room: i64,
  road: &str,
  limit: u32,
) -> Result<Vec<Placeholder>, Fault> {
  let heads = &ROADS.filling.heads;
  let mut values: Vec<Value> = vec![room.into(), road.into(), i64::from(limit).into()];
  let matching: Vec<String> = heads
    .iter()
    .map(|head| {
      values.push(head.clone().into());
      let at = values.len();

      format!(
        r#"(m."text" = ${at} OR starts_with(m."text", ${at} || ' ') OR starts_with(m."text", ${at} || chr(10)))"#
      )
    })
    .collect();

  if matching.is_empty() {
    return Ok(vec![]);
  }

  let sql = format!(
    r#"SELECT m."message_id", m."sent_at" FROM "messages" m
       WHERE m."chat_id" = $1 AND m."via" = $2
         AND m."deleted_at" IS NULL AND m."image_path" IS NULL AND m."kind" IS DISTINCT FROM 'thinking'
         AND NOT EXISTS (SELECT 1 FROM "collected_items" c WHERE c."chat_id" = m."chat_id" AND c."message_id" = m."message_id")
         AND NOT EXISTS (SELECT 1 FROM "message_file_asks" a WHERE a."chat_id" = m."chat_id" AND a."message_id" = m."message_id")
         AND ({})
       ORDER BY m."sent_at" DESC, m."message_id" DESC
       LIMIT $3"#,
    matching.join(" OR ")
  );

  all(db, &sql, values)
    .await?
    .iter()
    .map(|row| {
      Ok(Placeholder {
        message_id: row.try_get("", "message_id")?,
        sent_at: row.try_get("", "sent_at")?,
      })
    })
    .collect()
}

// What the road answered for one row, as the record keeps it.
struct Answer {
  outcome: FillOutcome,
  reason: Option<String>,
}

impl Answer {
  fn of(outcome: FillOutcome, reason: Option<String>) -> Self {
    Self { outcome, reason }
  }
}

// A road's failure read as the row's outcome: the platform's `unavailable`, any other answer a refusal; none where
// the road was not reached, which ends the pass with nothing recorded.
fn answered_failure(why: &ClientFailure) -> Option<Answer> {
  match why {
    ClientFailure::Unreachable { .. } | ClientFailure::Transport { .. } => None,
    ClientFailure::Answered {
      answer: Some(answer),
      ..
    } if answer.code == ErrorCode::Unavailable => {
      Some(Answer::of(FillOutcome::Unavailable, Some(answer.error.clone())))
    }
    ClientFailure::Answered {
      answer: Some(answer),
      ..
    } => Some(Answer::of(FillOutcome::Refused, Some(answer.error.clone()))),
    failure => Some(Answer::of(FillOutcome::Refused, Some(failure.to_string()))),
  }
}

// A file's size on the disk, nothing where it is gone.
fn size_of(path: &Path) -> Option<u64> {
  std::fs::metadata(path).ok().filter(|held| held.is_file()).map(|held| held.len())
}

// What the stores hold of a message's files now: its pictures, the first on its row and an album's later ones
// beside it, then its kept files with their kinds.
async fn held_files(db: &DatabaseConnection, room: i64, message: i64) -> Result<Vec<FilledFile>, Fault> {
  let mut held = vec![];
  let row = rows::message(db, room, message, "").await?;

  if let Some(first) = row.and_then(|row| row.image_path).map(std::path::PathBuf::from) {
    let folder = first.parent().map(Path::to_path_buf).unwrap_or_default();
    let later = (1..).map_while(|place| files::later_picture(&folder, message, place));

    for picture in std::iter::once(first).chain(later) {
      let Some(bytes) = size_of(&picture) else { break };

      held.push(FilledFile {
        kind: "image".to_owned(),
        bytes,
      });
    }
  }

  let kept = all(
    db,
    r#"SELECT "kind", "stored_path" FROM "collected_items" WHERE "chat_id" = $1 AND "message_id" = $2 ORDER BY "id""#,
    vec![room.into(), message.into()],
  )
  .await?;

  for file in kept {
    let stored: Option<String> = file.try_get("", "stored_path")?;

    held.push(FilledFile {
      kind: file.try_get("", "kind")?,
      bytes: stored.as_deref().map(Path::new).and_then(size_of).unwrap_or(0),
    });
  }

  Ok(held)
}

// The outcome kept on the row's account, so no pass asks it again.
async fn record(
  db: &DatabaseConnection,
  room: i64,
  message: i64,
  answer: &Answer,
  files: &[FilledFile],
) -> Result<(), Fault> {
  let bytes: u64 = files.iter().map(|file| file.bytes).sum();

  run(
    db,
    r#"INSERT INTO "message_file_asks" ("chat_id", "message_id", "outcome", "reason", "files", "bytes", "asked_at")
       VALUES ($1, $2, $3, $4, $5, $6, $7)
       ON CONFLICT ("chat_id", "message_id") DO NOTHING"#,
    vec![
      room.into(),
      message.into(),
      answer.outcome.as_str().into(),
      answer.reason.clone().into(),
      i64::try_from(files.len()).unwrap_or(i64::MAX).into(),
      i64::try_from(bytes).unwrap_or(i64::MAX).into(),
      now_ms().into(),
    ],
  )
  .await
  .map(drop)
}

/// A pass over a room's placeholder rows: each asked of the room's road once, what comes back kept, the outcome
/// recorded on the row; refused where no road of the room fetches a message's files or a pass already runs there.
pub async fn fill(receiver: &Receiver, room: i64, asked: &FileFill) -> Result<FileFills, FillRefused> {
  let roads = receiver.roads();
  let no_files = FileFillRefusal::RoomFetchesNoFiles { room };
  let road = roads.room_road(room).await.ok_or(no_files.clone())?;
  let declared = roads.declaration(&road).await.ok_or(no_files.clone())?;
  let service = roads.chats().road(&road).filter(|_| added::can(&declared, ChatCapability::FetchMedia));
  let service = service.ok_or(no_files)?;
  let _filling = Filling::hold(room).ok_or(FileFillRefusal::RoomFilesFilling { room })?;
  let db = receiver.db();
  let found = placeholders(db, room, &road, bounded(asked.rows)).await?;
  let mut filled = FileFills {
    room,
    rows: vec![],
    unreached: None,
  };

  for row in found {
    let mut reached = FilledRow {
      message: row.message_id,
      sent_at: row.sent_at,
      outcome: None,
      reason: None,
      files: vec![],
    };

    if asked.dry_run {
      filled.rows.push(reached);
      continue;
    }

    let request = ChatMediaRequest {
      room_id: room,
      message_id: row.message_id,
      sent_at: Some(row.sent_at),
    };
    let answer = match service.message_files(&request).await {
      Ok(page) if page.items.is_empty() => Answer::of(FillOutcome::None, None),
      Ok(page) => {
        let Some(message) = rows::message(db, room, row.message_id, &road).await? else {
          continue;
        };

        files::keep(db, receiver.data_root(), &message, &page.items).await?;
        reached.files = held_files(db, room, row.message_id).await?;

        Answer::of(FillOutcome::Filled, None)
      }
      Err(why) => match answered_failure(&why) {
        Some(answer) => answer,
        None => {
          filled.unreached = Some(why.to_string());
          break;
        }
      },
    };

    record(db, room, row.message_id, &answer, &reached.files).await?;
    reached.outcome = Some(answer.outcome);
    reached.reason = answer.reason;
    filled.rows.push(reached);
  }

  Ok(filled)
}

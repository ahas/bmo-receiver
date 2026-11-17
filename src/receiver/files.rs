//! The files a message carried, kept as the chat service hands them over (docs/room-files.md, 「Message files」;
//! docs/pictures.md, 「Pictures received」). A picture is filed under its message in the picture store, brought down
//! and laid on white where it must be, its path on the message's row. Any other file is kept in the message-file
//! store with its row in `collected_items`, whatever its size: a video kept as a video whatever the service called
//! it, a sound or a video with no name of its own named for what it is. Keeping a file launches nothing and notifies
//! nobody; a file that could not be kept leaves its row, which a later hand-over of the same file fills, and the
//! message stands.

use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};

use bmo_chat_client::{ChatFileKind, FileBody, MessageFile};
use receiver_policy::roads::ROADS;
use sea_orm::DatabaseConnection;

use super::Inbound;
use super::pictures::{self, Keeping};
use crate::data_root::{DataRoot, room_path};
use crate::roads::record::mark_sticker;
use crate::{Fault, all, now_ms, one, run};

/// A file a message carried, as the receiver holds it until it is filed: whole in memory, as a chat service hands
/// it over, or on the disk, as a form of the console was written while it arrived (docs/room-files.md, 「Message
/// files」).
pub trait Handed: Sync {
  /// Its id among its message's files.
  fn id(&self) -> &str;
  /// Its own name, empty where it came with none.
  fn name(&self) -> &str;
  fn mime(&self) -> &str;
  /// What the chat service handed it over as, where it said.
  fn kind(&self) -> Option<ChatFileKind>;
  /// Its bytes read whole, as a picture is drawn from them.
  fn bytes(&self) -> impl Future<Output = io::Result<Vec<u8>>> + Send;
  /// Its bytes filed at `path`, in a folder that already stands; a file that arrived in `spool` is moved from it.
  fn filed_at(&self, path: &Path, spool: &Path) -> impl Future<Output = io::Result<()>> + Send;
}

impl Handed for MessageFile {
  fn id(&self) -> &str {
    &self.id
  }

  fn name(&self) -> &str {
    &self.name
  }

  fn mime(&self) -> &str {
    &self.mime
  }

  fn kind(&self) -> Option<ChatFileKind> {
    self.kind
  }

  async fn bytes(&self) -> io::Result<Vec<u8>> {
    self.bytes.read().await
  }

  // A file that arrived as a part stands in the spool, whose name for it goes once its last holder lets go: it is
  // linked where it is filed, which moves it, while the hearing and the carriage to other providers still read it
  // where it arrived. A file a send took from where it stands is its sender's to change, so it is copied. Neither
  // is read whole into memory.
  async fn filed_at(&self, path: &Path, spool: &Path) -> io::Result<()> {
    match &self.bytes {
      FileBody::Stored(stored) => {
        if stored.path().starts_with(spool) && tokio::fs::hard_link(stored.path(), path).await.is_ok() {
          return Ok(());
        }

        tokio::fs::copy(stored.path(), path).await.map(drop)
      }
      held => tokio::fs::write(path, held.read().await?).await,
    }
  }
}

/// What a kept file is, as its row names it, by the type it came under: a picture, a sound, a video, or any other
/// file.
pub fn kind_of(mime: &str) -> &'static str {
  match bare_type(mime).split('/').next().unwrap_or_default() {
    "image" => "image",
    "audio" => "audio",
    "video" => "video",
    _ => "document",
  }
}

/// What a kept file's row names it: a sticker or an animation as the chat service handed it over, where it is no
/// picture; otherwise by its type (docs/room-files.md, 「Message files」).
pub fn kept_kind(file: &impl Handed) -> &'static str {
  let by_type = kind_of(file.mime());

  match file.kind() {
    Some(ChatFileKind::Sticker) if by_type != "image" => "sticker",
    Some(ChatFileKind::Animation) if by_type != "image" => "animation",
    _ => by_type,
  }
}

// A type as its first part reads, without its parameters or its case.
fn bare_type(mime: &str) -> String {
  mime.split(';').next().unwrap_or_default().trim().to_lowercase()
}

/// The name a file goes by: its own, or for a sound or a video handed over with none, what it is with the extension
/// its type implies.
pub fn named(file: &impl Handed) -> String {
  if !file.name().trim().is_empty() {
    return file.name().to_owned();
  }

  let gathering = &ROADS.gathering;
  let mime = bare_type(file.mime());

  match kept_kind(file) {
    "video" | "animation" => format!(
      "{}{}",
      gathering.video_name,
      gathering.video_extensions.get(&mime).unwrap_or(&gathering.video_fallback_extension)
    ),
    "audio" => format!(
      "{}{}",
      gathering.voice_name,
      gathering.audio_extensions.get(&mime).map(String::as_str).unwrap_or_default()
    ),
    _ => "file".to_owned(),
  }
}

// The name a file is filed under in its room's folder: its own name's last part, behind the message's number, so
// two messages' files of one name stand apart, and from the second `place` on behind that number too, so two files
// of one name in one message do.
fn filed_name(message_id: i64, place: u32, name: &str) -> String {
  let own = Path::new(name)
    .file_name()
    .map(|own| own.to_string_lossy().into_owned())
    .filter(|own| !own.is_empty() && !own.starts_with('.'))
    .unwrap_or_else(|| "file".to_owned());

  match place {
    1 => format!("{message_id}-{own}"),
    place => format!("{message_id}-{place}-{own}"),
  }
}

async fn written(path: &Path, bytes: &[u8]) -> io::Result<()> {
  if let Some(folder) = path.parent() {
    tokio::fs::create_dir_all(folder).await?;
  }

  tokio::fs::write(path, bytes).await
}

// A file filed at `path`, its folder made where it is missing.
async fn filed(path: &Path, file: &impl Handed, spool: &Path) -> io::Result<()> {
  if let Some(folder) = path.parent() {
    tokio::fs::create_dir_all(folder).await?;
  }

  file.filed_at(path, spool).await
}

/// Every file a message carried kept: its pictures in the picture store, the first of them named on the message's
/// row, and its other files in the message-file store, each with its row.
pub async fn keep(
  db: &DatabaseConnection,
  data_root: &DataRoot,
  inbound: &Inbound,
  files: &[impl Handed],
) -> Result<(), Fault> {
  let mut pictures = 0;

  for file in files {
    if kind_of(file.mime()) == "image" {
      if picture(db, data_root, inbound.chat_id, inbound.message_id, file, pictures).await? {
        pictures += 1;
      }

      continue;
    }

    collected(db, data_root, inbound, file).await?;
  }

  Ok(())
}

/// The extensions a message's picture is filed under: `jpg` as the store draws a picture, or the moving one's own
/// that a sticker or an animation is kept as (`pictures::moving`).
const FILED_AS: [&str; 3] = ["jpg", "webp", "gif"];

// The name a message's picture is filed under: the first as `<message>.<extension>`, a later one with its place.
fn picture_name(message_id: i64, before: usize, extension: &str) -> String {
  match before {
    0 => format!("{message_id}.{extension}"),
    n => format!("{message_id}-{n}.{extension}"),
  }
}

/// The later picture of a message at `place`, from 1, where one stands in its room's folder of the picture store.
pub fn later_picture(folder: &Path, message_id: i64, place: usize) -> Option<PathBuf> {
  FILED_AS
    .iter()
    .map(|extension| folder.join(picture_name(message_id, place, extension)))
    .find(|path| path.is_file())
}

// One picture filed under its message, the first's path on the row: whether it was kept. A sticker or an animation
// that moves is filed as it came under its own extension, any other picture as the store draws it; a sticker marks
// its message a sticker (docs/pictures.md, 「Pictures received」).
async fn picture(
  db: &DatabaseConnection,
  data_root: &DataRoot,
  chat_id: i64,
  message_id: i64,
  file: &impl Handed,
  before: usize,
) -> Result<bool, Fault> {
  let filed = &ROADS.pictures;
  let keeping = Keeping {
    threshold_bytes: filed.kept_threshold_bytes,
    max_dimension: filed.kept_max_dimension,
    quality: filed.kept_quality,
  };
  let bytes = match file.bytes().await {
    Ok(bytes) => bytes,
    Err(why) => {
      eprintln!("bmo-receiver: the picture of message {message_id} was not read: {why}");
      image_state(db, chat_id, message_id, "failed").await?;

      return Ok(false);
    }
  };
  let may_move = matches!(file.kind(), Some(ChatFileKind::Sticker | ChatFileKind::Animation));
  let filing = move || match pictures::moving(&bytes).filter(|_| may_move) {
    Some(extension) => (bytes, extension),
    None => (pictures::kept(bytes, keeping), "jpg"),
  };
  let (kept, extension) = match tokio::task::spawn_blocking(filing).await {
    Ok(kept) => kept,
    Err(why) => {
      eprintln!("bmo-receiver: the picture of message {message_id} was not kept: {why}");
      image_state(db, chat_id, message_id, "failed").await?;

      return Ok(false);
    }
  };

  let path = room_path(&data_root.images(), chat_id).join(picture_name(message_id, before, extension));

  if let Err(why) = written(&path, &kept).await {
    eprintln!("bmo-receiver: the picture of message {message_id} was not kept: {why}");
    image_state(db, chat_id, message_id, "failed").await?;

    return Ok(false);
  }

  if before == 0 {
    run(
      db,
      r#"UPDATE "messages" SET "image_path" = $1, "image_state" = 'stored' WHERE "chat_id" = $2 AND "message_id" = $3"#,
      vec![
        path.to_string_lossy().into_owned().into(),
        chat_id.into(),
        message_id.into(),
      ],
    )
    .await?;
  }

  if file.kind() == Some(ChatFileKind::Sticker) {
    mark_sticker(db, chat_id, message_id).await?;
  }

  Ok(true)
}

// What the message's own row says of the picture it carried, where keeping it failed.
async fn image_state(
  db: &DatabaseConnection,
  chat_id: i64,
  message_id: i64,
  state: &str,
) -> Result<(), Fault> {
  run(
    db,
    r#"UPDATE "messages" SET "image_state" = $1 WHERE "chat_id" = $2 AND "message_id" = $3"#,
    vec![state.into(), chat_id.into(), message_id.into()],
  )
  .await
  .map(drop)
}

// One file kept with its row: none where the same file already stands kept, and its row alone, without the file,
// where it could not be written.
async fn collected(
  db: &DatabaseConnection,
  data_root: &DataRoot,
  inbound: &Inbound,
  file: &impl Handed,
) -> Result<(), Fault> {
  let source = named(file);
  let standing = standing(db, inbound, file.id(), &source).await?;

  if standing.as_ref().is_some_and(Standing::kept) {
    return Ok(());
  }

  let taken = taken_paths(db, inbound, standing.as_ref().map(|row| row.id)).await?;
  let path =
    free_path(&room_path(&data_root.collected(), inbound.chat_id), inbound.message_id, &source, &taken);
  let (stored, summary) = match filed(&path, file, &data_root.spool()).await {
    Ok(()) => (Some(path), None),
    Err(why) => {
      eprintln!("bmo-receiver: the file {source} of message {} was not kept: {why}", inbound.message_id);
      (None, Some(format!("not written: {why}")))
    }
  };

  let kept = Kept {
    kind: kept_kind(file),
    source: &source,
    id: file.id(),
    stored,
    summary,
  };

  match standing {
    Some(standing) => refilled(db, standing.id, kept).await,
    None => row(db, inbound, kept).await,
  }
}

// The row a file of a message already has: the one of its id, or else one of its name that names no id.
struct Standing {
  id: i64,
  stored: Option<String>,
}

impl Standing {
  // Whether its file stands on the disk: a row whose file is gone, or never was written, is kept again.
  fn kept(&self) -> bool {
    self.stored.as_deref().is_some_and(|stored| Path::new(stored).exists())
  }
}

async fn standing(
  db: &DatabaseConnection,
  inbound: &Inbound,
  id: &str,
  source: &str,
) -> Result<Option<Standing>, Fault> {
  let Some(row) = one(
    db,
    r#"SELECT "id", "stored_path" FROM "collected_items"
       WHERE "chat_id" = $1 AND "message_id" = $2
         AND ("file_unique_id" = $3 OR ("file_unique_id" IS NULL AND "source" = $4))
       ORDER BY "file_unique_id" IS NULL, "id"
       LIMIT 1"#,
    vec![inbound.chat_id.into(), inbound.message_id.into(), id.into(), source.into()],
  )
  .await?
  else {
    return Ok(None);
  };

  Ok(Some(Standing {
    id: row.try_get("", "id")?,
    stored: row.try_get("", "stored_path")?,
  }))
}

// The paths the message's other files are filed under, so a file of the same name is filed beside them.
async fn taken_paths(
  db: &DatabaseConnection,
  inbound: &Inbound,
  own: Option<i64>,
) -> Result<Vec<String>, Fault> {
  all(
    db,
    r#"SELECT "stored_path" FROM "collected_items"
       WHERE "chat_id" = $1 AND "message_id" = $2 AND "stored_path" IS NOT NULL AND "id" <> $3"#,
    vec![inbound.chat_id.into(), inbound.message_id.into(), own.unwrap_or(-1).into()],
  )
  .await?
  .iter()
  .map(|row| row.try_get("", "stored_path").map_err(Fault::from))
  .collect()
}

// The first path of the file's name in the room's folder that none of `taken` stands under.
fn free_path(folder: &Path, message_id: i64, source: &str, taken: &[String]) -> PathBuf {
  let mut place = 1;

  loop {
    let path = folder.join(filed_name(message_id, place, source));

    if !taken.iter().any(|taken| Path::new(taken) == path) {
      return path;
    }

    place += 1;
  }
}

/// The saved recording a voice note's message carries, where its file stands on the disk.
pub async fn voice_path(db: &DatabaseConnection, inbound: &Inbound) -> Result<Option<String>, Fault> {
  let Some(row) = one(
    db,
    r#"SELECT "stored_path" FROM "collected_items"
       WHERE "chat_id" = $1 AND "message_id" = $2 AND "kind" = 'audio'
         AND "stored_path" IS NOT NULL AND "stored_path" <> ''
       ORDER BY "id" LIMIT 1"#,
    vec![inbound.chat_id.into(), inbound.message_id.into()],
  )
  .await?
  else {
    return Ok(None);
  };

  Ok(Some(row.try_get("", "stored_path")?))
}

// What one file's row records of it.
struct Kept<'a> {
  kind: &'static str,
  source: &'a str,
  id: &'a str,
  stored: Option<PathBuf>,
  summary: Option<String>,
}

fn stored_text(stored: Option<PathBuf>) -> Option<String> {
  stored.map(|path| path.to_string_lossy().into_owned())
}

// The file's row it already has, filled afresh and naming its id.
async fn refilled(db: &DatabaseConnection, row: i64, kept: Kept<'_>) -> Result<(), Fault> {
  run(
    db,
    r#"UPDATE "collected_items" SET "kind" = $1, "file_unique_id" = $2, "stored_path" = $3, "summary" = $4
       WHERE "id" = $5"#,
    vec![
      kept.kind.into(),
      kept.id.into(),
      stored_text(kept.stored).into(),
      kept.summary.into(),
      row.into(),
    ],
  )
  .await
  .map(drop)
}

async fn row(db: &DatabaseConnection, inbound: &Inbound, kept: Kept<'_>) -> Result<(), Fault> {
  run(
    db,
    r#"INSERT INTO "collected_items" ("chat_id", "message_id", "sender_name", "kind", "source", "file_unique_id", "stored_path", "summary", "collected_at")
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
       ON CONFLICT ("chat_id", "message_id", "file_unique_id")
       DO UPDATE SET "kind" = excluded."kind", "stored_path" = excluded."stored_path", "summary" = excluded."summary""#,
    vec![
      inbound.chat_id.into(),
      inbound.message_id.into(),
      inbound.sender_name.clone().into(),
      kept.kind.into(),
      kept.source.into(),
      kept.id.into(),
      stored_text(kept.stored).into(),
      kept.summary.into(),
      now_ms().into(),
    ],
  )
  .await
  .map(drop)
}

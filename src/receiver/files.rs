//! The files a message carried, kept as the chat service hands them over (docs/room-files.md, 「Message files」;
//! docs/pictures.md, 「Pictures received」). A picture is filed under its message in the picture store, brought down
//! and laid on white where it must be, its path on the message's row. Any other file is kept in the message-file
//! store with its row in `collected_items`, within the deployment's attachment size cap: a video kept as a video
//! whatever the service called it, a sound or a video with no name of its own named for what it is. Keeping a file
//! launches nothing and notifies nobody; a file that could not be kept leaves its row, which a later hand-over of the
//! same file fills, and the message stands.

use std::path::{Path, PathBuf};

use bmo_chat_client::FileItem;
use receiver_policy::roads::ROADS;
use sea_orm::DatabaseConnection;

use super::Inbound;
use super::pictures::{self, Keeping};
use crate::data_root::{DataRoot, room_path};
use crate::{Fault, now_ms, one, run};

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

// A type as its first part reads, without its parameters or its case.
fn bare_type(mime: &str) -> String {
  mime.split(';').next().unwrap_or_default().trim().to_lowercase()
}

/// The name a file goes by: its own, or for a sound or a video handed over with none, what it is with the extension
/// its type implies.
pub fn named(file: &FileItem) -> String {
  if !file.name.trim().is_empty() {
    return file.name.clone();
  }

  let gathering = &ROADS.gathering;
  let mime = bare_type(&file.mime);

  match kind_of(&file.mime) {
    "video" => format!(
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
// two messages' files of one name stand apart.
fn filed_name(message_id: i64, name: &str) -> String {
  let own = Path::new(name)
    .file_name()
    .map(|own| own.to_string_lossy().into_owned())
    .filter(|own| !own.is_empty() && !own.starts_with('.'))
    .unwrap_or_else(|| "file".to_owned());

  format!("{message_id}-{own}")
}

async fn written(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
  if let Some(folder) = path.parent() {
    tokio::fs::create_dir_all(folder).await?;
  }

  tokio::fs::write(path, bytes).await
}

/// Every file a message carried kept: its pictures in the picture store, the first of them named on the message's
/// row, and its other files in the message-file store, each with its row, none past `cap` bytes where a cap is set.
pub async fn keep(
  db: &DatabaseConnection,
  data_root: &DataRoot,
  cap: Option<u64>,
  inbound: &Inbound,
  files: &[FileItem],
) -> Result<(), Fault> {
  let mut pictures = 0;

  for file in files {
    if kind_of(&file.mime) == "image" {
      if picture(db, data_root, inbound, file, pictures).await? {
        pictures += 1;
      }

      continue;
    }

    collected(db, data_root, cap, inbound, file).await?;
  }

  Ok(())
}

// One picture filed under its message, the first as `<message>.jpg` and its path on the row: whether it was kept.
async fn picture(
  db: &DatabaseConnection,
  data_root: &DataRoot,
  inbound: &Inbound,
  file: &FileItem,
  before: usize,
) -> Result<bool, Fault> {
  let name = match before {
    0 => format!("{}.jpg", inbound.message_id),
    n => format!("{}-{n}.jpg", inbound.message_id),
  };
  let path = room_path(&data_root.images(), inbound.chat_id).join(name);
  let filed = &ROADS.pictures;
  let keeping = Keeping {
    threshold_bytes: filed.kept_threshold_bytes,
    max_dimension: filed.kept_max_dimension,
    quality: filed.kept_quality,
  };
  let bytes = file.bytes.clone();
  let kept = tokio::task::spawn_blocking(move || pictures::kept(bytes, keeping))
    .await
    .map_err(|why| Fault::new(format!("the picture's redrawing stopped: {why}")))?;

  if let Err(why) = written(&path, &kept).await {
    eprintln!("bmo-receiver: the picture of message {} was not kept: {why}", inbound.message_id);
    return Ok(false);
  }

  if before == 0 {
    run(
      db,
      r#"UPDATE "messages" SET "image_path" = $1 WHERE "chat_id" = $2 AND "message_id" = $3"#,
      vec![
        path.to_string_lossy().into_owned().into(),
        inbound.chat_id.into(),
        inbound.message_id.into(),
      ],
    )
    .await?;
  }

  Ok(true)
}

// One file kept with its row: none where the same file already stands kept, and its row alone, without the file,
// where it is past the cap or could not be written.
async fn collected(
  db: &DatabaseConnection,
  data_root: &DataRoot,
  cap: Option<u64>,
  inbound: &Inbound,
  file: &FileItem,
) -> Result<(), Fault> {
  let source = named(file);

  if kept_already(db, inbound, &source).await? {
    return Ok(());
  }

  let size = u64::try_from(file.bytes.len()).unwrap_or(u64::MAX);
  let (stored, summary) = match cap.filter(|cap| size > *cap) {
    Some(cap) => (None, Some(format!("past the attachment size cap of {cap} bytes"))),
    None => {
      let path =
        room_path(&data_root.collected(), inbound.chat_id).join(filed_name(inbound.message_id, &source));

      match written(&path, &file.bytes).await {
        Ok(()) => (Some(path), None),
        Err(why) => {
          eprintln!(
            "bmo-receiver: the file {source} of message {} was not kept: {why}",
            inbound.message_id
          );
          (None, Some(format!("not written: {why}")))
        }
      }
    }
  };

  row(db, inbound, kind_of(&file.mime), &source, stored, summary).await
}

// Whether the room's store already keeps this file of the message: its row names it and its file stands. A row
// whose file is gone, or never was written, is kept again. The protocol hands over no identifier of a file's own, so
// the bytes standing under a name stand for the file of that name.
async fn kept_already(db: &DatabaseConnection, inbound: &Inbound, source: &str) -> Result<bool, Fault> {
  let stored: Option<String> = one(
    db,
    r#"SELECT "stored_path" FROM "collected_items" WHERE "chat_id" = $1 AND "message_id" = $2 AND "source" = $3"#,
    vec![inbound.chat_id.into(), inbound.message_id.into(), source.into()],
  )
  .await?
  .map(|row| row.try_get::<Option<String>>("", "stored_path"))
  .transpose()?
  .flatten();

  Ok(stored.is_some_and(|stored| Path::new(&stored).exists()))
}

async fn row(
  db: &DatabaseConnection,
  inbound: &Inbound,
  kind: &str,
  source: &str,
  stored: Option<PathBuf>,
  summary: Option<String>,
) -> Result<(), Fault> {
  run(
    db,
    r#"INSERT INTO "collected_items" ("chat_id", "message_id", "sender_name", "kind", "source", "stored_path", "summary", "collected_at")
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
       ON CONFLICT ("chat_id", "message_id", "source")
       DO UPDATE SET "kind" = excluded."kind", "stored_path" = excluded."stored_path", "summary" = excluded."summary""#,
    vec![
      inbound.chat_id.into(),
      inbound.message_id.into(),
      inbound.sender_name.clone().into(),
      kind.into(),
      source.into(),
      stored.map(|path| path.to_string_lossy().into_owned()).into(),
      summary.into(),
      (now_ms() / 1000).into(),
    ],
  )
  .await
  .map(drop)
}

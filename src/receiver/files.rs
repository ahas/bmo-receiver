//! The files a message carried, kept as the chat service hands them over (docs/room-files.md, 「Message files」;
//! docs/pictures.md, 「Pictures received」): a picture in the picture store, its path on the message's row, and any
//! other file in the message-file store with its row in `collected_items`. Keeping a file launches nothing and
//! notifies nobody; a file that could not be written is said and leaves the message standing.

use std::path::{Path, PathBuf};

use bmo_chat_client::FileItem;
use sea_orm::DatabaseConnection;

use super::Inbound;
use crate::data_root::{DataRoot, room_path};
use crate::{Fault, now_ms, run};

/// What a kept file is, as its row names it.
pub fn kind_of(mime: &str) -> &'static str {
  match mime.split('/').next().unwrap_or_default() {
    "image" => "image",
    "audio" => "audio",
    "video" => "video",
    _ => "document",
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

// A picture's extension, by its type.
fn picture_extension(mime: &str) -> &str {
  match mime.split('/').nth(1).unwrap_or_default() {
    "jpeg" | "jpg" | "" => "jpg",
    other => other,
  }
}

async fn written(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
  if let Some(folder) = path.parent() {
    tokio::fs::create_dir_all(folder).await?;
  }

  tokio::fs::write(path, bytes).await
}

/// Every file a message carried kept: its pictures in the picture store, the first of them named on the message's
/// row, and its other files in the message-file store, each with its row.
pub async fn keep(
  db: &DatabaseConnection,
  data_root: &DataRoot,
  inbound: &Inbound,
  files: &[FileItem],
) -> Result<(), Fault> {
  let mut pictures = 0;

  for file in files {
    if kind_of(&file.mime) == "image" {
      let name = match pictures {
        0 => format!("{}.{}", inbound.message_id, picture_extension(&file.mime)),
        n => format!("{}-{n}.{}", inbound.message_id, picture_extension(&file.mime)),
      };
      let path = room_path(&data_root.images(), inbound.chat_id).join(name);

      if let Err(why) = written(&path, &file.bytes).await {
        eprintln!("bmo-receiver: the picture of message {} was not kept: {why}", inbound.message_id);
        continue;
      }

      if pictures == 0 {
        picture_named(db, inbound, &path).await?;
      }

      pictures += 1;
      continue;
    }

    let path =
      room_path(&data_root.collected(), inbound.chat_id).join(filed_name(inbound.message_id, &file.name));
    let stored = match written(&path, &file.bytes).await {
      Ok(()) => Some(path),
      Err(why) => {
        eprintln!(
          "bmo-receiver: the file {} of message {} was not kept: {why}",
          file.name, inbound.message_id
        );
        None
      }
    };

    collected(db, inbound, kind_of(&file.mime), &file.name, stored).await?;
  }

  Ok(())
}

async fn picture_named(db: &DatabaseConnection, inbound: &Inbound, path: &Path) -> Result<(), Fault> {
  run(
    db,
    r#"UPDATE "messages" SET "image_path" = $1 WHERE "chat_id" = $2 AND "message_id" = $3"#,
    vec![
      path.to_string_lossy().into_owned().into(),
      inbound.chat_id.into(),
      inbound.message_id.into(),
    ],
  )
  .await
  .map(drop)
}

// One file's row: where it was kept, or none where it could not be, which a later hand-over may fill.
async fn collected(
  db: &DatabaseConnection,
  inbound: &Inbound,
  kind: &str,
  source: &str,
  stored: Option<PathBuf>,
) -> Result<(), Fault> {
  run(
    db,
    r#"INSERT INTO "collected_items" ("chat_id", "message_id", "sender_name", "kind", "source", "stored_path", "collected_at")
       VALUES ($1, $2, $3, $4, $5, $6, $7)
       ON CONFLICT ("chat_id", "message_id", "source") DO UPDATE SET "stored_path" = COALESCE(excluded."stored_path", "collected_items"."stored_path")"#,
    vec![
      inbound.chat_id.into(),
      inbound.message_id.into(),
      inbound.sender_name.clone().into(),
      kind.into(),
      source.into(),
      stored.map(|path| path.to_string_lossy().into_owned()).into(),
      (now_ms() / 1000).into(),
    ],
  )
  .await
  .map(drop)
}

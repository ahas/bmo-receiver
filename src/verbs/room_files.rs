//! What a room keeps stays in that room (docs/room-files.md). Each store keeps a folder per room, named by the
//! room's chat id: a file in one room's folder never goes into another room, and a job sends nothing any
//! room's folder holds, since a job's send is a check and a check is made of material the job made itself.

use std::path::{Component, Path, PathBuf};

use crate::data_root::DataRoot;

/// The stores that keep files by room under a data root: the message-file store, the picture store and the
/// outboxes, the first two where the environment names them elsewhere.
pub fn room_stores(data_root: &DataRoot) -> Vec<PathBuf> {
  vec![data_root.collected(), data_root.images(), data_root.outboxes()]
}

/// Why a file named by its path may not be sent into a room, or none where it may. Links are followed first,
/// so a link into a room's folder is that room's file wherever the link stands.
pub fn room_file_refusal(path: &str, chat_id: i64, stores: &[PathBuf], job: bool) -> Option<String> {
  let file = real_or_resolved(Path::new(path));

  for store in stores {
    let Ok(inside) = file.strip_prefix(real_or_resolved(store)) else { continue };
    let mut parts = inside.components();
    let Some(Component::Normal(room)) = parts.next() else { continue };
    let room = room.to_string_lossy();
    let in_room = parts.next().is_some();

    if job {
      let kept = if in_room {
        format!("room {room} keeps")
      } else {
        "a store of room files holds".to_owned()
      };

      return Some(format!(
        "{path} is a file {kept}, and a job sends none: a check is made of material the job makes itself — a short line it synthesises, a picture it draws, a file it writes for the check (docs/room-files.md)."
      ));
    }

    if in_room && room != chat_id.to_string() {
      return Some(format!(
        "{path} is kept for room {room}, and a room's file never goes into another room (chat {chat_id})."
      ));
    }
  }

  None
}

// A path as it really stands, every link followed, or made absolute where nothing stands there.
fn real_or_resolved(path: &Path) -> PathBuf {
  std::fs::canonicalize(path)
    .unwrap_or_else(|_| std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()))
}

//! The receiver's folders under the data root (`docs/store.md`, 「The data root's folders」): the stores that
//! keep a folder per room, named by the room's number.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::room_record::{Facing, Found, RoomHolding};

// Each store's folder for a room, where it stands, declared as the room's so its removal takes it.
fn collected_of<'a>(facing: &'a Facing<'a>) -> Found<'a> {
  Box::pin(async move { Ok(room_folder(&facing.receiver.data_root().collected(), facing.room)) })
}

fn images_of<'a>(facing: &'a Facing<'a>) -> Found<'a> {
  Box::pin(async move { Ok(room_folder(&facing.receiver.data_root().images(), facing.room)) })
}

fn outbox_of<'a>(facing: &'a Facing<'a>) -> Found<'a> {
  Box::pin(async move { Ok(room_folder(&facing.receiver.data_root().outboxes(), facing.room)) })
}

crate::room_holding!(RoomHolding::Folder {
  what: "the files the room's messages carried",
  of: collected_of,
  removes: None,
  named: false,
});

crate::room_holding!(RoomHolding::Folder {
  what: "the pictures read off the room's messages",
  of: images_of,
  removes: None,
  named: false,
});

crate::room_holding!(RoomHolding::Folder {
  what: "the files an agent made for the room",
  of: outbox_of,
  removes: None,
  named: false,
});

/// The data root the configuration names, and the stores under it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataRoot {
  root: PathBuf,
}

impl DataRoot {
  pub fn new(root: impl Into<PathBuf>) -> Self {
    Self { root: root.into() }
  }

  pub fn path(&self) -> &Path {
    &self.root
  }

  /// The message-file store: the folder `BMO_COLLECT_DIR` names, the data root's own otherwise.
  pub fn collected(&self) -> PathBuf {
    moved(std::env::var_os("BMO_COLLECT_DIR")).unwrap_or_else(|| self.root.join("collected"))
  }

  /// The picture store: the folder `BMO_IMAGE_DIR` names, the data root's own otherwise.
  pub fn images(&self) -> PathBuf {
    moved(std::env::var_os("BMO_IMAGE_DIR")).unwrap_or_else(|| self.root.join("images"))
  }

  /// The profile pictures kept, under `avatars/room` and `avatars/person` (`docs/pictures.md`, 「Profile pictures」).
  pub fn avatars(&self) -> PathBuf {
    self.root.join("avatars")
  }

  /// The outboxes, one folder a room.
  pub fn outboxes(&self) -> PathBuf {
    self.root.join("outbox")
  }

  /// The outbox of one room.
  pub fn outbox(&self, room: i64) -> PathBuf {
    room_path(&self.outboxes(), room)
  }

  /// A room's folder in each of the stores, where it stands.
  pub fn room_folders(&self, room: i64) -> Vec<PathBuf> {
    [self.collected(), self.images(), self.outboxes()]
      .iter()
      .flat_map(|store| room_folder(store, room))
      .collect()
  }
}

// A folder a variable names in place of the data root's own; a variable set empty names none.
fn moved(named: Option<OsString>) -> Option<PathBuf> {
  named.filter(|named| !named.is_empty()).map(PathBuf::from)
}

/// The folder named by a room's number under a store, whether or not it stands.
pub fn room_path(store: &Path, room: i64) -> PathBuf {
  store.join(room.to_string())
}

/// The folder named by a room's number under a store, where it stands; finding it makes nothing.
pub fn room_folder(store: &Path, room: i64) -> Vec<PathBuf> {
  let path = room_path(store, room);

  if path.exists() { vec![path] } else { vec![] }
}

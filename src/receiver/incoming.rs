//! A file a form of the console carries, written to the disk as its bytes arrive rather than held whole in memory
//! (docs/api.md, 「Sends」; docs/store.md, 「The data root's folders」). It stands under `incoming/` for as long as
//! the request holds it and is removed with it; keeping it files its bytes under its message by a link to them, or
//! a copy where the message-file store stands on another disk (docs/room-files.md, 「Message files」).

use std::fs::Permissions;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use bmo_chat_client::ChatFileKind;
use tempfile::TempPath;
use tokio::io::AsyncWriteExt;

use super::files::Handed;
use crate::data_root::DataRoot;

/// A file being written as it arrives: what of it has come so far.
pub struct Arriving {
  file: tokio::fs::File,
  path: TempPath,
  size: u64,
}

impl Arriving {
  /// A file begun under the data root's `incoming/`, the folder made where it is missing.
  pub async fn begun(data_root: &DataRoot) -> io::Result<Self> {
    let folder = data_root.incoming();

    tokio::fs::create_dir_all(&folder).await?;

    // Filed by a link, the file keeps the mode it was begun with: the one a file the store writes itself takes.
    let begun =
      move || tempfile::Builder::new().permissions(Permissions::from_mode(0o644)).tempfile_in(folder);
    let (file, path) = tokio::task::spawn_blocking(begun).await.map_err(io::Error::other)??.into_parts();

    Ok(Self {
      file: tokio::fs::File::from_std(file),
      path,
      size: 0,
    })
  }

  /// The next bytes of the file written behind those before them.
  pub async fn taken(&mut self, chunk: &[u8]) -> io::Result<()> {
    self.file.write_all(chunk).await?;
    self.size += u64::try_from(chunk.len()).unwrap_or(u64::MAX);

    Ok(())
  }

  /// The file whole, under the name and the type the form gave it.
  pub async fn done(mut self, name: String, mime: String) -> io::Result<Incoming> {
    self.file.flush().await?;
    self.file.sync_data().await?;

    Ok(Incoming {
      name,
      mime,
      kind: None,
      path: self.path,
      size: self.size,
    })
  }
}

/// A file a form carried, whole on the disk: its name, its type, what it is sent as, where it stands and its size.
/// Its bytes are removed as it is dropped.
pub struct Incoming {
  name: String,
  mime: String,
  kind: Option<ChatFileKind>,
  path: TempPath,
  size: u64,
}

impl Incoming {
  /// The file handed over as what `kind` names, as a voice note is.
  pub fn as_kind(mut self, kind: ChatFileKind) -> Self {
    self.kind = Some(kind);

    self
  }

  pub fn size(&self) -> u64 {
    self.size
  }
}

impl Handed for Incoming {
  /// The record hands each file a form carried over as its own message's one file, identified by that place.
  fn id(&self) -> &str {
    "1"
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
    tokio::fs::read(&self.path).await
  }

  async fn filed_at(&self, path: &Path) -> io::Result<()> {
    if tokio::fs::hard_link(&self.path, path).await.is_ok() {
      return Ok(());
    }

    tokio::fs::copy(&self.path, path).await.map(drop)
  }
}

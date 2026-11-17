//! What the receiver puts into a room for an agent's own round (docs/messages.md, 「What an agent can send
//! back」): a reaction, a pin, a sticker, pictures and files, none of them carrying words, since the agent's words
//! go into a room only as its round's message; and a procedure document as one room may use it. Each answers the
//! lines it prints.

use std::path::{Path, PathBuf};

use bmo_chat_client::{
  AlbumItem, ChatSend, ChatSendRequest, ChatSticker, FileBody, FileItem, MissedPictureReason,
};
use room_calls::procedures;

use receiver_policy::files::FILES;

use crate::data_root::DataRoot;
use crate::receiver::labels;
use crate::roads::providers::FileLimit;
use crate::verbs::VerbError;
use crate::verbs::photo_repair::fetched;
use crate::verbs::room_files::{room_file_refusal, room_stores};
use crate::verbs::sends::{NotSent, Sends};

// A send pinned as a reply to the message `reply_to` names, where it names one.
fn replying(mut request: ChatSendRequest, reply_to: Option<i64>) -> ChatSendRequest {
  request.reply_to_message_id = reply_to;
  request
}

fn refused(why: NotSent) -> VerbError {
  VerbError::Refused(why.to_string())
}

/// A message pinned in a room, quietly, or unpinned.
pub async fn pin(
  sends: &Sends,
  chat_id: i64,
  message_id: i64,
  unpin: bool,
) -> Result<Vec<String>, VerbError> {
  let road = sends.road(chat_id, "pin").await?;

  sends
    .send(
      &road,
      &ChatSendRequest::new(
        chat_id,
        ChatSend::Pin {
          message_id,
          on: !unpin,
          silent: true,
        },
      ),
    )
    .await
    .map_err(refused)?;

  Ok(vec![if unpin {
    format!("unpinned message {message_id}")
  } else {
    format!("pinned message {message_id}")
  }])
}

/// An emoji reaction put on a message, or the agent's own taken off with `none`.
pub async fn react(
  sends: &Sends,
  chat_id: i64,
  message_id: i64,
  emoji: &str,
) -> Result<Vec<String>, VerbError> {
  let emoji = if emoji == "none" { String::new() } else { emoji.to_owned() };
  let road = sends.road(chat_id, "reaction").await?;

  sends
    .send(
      &road,
      &ChatSendRequest::new(
        chat_id,
        ChatSend::Reaction {
          message_id,
          emoji: emoji.clone(),
        },
      ),
    )
    .await
    .map_err(refused)?;

  Ok(vec![if emoji.is_empty() {
    format!("reaction cleared on message {message_id}")
  } else {
    format!("reacted {emoji} to message {message_id}")
  }])
}

/// One of the deployment's stickers, sent by its emoji.
pub async fn sticker(
  sends: &Sends,
  chat_id: i64,
  emoji: &str,
  reply_to: Option<i64>,
) -> Result<Vec<String>, VerbError> {
  let road = sends.road(chat_id, "sticker").await?;

  match sends
    .send(
      &road,
      &replying(
        ChatSendRequest::new(
          chat_id,
          ChatSend::Sticker(ChatSticker::Emoji {
            emoji: emoji.to_owned(),
          }),
        ),
        reply_to,
      ),
    )
    .await
  {
    Ok(_) => Ok(vec![format!("sent the {emoji} sticker to chat {chat_id}")]),
    Err(NotSent::Refused(refusal)) => {
      Err(VerbError::Refused(format!("{refusal} (--list shows the stickers there are)")))
    }
    Err(NotSent::Failed(why)) => Err(VerbError::Refused(why)),
  }
}

/// The set the deployment's stickers are sent out of: the set belongs to the chat service, which says what it
/// carries in its own words, so the listing names the road rather than a set of our own.
pub fn sticker_list() -> Result<Vec<String>, VerbError> {
  Ok(vec!["the stickers there are belong to the road the room is answered on, which says in its own words when a sticker is not one of them".to_owned()])
}

/// One picture asked for: a web address or a file this machine keeps, with the word drawn into it where one
/// is named.
#[derive(Clone, Debug)]
pub struct Picture {
  pub url: String,
  pub label: Option<String>,
}

/// One picture as it stands before a road carries it: a web address the road fetches for itself, or a file under
/// the data root, handed over from the disk so the road needs no reach into this machine's data root.
enum PictureSource {
  Web(String),
  Kept {
    name: String,
    mime: String,
    path: PathBuf,
  },
}

/// One picture, or several as one album: none that a room keeps goes into another room or out of a job, the
/// road is asked before anything is uploaded, a label is drawn in where one is named, and what did not go is
/// said by address with why. A web address goes as it stands; a file under the data root goes from the disk, read
/// only as the send carries it, since no road reaches this machine's data root. Anything else is refused before
/// anything goes. No caption goes with them: what the agent says of them is its round's message.
pub async fn photo(
  sends: &Sends,
  data_root: &DataRoot,
  chat_id: i64,
  pictures: &[Picture],
  reply_to: Option<i64>,
  job: bool,
) -> Result<Vec<String>, VerbError> {
  let pictures: Vec<(String, String)> = pictures
    .iter()
    .map(|picture| {
      (
        picture.url.trim().to_owned(),
        picture.label.as_deref().unwrap_or_default().trim().to_owned(),
      )
    })
    .filter(|(url, _)| !url.is_empty())
    .collect();

  if pictures.is_empty() {
    return Err(VerbError::Refused("a picture to send and the room to send it to".to_owned()));
  }

  let stores = room_stores(data_root);

  if let Some(refusal) = pictures.iter().find_map(|(url, _)| room_file_refusal(url, chat_id, &stores, job))
  {
    return Err(VerbError::Refused(refusal));
  }

  let road = sends.road(chat_id, "album").await?;
  let mut unlabelled: Vec<String> = vec![];
  let mut items: Vec<AlbumItem> = vec![];

  for (url, label) in &pictures {
    let source = picture_source(data_root, url).await?;

    // The path a relative address resolved to may stand in another room's store, which the first check could
    // not read from the address as it was written.
    if let PictureSource::Kept { .. } = &source
      && let Some(refusal) = resolved_room_refusal(data_root, url, chat_id, &stores, job)
    {
      return Err(VerbError::Refused(refusal));
    }

    if !label.is_empty() {
      if let Some(item) = labelled_item(&source, label).await {
        items.push(AlbumItem::File(item));
        continue;
      }

      unlabelled.push(url.clone());
    }

    items.push(match source {
      PictureSource::Web(address) => AlbumItem::Address { address },
      PictureSource::Kept { name, mime, path } => AlbumItem::File(FileItem {
        name,
        mime,
        bytes: FileBody::stored(path),
      }),
    });
  }

  let sent = sends
    .send(
      &road,
      &replying(
        ChatSendRequest::new(
          chat_id,
          ChatSend::Album {
            items,
            caption: None,
          },
        ),
        reply_to,
      ),
    )
    .await
    .map_err(refused)?;
  let mut lines: Vec<String> = vec![];

  if !sent.missed.is_empty() {
    let missed: Vec<String> = sent
      .missed
      .iter()
      .map(|picture| format!("{} — {}", picture.address, picture.reason))
      .collect();

    lines.push(format!("not sent as pictures ({}):\n{}", sent.missed.len(), missed.join("\n")));
  }

  if !unlabelled.is_empty() {
    lines.push(format!(
      "sent without the label drawn in ({}):\n{}",
      unlabelled.len(),
      unlabelled.join("\n")
    ));
  }

  if sent.missed.len() == pictures.len() {
    return Err(VerbError::Partly {
      lines,
      refusal: "no picture was sent".to_owned(),
    });
  }

  lines.push(if pictures.len() == 1 {
    "sent".to_owned()
  } else {
    format!("sent {} of {} pictures as an album", pictures.len() - sent.missed.len(), pictures.len())
  });

  Ok(lines)
}

/// One file of a file send: the path it stands at, and the name the room reads it under where one is named.
#[derive(Clone, Debug)]
pub struct SendFile {
  pub path: String,
  pub name: Option<String>,
}

// One file of this machine taken for a send: the name the room reads, its type and where it stands, read only as
// the send carries it (docs/api.md, 「Sends」).
struct TakenFile {
  name: String,
  mime: String,
  path: PathBuf,
}

impl TakenFile {
  fn body(&self) -> FileBody {
    FileBody::stored(&self.path)
  }
}

// A file taken for a send, refused where it is past the largest the room takes (docs/api.md, 「Sends」) or is no
// file; a file a room's folder holds goes only into that room and out of no job.
async fn taken_file(
  data_root: &DataRoot,
  chat_id: i64,
  path: &str,
  name: Option<&str>,
  job: bool,
  limit: Option<&FileLimit>,
) -> Result<TakenFile, VerbError> {
  if let Some(refusal) = room_file_refusal(path, chat_id, &room_stores(data_root), job) {
    return Err(VerbError::Refused(refusal));
  }

  let standing = tokio::fs::metadata(path)
    .await
    .map_err(|why| VerbError::Refused(format!("{path} is no file on this machine: {why}")))?;

  if !standing.is_file() {
    return Err(VerbError::Refused(format!("{path} is no file on this machine")));
  }

  let length = standing.len();

  if let Some(refusal) = limit.and_then(|limit| limit.turns_away(path, length)) {
    return Err(VerbError::Refused(refusal.to_string()));
  }

  let named = name
    .map(str::trim)
    .filter(|name| !name.is_empty())
    .and_then(|name| Path::new(name).file_name())
    .or_else(|| Path::new(path).file_name())
    .map(|name| name.to_string_lossy().into_owned())
    .unwrap_or_else(|| FILES.unnamed.clone());
  let mime = mime_guess::from_path(&named)
    .first()
    .map(|kind| kind.essence_str().to_owned())
    .unwrap_or_else(|| "application/octet-stream".to_owned());

  Ok(TakenFile {
    name: named,
    mime,
    path: PathBuf::from(path),
  })
}

/// One file or several: one goes as a document, several as one album, each from this machine's disk, with no
/// caption, since what the agent says of them is its round's message.
pub async fn file(
  sends: &Sends,
  data_root: &DataRoot,
  chat_id: i64,
  files: &[SendFile],
  reply_to: Option<i64>,
  job: bool,
) -> Result<Vec<String>, VerbError> {
  let mut held: Vec<TakenFile> = vec![];
  let limit = sends.file_limit(chat_id).await?;

  for file in files {
    let path = file.path.trim();

    if !path.is_empty() {
      held.push(taken_file(data_root, chat_id, path, file.name.as_deref(), job, limit.as_ref()).await?);
    }
  }

  if held.is_empty() {
    return Err(VerbError::Refused("a file to send and the room to send it to".to_owned()));
  }

  let mut lines: Vec<String> = vec![];

  if held.len() == 1 {
    let file = held.remove(0);
    let road = sends.road(chat_id, "document").await?;

    let sent = sends
      .send(
        &road,
        &replying(
          ChatSendRequest::new(
            chat_id,
            ChatSend::Document {
              bytes: file.body(),
              file_name: file.name.clone(),
            },
          ),
          reply_to,
        ),
      )
      .await;

    sent.map_err(refused)?;
    lines.push(format!("sent {} to chat {chat_id}", file.name));
  } else {
    let road = sends.road(chat_id, "album").await?;
    let count = held.len();
    let items: Vec<AlbumItem> = held
      .into_iter()
      .map(|file| {
        AlbumItem::File(FileItem {
          bytes: file.body(),
          name: file.name,
          mime: file.mime,
        })
      })
      .collect();
    let sent = sends
      .send(
        &road,
        &replying(
          ChatSendRequest::new(
            chat_id,
            ChatSend::Album {
              items,
              caption: None,
            },
          ),
          reply_to,
        ),
      )
      .await
      .map_err(refused)?;

    if !sent.missed.is_empty() {
      let missed: Vec<String> =
        sent.missed.iter().map(|one| format!("{} — {}", one.address, one.reason)).collect();

      lines.push(format!("not sent as files ({}):\n{}", sent.missed.len(), missed.join("\n")));
    }

    lines.push(format!(
      "sent {} of {count} files as an album to chat {chat_id}",
      count - sent.missed.len()
    ));
  }

  Ok(lines)
}

// One picture resolved: a web address as it stands, or a file under the data root by where it stands. An address
// that is neither is refused with the reason the pictures policy names.
async fn picture_source(data_root: &DataRoot, address: &str) -> Result<PictureSource, VerbError> {
  if crate::web_address::is_web_address(address) {
    return Ok(PictureSource::Web(address.to_owned()));
  }

  let path =
    kept_path(data_root, address).map_err(|reason| VerbError::Refused(format!("{address}: {reason}")))?;
  let name = path
    .file_name()
    .map(|name| name.to_string_lossy().into_owned())
    .filter(|name| !name.trim().is_empty())
    .unwrap_or_else(|| FILES.unnamed.clone());
  let mime = mime_guess::from_path(&name)
    .first()
    .map(|kind| kind.essence_str().to_owned())
    .unwrap_or_else(|| "application/octet-stream".to_owned());

  Ok(PictureSource::Kept { name, mime, path })
}

// Where the file an address names stands under the data root, every link followed, or why it may not be sent.
// A relative path is read against the folder that holds the data root, then the data root itself, then the
// working folder, so `data/collected/...` names the same file however the receiver was started.
fn kept_path(data_root: &DataRoot, address: &str) -> Result<PathBuf, MissedPictureReason> {
  let path = Path::new(address);
  let root = data_root.path().canonicalize().unwrap_or_else(|_| absolute_lexical(data_root.path()));
  let candidates: Vec<PathBuf> = if path.is_absolute() {
    vec![path.to_path_buf()]
  } else {
    let mut candidates = vec![];

    if let Some(parent) = data_root.path().parent() {
      candidates.push(parent.join(path));
    }

    candidates.push(data_root.path().join(path));

    if let Ok(cwd) = std::env::current_dir() {
      candidates.push(cwd.join(path));
    }

    candidates
  };

  for candidate in candidates {
    match candidate.canonicalize() {
      Ok(real) if real.starts_with(&root) => {
        return if real.is_file() { Ok(real) } else { Err(MissedPictureReason::MissingFile) };
      }
      Ok(_) => {}
      Err(_) if absolute_lexical(&candidate).starts_with(&root) => {
        return Err(MissedPictureReason::MissingFile);
      }
      Err(_) => {}
    }
  }

  Err(if path.is_absolute() || address.contains('/') {
    MissedPictureReason::OutsideDataRoot
  } else {
    MissedPictureReason::NotAnAddress
  })
}

fn absolute_lexical(path: &Path) -> PathBuf {
  if path.is_absolute() {
    return path.to_path_buf();
  }

  std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

// The room a resolved file stands in, refused where it is another room's or a job sends it.
fn resolved_room_refusal(
  data_root: &DataRoot,
  address: &str,
  chat_id: i64,
  stores: &[PathBuf],
  job: bool,
) -> Option<String> {
  let path = kept_path(data_root, address).ok()?;

  room_file_refusal(&path.to_string_lossy(), chat_id, stores, job)
}

// A picture with its word drawn into it, as the JPEG bytes the road carries: the one picture read whole, since it
// is drawn. None where it could not be read, was no picture, or the label said nothing, so the caller sends the
// picture as it stands instead.
async fn labelled_item(source: &PictureSource, label: &str) -> Option<FileItem> {
  let bytes = match source {
    PictureSource::Web(address) => fetched(address).await.ok()?,
    PictureSource::Kept { path, .. } => tokio::fs::read(path).await.ok()?,
  };

  if bytes.is_empty() {
    return None;
  }

  let wording = label.to_owned();
  let drawn = tokio::task::spawn_blocking(move || labels::labelled(&bytes, &wording)).await.ok()?;
  let jpeg = drawn.ok()??;

  Some(FileItem {
    name: labelled_name(source),
    mime: "image/jpeg".to_owned(),
    bytes: jpeg.into(),
  })
}

fn labelled_name(source: &PictureSource) -> String {
  let name = match source {
    PictureSource::Web(address) => address.rsplit('/').next().unwrap_or_default().to_owned(),
    PictureSource::Kept { name, .. } => name.clone(),
  };
  let stem = Path::new(&name)
    .file_stem()
    .map(|stem| stem.to_string_lossy().into_owned())
    .filter(|stem| !stem.trim().is_empty())
    .unwrap_or_else(|| "picture".to_owned());

  format!("{stem}.jpg")
}

/// A procedure document as the room it is read in may use it: the sections and lines for a verb that room's
/// road does not perform left out, as `room-calls` holds a document to the verbs. A read made outside any room
/// has no road to hold the document to, and reads it whole. A name that is no procedure is refused rather than
/// read.
pub async fn procedure(
  sends: &Sends,
  tree: &Path,
  name: &str,
  chat_id: Option<i64>,
) -> Result<Vec<String>, VerbError> {
  let text = procedures::written(tree, name).map_err(VerbError::Refused)?;
  let can = match chat_id {
    Some(chat_id) => Some(sends.answering_can(chat_id).await?),
    None => None,
  };

  Ok(vec![procedures::as_used(&text, can.as_deref()).map_err(VerbError::Refused)?])
}

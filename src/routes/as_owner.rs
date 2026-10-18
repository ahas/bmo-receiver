//! What the console sends in the owner's own name into a room that is neither his private chat nor one the
//! console made (docs/boundary-api.md, 「The owner's words」; docs/receiver.md, 「The owner's words」): the words
//! and the files go on the road the room declares for a line nobody asked for, in that road's own voice, since the
//! owner's words stand without any agent's turn. A file past the ceiling a road takes a file at is turned away
//! with its name and why; the files that stand are gathered into the albums the road takes, the way a phone sends
//! several at once.

use axum::http::StatusCode;
use bmo_chat_client::{AlbumItem, ChatSend, ChatSendRequest, ChatSendResult, FileItem};

use std::path::Path;

use receiver_policy::files::FILES;
use serde_json::{Value, json};

use super::{Refusal, refused};
use crate::receiver::{Receiver, speech::NotSaid};

/// A file the owner put in on the screen: its name, its type where the browser gave one and its bytes.
#[derive(Clone, Debug)]
pub struct OwnerFile {
  pub name: String,
  pub mime: String,
  pub bytes: Vec<u8>,
}

/// What a send of the owner's files came to: what went, each as its name and the number it went out as; what the
/// road is still drawing; what was turned away, each with why; and the refusal of a send none of it went at all.
pub struct FilesSent {
  pub sent: Vec<Value>,
  pub checking: Vec<Value>,
  pub failed: Vec<Value>,
  pub refusal: Option<String>,
}

/// What the road makes of a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
  Picture,
  Video,
  Sound,
  Document,
}

/// A file's kind: by its type where the browser gave one, and by the extension of its name where it did not.
pub fn kind_of(file: &OwnerFile) -> Kind {
  let given = file.mime.trim().to_lowercase();
  let named = Path::new(&file.name)
    .extension()
    .map(|held| held.to_string_lossy().into_owned())
    .unwrap_or_default()
    .to_lowercase();
  let guessed = if given.is_empty() { named } else { given };

  match guessed.as_str() {
    "mp4" | "mov" | "webm" | "avi" | "mkv" => Kind::Video,
    "mp3" | "wav" | "ogg" | "m4a" | "flac" | "aac" => Kind::Sound,
    "jpg" | "jpeg" | "png" | "gif" | "webp" | "heic" | "heif" | "tif" | "tiff" | "bmp" => Kind::Picture,
    other if other.starts_with("video/") => Kind::Video,
    other if other.starts_with("audio/") => Kind::Sound,
    other if other.starts_with("image/") => Kind::Picture,
    _ => Kind::Document,
  }
}

// Which files stand together in one album: pictures and videos, sounds; a document stands as a file of its own.
fn album_of(kind: Kind) -> Option<u8> {
  match kind {
    Kind::Picture | Kind::Video => Some(0),
    Kind::Sound => Some(1),
    Kind::Document => None,
  }
}

/// Why a file is turned away before anything is sent: past the ceiling every road takes a file at.
pub fn refusal(name: &str, bytes: u64) -> Option<String> {
  (bytes > FILES.largest_bytes)
    .then(|| format!("{name} is {bytes} bytes, past the {} a road takes a file at", FILES.largest_bytes))
}

/// The caption the road the room is on takes, where it says one: words past it go ahead of the files as the
/// owner's own message.
pub async fn caption_limit(receiver: &Receiver, chat_id: i64) -> Result<Option<u64>, Refusal> {
  let Some(road) = receiver.roads().unasked(chat_id).await else { return Ok(None) };
  let Some(declared) = receiver.roads().declaration(road.key()).await else {
    return Ok(None);
  };

  Ok(declared.dialect.limits.caption)
}

// A road that holds no room, in the receiver's error shape.
fn no_room(chat_id: i64) -> Refusal {
  refused(
    StatusCode::CONFLICT,
    "no-route",
    &format!("no road holds the room {chat_id}, so nothing was said there"),
  )
}

// A send the road did not carry, in the receiver's error shape.
fn not_said(why: NotSaid) -> Refusal {
  refused(StatusCode::CONFLICT, "not-sent", &why.to_string())
}

/// The owner's words said in the room, on the road a line nobody asked for goes on: its message's number, where
/// the road gives one.
pub async fn say(
  receiver: &Receiver,
  chat_id: i64,
  text: &str,
  reply_to: Option<i64>,
) -> Result<Option<i64>, Refusal> {
  let Some(road) = receiver.roads().unasked(chat_id).await else {
    return Err(no_room(chat_id));
  };
  let request = ChatSendRequest::new(
    chat_id,
    ChatSend::Text {
      text: text.to_owned(),
      reply_to,
      silent: false,
      fold: true,
    },
  );

  receiver.speech().act(road.key(), &request).await.map_err(not_said)
}

/// The owner's files sent into the room, gathered into the albums the road takes: each send answered as it came,
/// the caption and the reply carried on the first of them alone.
pub async fn send_files(
  receiver: &Receiver,
  chat_id: i64,
  files: &[OwnerFile],
  caption: &str,
  reply_to: Option<i64>,
) -> Result<FilesSent, Refusal> {
  let Some(road) = receiver.roads().unasked(chat_id).await else {
    return Err(no_room(chat_id));
  };
  let key = road.key();
  let mut outcome = FilesSent {
    sent: vec![],
    checking: vec![],
    failed: vec![],
    refusal: None,
  };

  // The files that stand together in one album, in the order they were put in; a document stands as a file of its
  // own, and so stands each of its own send.
  let mut groups: Vec<(u8, Vec<OwnerFile>)> = vec![];

  for (at, file) in files.iter().enumerate() {
    let group = album_of(kind_of(file)).unwrap_or(u8::try_from(at + 2).unwrap_or(255));

    match groups.iter().position(|(held, _)| *held == group) {
      Some(held) => groups[held].1.push(file.clone()),
      None => groups.push((group, vec![file.clone()])),
    }
  }

  for (at, (_, held)) in groups.iter().enumerate() {
    let words = if at == 0 { caption.to_owned() } else { String::new() };
    let reply = if at == 0 { reply_to } else { None };
    let alone = held.iter().all(|file| album_of(kind_of(file)).is_none());
    let sends: Vec<(Vec<OwnerFile>, ChatSend)> = if alone {
      held
        .iter()
        .map(|file| {
          (
            vec![file.clone()],
            ChatSend::Document {
              bytes: file.bytes.clone(),
              file_name: file.name.clone(),
              reply_to: reply,
            },
          )
        })
        .collect()
    } else {
      vec![(
        held.clone(),
        ChatSend::Album {
          items: held
            .iter()
            .map(|file| {
              AlbumItem::File(FileItem {
                name: file.name.clone(),
                mime: file.mime.clone(),
                bytes: file.bytes.clone(),
              })
            })
            .collect(),
          caption: (!words.is_empty()).then_some(words),
          reply_to: reply,
        },
      )]
    };

    for (held, send) in sends {
      let answer = receiver
        .speech()
        .carried(key, &ChatSendRequest::new(chat_id, send))
        .await
        .map_err(not_said)?;

      match answer {
        ChatSendResult::Sent { message_id, missed } => {
          for file in held {
            outcome.sent.push(json!({ "name": file.name, "messageId": message_id }));
          }

          for one in missed {
            outcome.failed.push(json!({ "name": one.address, "reason": one.reason.to_string() }));
          }
        }
        ChatSendResult::Refused { refusal, .. } => {
          outcome.refusal.get_or_insert(refusal.to_string());

          for file in held {
            outcome.failed.push(json!({ "name": file.name, "reason": refusal.to_string() }));
          }
        }
      }
    }
  }

  Ok(outcome)
}

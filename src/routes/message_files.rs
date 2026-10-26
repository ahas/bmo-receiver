//! A message's kept files and its picture, served to the console (`docs/boundary-api.md`, 「A message's files and
//! picture」; `docs/room-files.md`, 「What the console reads of them」): the files listed in the order a page's
//! files stand, each asked for by its place among them, and the bytes served as HTTP serves a file. Only what
//! resolves inside the room's own folder of its store goes out.

use std::path::{Path as FilePath, PathBuf};

use axum::{
  extract::{Path, Query, Request, State},
  http::{HeaderValue, StatusCode, header},
  response::{IntoResponse, Response},
};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use serde::Deserialize;
use serde_json::json;
use tower::ServiceExt;
use tower_http::services::ServeFile;

use super::{Answered, Refusal, Served, answered, refused, store};
use crate::data_root::room_path;
use crate::receiver::{Receiver, reads};

// What RFC 5987 lets a header's extended value carry as it stands; every other byte is percent-encoded.
const ATTR_CHAR: &AsciiSet = &NON_ALPHANUMERIC
  .remove(b'!')
  .remove(b'#')
  .remove(b'$')
  .remove(b'&')
  .remove(b'+')
  .remove(b'-')
  .remove(b'.')
  .remove(b'^')
  .remove(b'_')
  .remove(b'`')
  .remove(b'|')
  .remove(b'~');

#[derive(Deserialize)]
pub struct Asked {
  #[serde(default)]
  download: bool,
}

fn nothing_held(what: &str) -> Refusal {
  refused(StatusCode::NOT_FOUND, "not-found", what)
}

/// A stored path as it really stands, every link followed, where it is a file inside the folder given.
pub(super) fn inside(folder: &FilePath, stored: &str) -> Option<PathBuf> {
  let folder = std::fs::canonicalize(folder).ok()?;
  let real = std::fs::canonicalize(stored).ok()?;

  (real.starts_with(&folder) && real.is_file()).then_some(real)
}

/// The content type a file's name implies, where it implies one.
pub(super) fn type_of(name: &str) -> Option<String> {
  mime_guess::from_path(name).first().map(|kind| kind.essence_str().to_owned())
}

// The files a message the record holds carried, in a page's order.
async fn kept_files(receiver: &Receiver, chat: i64, message: i64) -> Result<Vec<reads::KeptFile>, Refusal> {
  let db = receiver.db();

  if reads::message_of(db, chat, message).await.map_err(store)?.is_none() {
    return Err(nothing_held(&format!("The receiver keeps no message {message} in room {chat}.")));
  }

  reads::files(db, chat, message).await.map_err(store)
}

/// A message's files, each by its place among them, with its name, type, size and whether its bytes stand.
pub async fn files(State(served): State<Served>, Path((chat, message)): Path<(i64, i64)>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let folder = room_path(&receiver.data_root().collected(), chat);
  let listed: Vec<_> = kept_files(&receiver, chat, message)
    .await?
    .iter()
    .enumerate()
    .map(|(place, file)| {
      let standing = file.stored_path.as_deref().and_then(|stored| inside(&folder, stored));
      let named = file.stored_path.as_deref().unwrap_or(&file.source);

      json!({
        "place": place,
        "kind": file.kind,
        "name": file.source,
        "type": type_of(named),
        "size": standing.as_ref().and_then(|real| real.metadata().ok()).map(|held| held.len()),
        "kept": standing.is_some(),
      })
    })
    .collect();

  answered(json!({ "items": listed, "more": false }))
}

/// The bytes of the file at a place among a message's files.
pub async fn file(
  State(served): State<Served>,
  Path((chat, message, place)): Path<(i64, i64, usize)>,
  Query(asked): Query<Asked>,
  request: Request,
) -> Result<Response, Refusal> {
  let receiver: Receiver = (*served.receiver).clone();
  let kept = kept_files(&receiver, chat, message).await?;
  let Some(file) = kept.get(place) else {
    return Err(nothing_held(&format!(
      "Message {message} in room {chat} carried no file at place {place}."
    )));
  };
  let folder = room_path(&receiver.data_root().collected(), chat);
  let Some(real) = file.stored_path.as_deref().and_then(|stored| inside(&folder, stored)) else {
    return Err(nothing_held(&format!("The bytes of file {place} of message {message} are not kept.")));
  };
  let bytes = served_file(&real, request).await;

  Ok(if asked.download { saved_as(bytes, &file.source) } else { bytes })
}

/// The bytes of the picture a message stored.
pub async fn picture(
  State(served): State<Served>,
  Path((chat, message)): Path<(i64, i64)>,
  request: Request,
) -> Result<Response, Refusal> {
  let receiver: Receiver = (*served.receiver).clone();
  let page = reads::message_of(receiver.db(), chat, message).await.map_err(store)?;
  let folder = room_path(&receiver.data_root().images(), chat);
  let Some(real) = page.and_then(|page| page.image_path).and_then(|stored| inside(&folder, &stored)) else {
    return Err(nothing_held(&format!(
      "The receiver keeps no picture of message {message} in room {chat}."
    )));
  };

  Ok(served_file(&real, request).await)
}

#[derive(Deserialize, Default)]
pub struct AvatarCursor {
  #[serde(default)]
  after: Option<i64>,

  #[serde(default)]
  limit: Option<i64>,
}

// A page of kept pictures, and the largest one a caller asks for.
const AVATAR_PAGE: i64 = 100;
const AVATAR_MAX_PAGE: i64 = 1_000;

/// The profile pictures the archive keeps, oldest taken first (`docs/pictures.md`, 「Profile pictures」).
pub async fn avatars(State(served): State<Served>, Query(asked): Query<AvatarCursor>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let bound = i64::min(asked.limit.unwrap_or(AVATAR_PAGE).max(1), AVATAR_MAX_PAGE) + 1;
  let mut kept = reads::avatars(receiver.db(), asked.after, bound).await.map_err(store)?;
  let more = i64::try_from(kept.len()).unwrap_or(0) == bound;

  if more {
    kept.pop();
  }

  let items: Vec<serde_json::Value> = kept
    .iter()
    .map(|avatar| json!({ "kind": avatar.kind, "id": avatar.subject_id, "takenAt": avatar.taken_at }))
    .collect();

  answered(json!({ "items": items, "more": more }))
}

/// The bytes of a room's kept profile picture.
pub async fn room_avatar(
  State(served): State<Served>,
  Path(room): Path<i64>,
  request: Request,
) -> Result<Response, Refusal> {
  avatar_bytes(&served, "room", room, request).await
}

/// The bytes of a person's kept profile picture.
pub async fn person_avatar(
  State(served): State<Served>,
  Path(person): Path<i64>,
  request: Request,
) -> Result<Response, Refusal> {
  avatar_bytes(&served, "person", person, request).await
}

// A kept picture served from the data root's avatars folder alone.
async fn avatar_bytes(
  served: &Served,
  kind: &str,
  subject: i64,
  request: Request,
) -> Result<Response, Refusal> {
  let receiver: Receiver = (*served.receiver).clone();
  let kept = reads::avatar_of(receiver.db(), kind, subject).await.map_err(store)?;
  let folder = receiver.data_root().avatars();
  let Some(real) = kept.and_then(|avatar| avatar.path).and_then(|stored| inside(&folder, &stored)) else {
    return Err(nothing_held(&format!("The receiver keeps no picture of {kind} {subject}.")));
  };

  Ok(served_file(&real, request).await)
}

// A file served as HTTP serves one: its type, its length, and the range a request asks for.
async fn served_file(path: &FilePath, request: Request) -> Response {
  match ServeFile::new(path).oneshot(request).await {
    Ok(served) => served.into_response(),
    Err(never) => match never {},
  }
}

// A served file marked to be saved under its own name rather than opened.
fn saved_as(mut served: Response, name: &str) -> Response {
  let disposition = format!("attachment; filename*=UTF-8''{}", utf8_percent_encode(name, ATTR_CHAR));

  if served.status().is_success()
    && let Ok(value) = HeaderValue::from_str(&disposition)
  {
    served.headers_mut().insert(header::CONTENT_DISPOSITION, value);
  }

  served
}

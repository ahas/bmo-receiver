//! A room's words as the default answerer's session reads them (docs/bmo-window.md, 「What every turn carries」;
//! docs/pictures.md, 「Pictures received」): one line to a send in the room shape — the instant, the message's
//! number, its sender with the owner's tag, its words and a bracketed note for each thing it carries: a picture,
//! a file, the album it went out in, the message it replies to and that message's picture.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::js;
use bmo_chat_client::ChatMediaRequest;
use chrono::DateTime;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::fill_marks;
use receiver_policy::roads::ROADS;

use crate::Fault;
use crate::receiver::{Inbound, Receiver, files, rows};
use crate::secrets;
use crate::store::settings::get;
use crate::{all, one};

/// An instant as the room's transcript writes one: ISO 8601 in UTC, with its milliseconds.
pub fn iso_instant(milliseconds: i64) -> String {
  DateTime::from_timestamp_millis(milliseconds)
    .unwrap_or_default()
    .format("%Y-%m-%dT%H:%M:%S%.3fZ")
    .to_string()
}

/// A message's instant as its line writes one: the month and day, the hour and minute, in UTC.
pub fn clock(milliseconds: i64) -> String {
  DateTime::from_timestamp_millis(milliseconds)
    .unwrap_or_default()
    .format("%m-%d %H:%MZ")
    .to_string()
}

/// One send: the message its line stands under, and the other messages of its album.
pub struct Send<'a> {
  pub message: &'a Inbound,
  pub rest: Vec<&'a Inbound>,
}

/// The messages as the sends they went out in: the rows of one album stand under its first row the list holds.
pub fn sends(messages: &[Inbound]) -> Vec<Send<'_>> {
  let mut sends: Vec<Send<'_>> = Vec::new();
  let mut albums: HashMap<i64, usize> = HashMap::new();

  for message in messages {
    if let Some(album) = message.album
      && let Some(at) = albums.get(&album)
    {
      sends[*at].rest.push(message);
      continue;
    }

    if let Some(album) = message.album {
      albums.insert(album, sends.len());
    }

    sends.push(Send {
      message,
      rest: vec![],
    });
  }

  sends
}

/// What one send said: the words any of its messages carried, the bare picture mark left out wherever one of
/// them carried words.
pub fn send_words(send: &Send<'_>) -> String {
  if send.rest.is_empty() {
    return send.message.text.clone();
  }

  let mark = ROADS.marks.picture.as_str();
  let said: Vec<&str> = std::iter::once(send.message)
    .chain(send.rest.iter().copied())
    .map(|message| js::trim(&message.text))
    .filter(|text| !text.is_empty() && *text != mark)
    .collect();

  if said.is_empty() { send.message.text.clone() } else { said.join("\n") }
}

/// The note naming the other messages a send went out with.
pub fn album_note(send: &Send<'_>) -> Option<String> {
  (!send.rest.is_empty()).then(|| {
    let ids: Vec<String> = send.rest.iter().map(|message| format!("#{}", message.message_id)).collect();

    fill_marks(&CHAT_ROUND.shape.album, &[("ids", &ids.join(", "))])
  })
}

/// A quoted message on its way into a one-line note: its line breaks gone, cut at the limit with a mark.
pub fn clip_quote(text: &str) -> String {
  let mut flat = String::new();
  let mut spaced = false;

  for character in text.chars() {
    if js::is_space(character) {
      spaced = true;
    } else {
      if spaced && !flat.is_empty() {
        flat.push(' ');
      }

      spaced = false;
      flat.push(character);
    }
  }

  let shape = &CHAT_ROUND.shape;

  if js::units(&flat) <= shape.quote_limit {
    return flat;
  }

  let units: Vec<u16> = flat.encode_utf16().take(shape.quote_limit).collect();

  format!("{}{}", String::from_utf16_lossy(&units), shape.quote_cut)
}

/// One message's line in the room shape.
pub struct ShapeLine<'a> {
  pub sent_at: i64,
  pub message_id: i64,
  pub sender_name: &'a str,
  pub owner: bool,
  pub text: &'a str,
  pub notes: &'a [String],
}

pub fn shape_line(line: &ShapeLine<'_>) -> String {
  let shape = &CHAT_ROUND.shape;
  let sender = if line.sender_name.is_empty() { shape.unknown.as_str() } else { line.sender_name };
  let tag = if line.owner { shape.owner_tag.as_str() } else { "" };
  let notes = if line.notes.is_empty() { String::new() } else { format!(" {}", line.notes.join(" ")) };

  format!("[{}] #{} {}{tag}: {}{notes}", clock(line.sent_at), line.message_id, sender, line.text)
}

/// Whether the agent looks at the pictures of a room: the room's own switch, the deployment's otherwise.
pub async fn looks_in(receiver: &Receiver, chat_id: i64) -> Result<bool, Fault> {
  let row = one(
    receiver.db(),
    r#"SELECT "look" FROM "photo_chats" WHERE "chat_id" = $1"#,
    vec![chat_id.into()],
  )
  .await?;
  if let Some(row) = row {
    return Ok(row.try_get::<i64>("", "look")? == 1);
  }

  Ok(get(receiver.db(), "photos_look").await?.as_deref() == Some("on"))
}

fn image_attached(look: bool, stored: Option<&str>) -> String {
  let shape = &CHAT_ROUND.shape;

  match (look, stored) {
    (false, _) => fill_marks(&shape.image_attached_blind, &[("blind", &shape.image_blind)]),
    (true, None) => shape.image_attached_gone.clone(),
    (true, Some(path)) => fill_marks(&shape.image_attached, &[("path", path), ("look", &shape.image_look)]),
  }
}

fn image_replied(look: bool, id: i64, stored: Option<&str>) -> String {
  let shape = &CHAT_ROUND.shape;
  let id = id.to_string();

  match (look, stored) {
    (false, _) => fill_marks(&shape.image_replied_blind, &[("id", &id), ("blind", &shape.image_blind)]),
    (true, None) => fill_marks(&shape.image_replied_gone, &[("id", &id)]),
    (true, Some(path)) => {
      fill_marks(&shape.image_replied, &[("id", &id), ("path", path), ("look", &shape.image_look)])
    }
  }
}

// A path the disk still holds.
fn present(path: &str) -> bool {
  Path::new(path).exists()
}

/// What a picture fetched for a reply came to: where it now stands, that the message holds none, that no road
/// reaches the room to fetch from, or that the fetch failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReaderPicture {
  Stored(String),
  None,
  NoReader,
  Failed,
}

// Whether the room is one a chat service numbered, so its pictures are a service's to fetch.
async fn numbered_room(receiver: &Receiver, chat_id: i64) -> Result<bool, Fault> {
  Ok(
    one(
      receiver.db(),
      r#"SELECT 1 AS "one" FROM "provider_rooms" WHERE "chat_id" = $1"#,
      vec![chat_id.into()],
    )
    .await?
    .is_some(),
  )
}

/// The picture of a message fetched through the road that holds the room and kept as that message's.
async fn fetch_picture(receiver: &Receiver, target: &Inbound) -> ReaderPicture {
  let Some(key) = receiver.roads().room_road(target.chat_id).await else {
    return ReaderPicture::NoReader;
  };
  let Some(service) = receiver.roads().chats().road(&key) else {
    return ReaderPicture::NoReader;
  };
  let request = ChatMediaRequest {
    room_id: target.chat_id,
    message_id: target.message_id,
    sent_at: Some(target.sent_at),
  };

  let room = target.chat_id;

  match service.message_files(&request).await {
    Ok(page) if !page.items.is_empty() => {
      if let Err(why) = files::keep(
        receiver.db(),
        receiver.data_root(),
        receiver.deployment().attachment_bytes,
        target,
        &page.items,
      )
      .await
      {
        eprintln!("chat {room}: a reply's picture was not kept: {why}");

        return ReaderPicture::Failed;
      }

      let read = rows::message(receiver.db(), room, target.message_id, &target.via).await.ok().flatten();

      match read.as_ref().and_then(|row| row.image_path.clone()) {
        Some(path) => ReaderPicture::Stored(path.clone()),
        None => ReaderPicture::None,
      }
    }
    Ok(_) => ReaderPicture::None,
    Err(why) => {
      eprintln!("chat {room}: the road did not answer for a reply's picture: {why}");

      ReaderPicture::Failed
    }
  }
}

/// What reading a room's words needs beyond the archive: the owner, whether a reply's picture may be fetched
/// on demand, and the stretch of messages the prompt shows, a reply to one of which is pointed at rather than
/// quoted.
pub struct Reading<'a> {
  pub receiver: &'a Receiver,
  pub owner: i64,
  pub fetch_on_demand: bool,
  pub shown: &'a HashSet<i64>,
}

impl Reading<'_> {
  // The note on a reply's picture: what the archive holds, or, where the agent looks and may fetch, the room's
  // road's answer.
  async fn reply_image_note(
    &self,
    chat_id: i64,
    reply_to: i64,
    target: Option<&Inbound>,
    look: bool,
  ) -> Option<String> {
    let had_image = target.is_some_and(|target| target.image_path.is_some());
    let may_have = target.is_none()
      || had_image
      || target.is_some_and(|target| target.text.starts_with(ROADS.marks.picture.as_str()));

    if !look {
      return (target.is_some() && may_have).then(|| image_replied(false, reply_to, None));
    }

    if let Some(path) = target.and_then(|target| target.image_path.as_deref()).filter(|path| present(path))
    {
      return Some(image_replied(true, reply_to, Some(path)));
    }

    let numbered = numbered_room(self.receiver, chat_id).await.unwrap_or(false);

    if !self.fetch_on_demand || !may_have || !numbered {
      return had_image.then(|| image_replied(true, reply_to, None));
    }

    let own = self.receiver;
    let read = rows::message(own.db(), chat_id, reply_to, "").await.ok().flatten();
    let found = match target {
      Some(target) => Some(target),
      None => read.as_ref(),
    };
    let target = found?;

    let id = reply_to.to_string();
    let shape = &CHAT_ROUND.shape;

    match fetch_picture(own, target).await {
      ReaderPicture::Stored(path) => Some(image_replied(true, reply_to, Some(&path))),
      ReaderPicture::None => None,
      ReaderPicture::NoReader => Some(fill_marks(&shape.image_no_reader, &[("id", &id)])),
      ReaderPicture::Failed => Some(fill_marks(&shape.image_fetch_failed, &[("id", &id)])),
    }
  }

  /// One send as its line.
  pub async fn send_line(&self, send: &Send<'_>) -> Result<String, Fault> {
    let shape = &CHAT_ROUND.shape;
    let message = send.message;
    let mut notes = Vec::new();
    let look = looks_in(self.receiver, message.chat_id).await?;
    let mut secret_noted = false;

    for one in std::iter::once(message).chain(send.rest.iter().copied()) {
      if let Some(path) = &one.image_path {
        notes.push(image_attached(look, present(path).then_some(path.as_str())));
      } else if one.image_state.as_deref() == Some("failed") {
        notes.push(shape.image_not_stored.clone());
      }

      if let Some(state) = &one.voice_state {
        let seconds = one
          .voice_seconds
          .map(|seconds| format!("{seconds:.1}"))
          .unwrap_or_else(|| "?".to_owned());
        let transcript = one
          .transcript
          .as_deref()
          .unwrap_or_default()
          .split_whitespace()
          .collect::<Vec<_>>()
          .join(" ");

        notes.push(match state.as_str() {
          "heard" if transcript.is_empty() => fill_marks(&shape.voice_empty, &[("seconds", &seconds)]),
          "heard" => fill_marks(&shape.voice, &[("seconds", &seconds), ("transcript", &transcript)]),
          _ => shape.voice_failed.clone(),
        });
      }

      // An owner's message a secret was taken out of tells the agent so, once to a send, and never the secret
      // (docs/messages.md, 「Secrets taken out」).
      let secret_taken =
        secrets::marked(&one.text) || one.transcript.as_deref().is_some_and(secrets::marked);

      if !secret_noted && one.sender_id == Some(self.owner) && secret_taken {
        notes.push(fill_marks(&shape.secret_taken, &[("marker", secrets::marker())]));
        secret_noted = true;
      }

      let kept = all(
        self.receiver.db(),
        r#"SELECT "source", "stored_path", "summary" FROM "collected_items"
             WHERE "chat_id" = $1 AND "message_id" = $2 AND "kind" IN ('audio', 'video', 'animation', 'sticker', 'document')
               AND "source" NOT LIKE 'agent:%'
             ORDER BY "id""#,
        vec![one.chat_id.into(), one.message_id.into()],
      )
      .await?;

      for file in kept {
        let name: String = file.try_get("", "source")?;
        let stored: Option<String> = file.try_get("", "stored_path")?;
        let summary: Option<String> = file.try_get("", "summary")?;

        match &stored {
          Some(path) if present(path) => {
            notes.push(fill_marks(&shape.file_saved, &[("name", &name), ("path", path)]))
          }
          Some(_) => notes.push(fill_marks(&shape.file_not_stored, &[("name", &name)])),
          None if summary.is_some() => notes.push(fill_marks(&shape.file_not_stored, &[("name", &name)])),
          None => {}
        }
      }
    }

    notes.extend(album_note(send));

    // The replied-to message is read once and stands for both notes: what it said, then its picture.
    if let Some(reply_to) = message.reply_to_message_id {
      let target = rows::message(self.receiver.db(), message.chat_id, reply_to, &message.via).await?;
      let id = reply_to.to_string();

      notes.push(match &target {
        None => fill_marks(&shape.quoted_missing, &[("id", &id)]),
        Some(target) if self.shown.contains(&reply_to) => {
          fill_marks(&shape.quoted_shown, &[("id", &id), ("name", &target.sender_name)])
        }
        Some(target) => {
          let quote = clip_quote(&target.text);
          let when = iso_instant(target.sent_at);
          let values = [
            ("id", id.as_str()),
            ("name", target.sender_name.as_str()),
            ("when", when.as_str()),
            ("quote", quote.as_str()),
          ];

          fill_marks(if quote.is_empty() { &shape.quoted_empty } else { &shape.quoted }, &values)
        }
      });
      notes.extend(self.reply_image_note(message.chat_id, reply_to, target.as_ref(), look).await);
    }

    let text = send_words(send);

    Ok(shape_line(&ShapeLine {
      sent_at: message.sent_at,
      message_id: message.message_id,
      sender_name: &message.sender_name,
      owner: message.sender_id == Some(self.owner),
      text: &text,
      notes: &notes,
    }))
  }

  /// The messages as the session reads them, one line to a send.
  pub async fn lines(&self, messages: &[Inbound]) -> Result<Vec<String>, Fault> {
    let mut lines = Vec::new();

    for send in sends(messages) {
      lines.push(self.send_line(&send).await?);
    }

    Ok(lines)
  }
}

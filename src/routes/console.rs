//! A room's line, its pictures, its figures' window and the pinned order (`docs/api.md`, 「Rooms」 and
//! 「Messages」). A room's line is walked by a cursor the reader holds and by when each message was sent, never by
//! its number; a page holds only what a person said unless an agent's marks are asked for; and the owner's private
//! chat is read as any room is. Every room read says whose private room it is, and no account pins or unpins
//! another account's private room.

use std::collections::BTreeMap;

use axum::{
  extract::{Path, RawQuery, State},
  http::StatusCode,
};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::{Value, json};

use super::message_files::{inside, type_of};
use super::writable::Writer;
use super::{Answered, Refusal, Served, answered, query, read, refused, store};
use crate::accounts::{self, Privacy};
use crate::data_root::room_path;
use crate::receiver::listening::Standing;
use crate::receiver::reads::PictureWalk;
use crate::receiver::wake::agents;
use crate::receiver::{NotReceived, Receiver, metadata, pinned, reads, rows};
use crate::{Fault, now_ms};

// A page of a room, and the largest one a caller asks for.
const PAGE_SIZE: i64 = 100;
const MAX_PAGE: i64 = 500;

// How far a day is looked over for its first line.
const DAY_LOOK: i64 = 50;

// The opening a client sent for a person rather than words the person typed: it is left out of every page, and
// nothing is removed — the record keeps the row.
const OPENING: &str = "/start";

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MessageQuery {
  #[serde(default)]
  before: Option<i64>,

  #[serde(default)]
  after: Option<i64>,

  #[serde(default)]
  limit: Option<i64>,

  #[serde(default)]
  agent_name: Option<String>,

  /// The text the messages' words carry, for a search in the room.
  #[serde(default)]
  q: Option<String>,

  /// A moment in epoch milliseconds: only messages sent at or after it.
  #[serde(default)]
  since: Option<i64>,

  /// The messages an agent has marked as read, after `after`.
  #[serde(default)]
  marked: bool,

  /// The messages an agent has not marked yet, oldest first.
  #[serde(default)]
  unread: bool,

  /// `person`, `agent` or `any`: whose messages the page holds.
  #[serde(default)]
  by: Option<String>,
}

#[derive(Deserialize)]
pub struct DayStart {
  from: i64,
}

#[derive(Deserialize)]
pub struct PictureCursor {
  #[serde(default)]
  at: Option<i64>,

  #[serde(default)]
  before: Option<i64>,

  #[serde(default)]
  after: Option<i64>,

  #[serde(default)]
  limit: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pinning {
  room_ids: Vec<i64>,
}

#[derive(Deserialize)]
pub struct PinnedRoom {
  room: i64,
}

fn failed(why: NotReceived) -> Refusal {
  refused(StatusCode::BAD_GATEWAY, "upstream-failed", &why.to_string())
}

/// How many messages a page asks for, one more than it shows so the reader learns the gap was wider.
pub(super) fn window(limit: Option<i64>) -> i64 {
  match limit {
    None | Some(0) => PAGE_SIZE + 1,
    Some(asked) => i64::min(asked, MAX_PAGE) + 1,
  }
}

/// Every room the record shows, the pinned ones first in the owner's order, each with its name, how many
/// messages it holds, when the last was sent, the people it holds and the supervisors it names.
pub async fn rooms(State(served): State<Served>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let db = receiver.db();
  let shown = reads::shown(db).await.map_err(store)?;
  let pinned = pinned::pinned(db).await.map_err(store)?;
  let first: Vec<reads::RoomLine> = pinned
    .iter()
    .flat_map(|room| {
      shown
        .iter()
        .filter(|shown| shown.chat_id == *room)
        .cloned()
        .collect::<Vec<reads::RoomLine>>()
    })
    .collect();
  let standing: Vec<i64> = first.iter().map(|room| room.chat_id).collect();
  let listed: Vec<reads::RoomLine> = first
    .iter()
    .chain(shown.iter().filter(|room| !standing.contains(&room.chat_id)))
    .cloned()
    .collect();
  let modes = crate::roads::added::modes(db).await.map_err(store)?;
  let listen_list = rows::listen_list(db).await.map_err(store)?;
  let private = private_rooms(&receiver).await.map_err(store)?;
  let mut items: Vec<Value> = vec![];

  for room in listed {
    // The room's facts are read before its title, since an answer asked afresh carries the title its service
    // gives it now (docs/receiver.md, 「A room's title」).
    let owner_room = receiver.roads().is_owner_room(room.chat_id).await;
    let title = reads::title(db, room.chat_id).await.map_err(store)?;
    let supervisors = crate::receiver::figures::answerers(db, room.chat_id).await.map_err(store)?;
    let senders = reads::senders(db, room.chat_id).await.map_err(store)?;
    let registered = rows::is_registered(db, room.chat_id).await.map_err(store)?;
    let work = rows::KeptWorkRoom::of(room.chat_id, receiver.owner_room(), registered);
    let standing = Standing {
      owner_room: owner_room || room.chat_id == receiver.owner_room(),
      allowed: listen_list.contains(&room.chat_id),
      added: modes.contains_key(&room.chat_id),
      supervised: !supervisors.is_empty(),
      pinned: pinned.contains(&room.chat_id),
    };
    items.push(json!({
      "id": room.chat_id,
      "title": title,
      "messageCount": room.message_count,
      "lastMessageAt": room.last_message_at,
      "pinned": pinned.contains(&room.chat_id),
      "senders": senders,
      "supervisors": supervisors,
      "ownerRoom": owner_room,
      "registered": registered,
      "listened": standing.listened(),
      "workRoom": work.chat_id,
      "mode": modes.get(&room.chat_id).map(|mode| mode.as_str()),
      "privateOf": private.get(&room.chat_id).and_then(|privacy| privacy.account()),
      "sealed": private.get(&room.chat_id).is_some_and(|privacy| privacy.sealed()),
    }));
  }

  answered(json!({ "items": items, "more": false }))
}

// Every private room as the record keeps it, asking no other unit, but for the owner's private room where the record
// keeps no account of it yet: private whatever is kept, it is asked of the web unit, and read as sealed where that
// cannot say whose it is.
async fn private_rooms(receiver: &Receiver) -> Result<BTreeMap<i64, Privacy>, Fault> {
  let mut private = accounts::kept_private_rooms(receiver.db()).await?;
  let owner_room = receiver.owner_room();

  if !matches!(private.get(&owner_room), Some(Privacy::Of(_))) {
    let privacy = accounts::privacy(receiver, owner_room).await.unwrap_or(Privacy::Sealed);

    private.insert(owner_room, privacy);
  }

  Ok(private)
}

/// A room's name, as the room list gives it.
pub async fn title(State(served): State<Served>, Path(room): Path<i64>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();

  reads::title(receiver.db(), room)
    .await
    .map(|title| json!({ "title": title }))
    .map_err(store)
    .and_then(answered)
}

/// One room's line: the messages a person said, a stretch an agent has read or has not, walked by `before` or
/// `after` and answered as a page.
pub async fn messages(State(served): State<Served>, Path(room): Path<i64>, raw: RawQuery) -> Answered {
  let asked = query::<MessageQuery>(raw)?;
  let receiver: Receiver = (*served.receiver).clone();
  let bound = window(asked.limit);

  match (&asked.agent_name, asked.unread, asked.marked) {
    (Some(agent), true, _) => {
      let found = reads::unread(receiver.db(), agent, room, asked.since, bound).await.map_err(store)?;
      let more = i64::try_from(found.len()).unwrap_or(0) == bound;
      let kept = found.into_iter().take(usize::try_from(bound - 1).unwrap_or(0)).collect();

      answered(json!({ "items": listed(kept), "more": more }))
    }
    (Some(agent), _, true) => {
      let from = asked.after.unwrap_or(0);
      let found = reads::read_messages(receiver.db(), agent, room, from, bound).await.map_err(store)?;
      let more = i64::try_from(found.len()).unwrap_or(0) == bound;
      let kept = found.into_iter().take(usize::try_from(bound - 1).unwrap_or(0)).collect();

      answered(json!({ "items": listed(kept), "more": more }))
    }
    _ => {
      let searched = asked.q.as_deref().map(str::trim).filter(|text| !text.is_empty());
      let found = match (searched, asked.before, asked.after) {
        (Some(text), before, _) => reads::matching(receiver.db(), room, text, before, bound).await,
        (None, Some(before), _) => reads::before(receiver.db(), room, before, bound).await,
        (None, None, Some(after)) => reads::after(receiver.db(), room, after, bound).await,
        (None, None, None) => reads::latest(receiver.db(), room, bound).await,
      }
      .map_err(store)?;
      let more = i64::try_from(found.len()).unwrap_or(0) == bound;
      let mut kept: Vec<reads::Page> =
        if searched.is_none() && asked.after.is_some() && asked.before.is_none() {
          found.into_iter().take(usize::try_from(bound - 1).unwrap_or(0)).collect()
        } else if more {
          found.into_iter().skip(1).collect()
        } else {
          found
        };
      match asked.by.as_deref() {
        Some("any") => {}
        Some("agent") => {
          let names = agent_names(&receiver).await;
          kept.retain(|page| {
            page.sender_name == receiver.deployment().sender_name || names.contains(&page.sender_name)
          });
        }
        _ => {
          let one_to_one = receiver.roads().room_facts(room).await.one_to_one();
          kept.retain(|page| spoken_by_person(one_to_one, page));
        }
      }

      pages(&receiver, room, kept)
        .await
        .map(|items| json!({ "items": items, "more": more }))
        .map_err(store)
        .and_then(answered)
    }
  }
}

// A list of messages an agent's marks answered, each in the one message shape.
fn listed(messages: Vec<reads::Message>) -> Vec<Value> {
  messages
    .into_iter()
    .map(|message| {
      json!({
        "id": message.message_id,
        "room": message.chat_id,
        "senderId": message.sender_id,
        "senderName": message.sender_name,
        "sentAt": message.sent_at,
        "text": message.text,
        "imagePath": Value::Null,
        "replyToMessageId": message.reply_to_message_id,
        "answerSeconds": Value::Null,
        "round": Value::Null,
        "album": Value::Null,
        "via": Value::Null,
        "agentName": message.agent_name,
        "voice": message.voice,
        "kind": Value::Null,
        "editedAt": Value::Null,
        "deletedAt": Value::Null,
        "pinnedAt": Value::Null,
        "reactions": [],
        "forwardedFrom": Value::Null,
        "situation": Value::Null,
        "files": [],
      })
    })
    .collect()
}

// Every agent the agency registers, by name, for a page asked by an agent.
async fn agent_names(receiver: &Receiver) -> Vec<String> {
  receiver
    .agents()
    .await
    .map(|agents| agents.agents.iter().map(|agent| agent.name.clone()).collect())
    .unwrap_or_default()
}

/// Where a day begins in a room: the first message a person said at or after a moment.
pub async fn day_start(State(served): State<Served>, Path(room): Path<i64>, raw: RawQuery) -> Answered {
  let asked = query::<DayStart>(raw)?;
  let receiver: Receiver = (*served.receiver).clone();
  let found = reads::from(receiver.db(), room, asked.from, DAY_LOOK).await.map_err(store)?;
  let one_to_one = receiver.roads().room_facts(room).await.one_to_one();
  let begins = found
    .iter()
    .filter(|page| spoken_by_person(one_to_one, page))
    .map(|page| page.message_id)
    .collect::<Vec<i64>>();
  let start = begins.first();

  answered(json!({ "messageId": start }))
}

/// What the chat service handed over beside a message as its `metadata`, as it arrived, or null.
pub async fn metadata(State(served): State<Served>, Path((room, message)): Path<(i64, i64)>) -> Answered {
  metadata::kept(served.receiver.db(), room, message)
    .await
    .map(|kept| json!({ "metadata": kept }))
    .map_err(store)
    .and_then(answered)
}

/// A stretch of a room's pictures, oldest first, walked from a picture the reader holds, with how many of the
/// room's pictures stand before it and how many the room holds.
pub async fn pictures(State(served): State<Served>, Path(room): Path<i64>, raw: RawQuery) -> Answered {
  let asked = query::<PictureCursor>(raw)?;
  let receiver: Receiver = (*served.receiver).clone();
  let db = receiver.db();
  let bound = i64::min(asked.limit.unwrap_or(PAGE_SIZE), MAX_PAGE) + 1;
  let walk = match (asked.at, asked.before, asked.after) {
    (Some(at), _, _) => PictureWalk::At(at),
    (None, Some(before), _) => PictureWalk::Before(before),
    (None, None, Some(after)) => PictureWalk::After(after),
    (None, None, None) => PictureWalk::Newest,
  };
  let mut found = reads::pictures(db, room, walk, bound).await.map_err(store)?;
  let more = i64::try_from(found.len()).unwrap_or(0) == bound;

  // The extra row that says there is more is the one furthest from the cursor.
  if more {
    if matches!(walk, PictureWalk::After(_)) {
      found.pop();
    } else {
      found.remove(0);
    }
  }

  let one_to_one = receiver.roads().room_facts(room).await.one_to_one();
  let folder = room_path(&receiver.data_root().images(), room);
  let shown: Vec<reads::Page> =
    found.into_iter().filter(|page| spoken_by_person(one_to_one, page)).collect();
  let (before, total) = reads::pictures_counted(db, room, shown.first().map(|page| page.message_id))
    .await
    .map_err(store)?;
  let items: Vec<Value> = shown
    .iter()
    .map(|page| {
      let stored = page.image_path.as_deref().unwrap_or_default();
      let name = std::path::Path::new(stored).file_name().map(|name| name.to_string_lossy().into_owned());
      let file = inside(&folder, stored).and_then(|real| real.metadata().ok()).map(
        |held| json!({ "name": name, "bytes": held.len(), "type": name.as_deref().and_then(type_of) }),
      );

      json!({
        "id": page.message_id,
        "album": page.album,
        "senderName": page.sender_name,
        "sentAt": page.sent_at,
        "text": page.text,
        "file": file,
      })
    })
    .collect();

  answered(json!({ "items": items, "more": more, "before": before, "total": total }))
}

/// How long the default answerer's window in a room has left, in milliseconds, and when it closes, in epoch milliseconds;
/// a room that never opened one closes at no time.
pub async fn room_window(State(served): State<Served>, Path(room): Path<i64>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let until = agents::window_until(receiver.db(), room, &receiver.default_answerer())
    .await
    .map_err(failed)?;
  let closes_at = (until > 0).then_some(until);

  answered(json!({ "openForMs": i64::max(until - now_ms(), 0), "closesAt": closes_at }))
}

/// A room's situation note as it stands, with when it last changed: the note the round updates before it judges a line
/// (docs/drafting.md, 「The situation note」). A room with no note answers none and no moment.
pub async fn room_situation(State(served): State<Served>, Path(room): Path<i64>) -> Answered {
  let db = served.receiver.db();
  let held = crate::chat::situation::read_at(db, room).await.map_err(store)?;

  answered(json!(match held {
    Some((text, updated_at)) => json!({ "text": text, "updatedAt": updated_at }),
    None => json!({ "text": null, "updatedAt": null }),
  }))
}

/// The pinned order set whole, answered as it now stands; refused where it pins or unpins another account's private
/// room.
pub async fn pin(State(served): State<Served>, writer: Writer) -> Answered {
  let asked = read::<Pinning>(writer.body())?;
  let db = served.receiver.db();

  for room in &asked.room_ids {
    unheld(db, *room).await?;
  }

  let standing = pinned::pinned(db).await.map_err(store)?;
  let moved = standing
    .iter()
    .filter(|room| !asked.room_ids.contains(room))
    .chain(asked.room_ids.iter().filter(|room| !standing.contains(room)));

  for room in moved {
    writer.room(&served.receiver, *room).await?;
  }

  pinned::pin(db, &asked.room_ids)
    .await
    .map(|rooms| json!({ "roomIds": rooms }))
    .map_err(store)
    .and_then(answered)
}

/// A room stood last in the pinned order, answered as the order now stands.
pub async fn pin_one(State(served): State<Served>, writer: Writer) -> Answered {
  let asked = read::<PinnedRoom>(writer.body())?;
  let db = served.receiver.db();

  unheld(db, asked.room).await?;
  writer.room(&served.receiver, asked.room).await?;

  pinned::add(db, asked.room)
    .await
    .map(|rooms| json!({ "roomIds": rooms }))
    .map_err(store)
    .and_then(answered)
}

/// A room taken out of the pinned order, answered as the order now stands.
pub async fn unpin(State(served): State<Served>, Path(room): Path<i64>, writer: Writer) -> Answered {
  writer.room(&served.receiver, room).await?;

  pinned::remove(served.receiver.db(), room)
    .await
    .map(|rooms| json!({ "roomIds": rooms }))
    .map_err(store)
    .and_then(answered)
}

/// Refuses a room the record does not hold.
async fn unheld(db: &DatabaseConnection, room: i64) -> Result<(), Refusal> {
  if pinned::held(db, room).await.map_err(store)? {
    return Ok(());
  }

  Err(refused(
    StatusCode::NOT_FOUND,
    "not-found",
    &format!("The receiver holds no room {room}."),
  ))
}

/// Where a forwarded copy came from, in the one message shape; null on any other message.
pub fn forwarded_from(forwarded: &Option<(i64, i64, String)>) -> Value {
  match forwarded {
    Some((room, message, name)) => json!({ "room": room, "messageId": message, "senderName": name }),
    None => Value::Null,
  }
}

/// The page's messages with the files each carried kept beside it, in the one message shape.
pub(super) async fn pages(
  receiver: &Receiver,
  room: i64,
  kept: Vec<reads::Page>,
) -> Result<Vec<Value>, Fault> {
  let mut lines: Vec<Value> = vec![];

  for mut page in kept {
    page.files = reads::files(receiver.db(), room, page.message_id).await?;
    let files: Vec<Value> = page
      .files
      .iter()
      .map(|file| {
        json!({
          "kind": file.kind,
          "source": file.source,
          "storedPath": file.stored_path,
          "summary": file.summary,
          "collectedAt": file.collected_at,
        })
      })
      .collect();
    lines.push(json!({
      "id": page.message_id,
      "room": page.chat_id,
      "senderId": page.sender_id,
      "senderName": page.sender_name,
      "sentAt": page.sent_at,
      "text": page.text,
      "imagePath": page.image_path,
      "transcript": page.transcript,
      "voiceSeconds": page.voice_seconds,
      "voiceState": page.voice_state,
      "replyToMessageId": page.reply_to_message_id,
      "answerSeconds": page.answer_seconds,
      "round": page.round,
      "album": page.album,
      "via": page.via,
      "agentName": page.agent_name,
      "voice": page.voice,
      "kind": page.kind,
      "editedAt": page.edited_at,
      "deletedAt": page.deleted_at,
      "pinnedAt": page.pinned_at,
      "reactions": reads::reactions(receiver.db(), room, page.message_id).await?,
      "forwardedFrom": forwarded_from(&page.forwarded),
      "situation": situation_json(page.situation.as_ref()),
      "files": files,
    }));
  }

  Ok(lines)
}

/// A message's situation note as the round that judged it read it, none where it read none.
pub fn situation_json(situation: Option<&reads::Situation>) -> Value {
  match situation {
    Some(held) => json!({ "text": held.text, "changed": held.changed }),
    None => Value::Null,
  }
}

/// Whether a screen stands this line: the protocol's opening a client sent for a person is left out.
fn spoken_by_person(one_to_one: bool, page: &reads::Page) -> bool {
  let said = page.text.trim();

  !(one_to_one && (said == OPENING || said.starts_with(&format!("{OPENING} "))))
}

//! The console's rooms (`docs/boundary-api.md`, 「For the web unit's console」): the rooms the owner stands and the
//! order the owner pinned them in, one room's name, a message an agent filed, the room's line walked by a cursor the reader
//! holds, and how long the chat agent's window there has left.
//! A page holds only what a person said themselves; the judgements beside a page are the gate's, read there, and
//! the owner's private chat is read by `GET /owner`.

use axum::{
  body::Bytes,
  extract::{Path, Query, State},
  http::StatusCode,
};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Answered, Refusal, Served, answered, read, refused, store};
use crate::receiver::wake::agents;
use crate::receiver::{NotReceived, Receiver, pinned, reads};
use crate::{Fault, now_ms};

// A page of a room, and the largest one the console asks for.
const PAGE_SIZE: usize = 100;
const MAX_PAGE: usize = 500;

// How far a day is looked over for its first line.
const DAY_LOOK: i64 = 50;

// The opening a client sent for a person rather than words the person typed: it is left out of every page, and
// nothing is removed — the record keeps the row.
const OPENING: &str = "/start";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Paged {
  #[serde(default)]
  before: Option<i64>,

  #[serde(default)]
  limit: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AfterCursor {
  after: i64,

  #[serde(default)]
  limit: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayStart {
  from: i64,
}

#[derive(Deserialize)]
pub struct Pinning {
  rooms: Vec<i64>,
}

#[derive(Deserialize)]
pub struct PinnedRoom {
  chat: i64,
}

fn failed(why: NotReceived) -> Refusal {
  refused(StatusCode::INTERNAL_SERVER_ERROR, "failed", &why.to_string())
}

fn absent(chat: i64, message: i64) -> Refusal {
  refused(
    StatusCode::NOT_FOUND,
    "not-found",
    &format!("The receiver keeps no message {message} in room {chat} that an agent filed."),
  )
}

/// How many messages a page asks for, one more than it shows so the reader learns the gap was wider.
fn window(limit: i64) -> usize {
  let asked = if limit <= 0 {
    PAGE_SIZE
  } else {
    usize::min(usize::try_from(limit).unwrap_or(MAX_PAGE), MAX_PAGE)
  };

  asked + 1
}

/// Every room the record shows, the pinned ones first in the owner's order, each with its name, how many
/// messages it holds, when the last was sent, and the companions among those who answer there.
pub async fn rooms(State(served): State<Served>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let db = receiver.db();
  let shown = reads::shown(db).await.map_err(store)?;
  let pinned = pinned::pinned(db).await.map_err(store)?;
  let first: Vec<reads::RoomLine> = pinned
    .iter()
    .flat_map(|chat| {
      shown
        .iter()
        .filter(|room| room.chat_id == *chat)
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
  let mut lines: Vec<Value> = vec![];

  for room in listed {
    let answerers = agents::named_designated(&receiver, room.chat_id).await.map_err(failed)?;
    let companions: Vec<String> = answerers
      .iter()
      .filter(|agent| agent.companion())
      .map(|agent| agent.name.clone())
      .collect();
    let mut line = json!({
      "chatId": room.chat_id,
      "title": room.title,
      "messageCount": room.message_count,
      "lastMessageAt": room.last_message_at,
      "pinned": pinned.contains(&room.chat_id),
    });

    if !companions.is_empty() {
      line["companions"] = json!(companions);
    }
    lines.push(line);
  }

  answered(json!({ "rooms": lines }))
}

/// A room's name, as the room list gives it.
pub async fn title(State(served): State<Served>, Path(chat): Path<i64>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();

  reads::title(receiver.db(), chat)
    .await
    .map(|title| json!({ "title": title }))
    .map_err(store)
    .and_then(answered)
}

/// A message an agent filed in a room; nothing found for a person's or one the record does not hold.
pub async fn agent_message(
  State(served): State<Served>,
  Path((chat, message)): Path<(i64, i64)>,
) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let Some(kept) = reads::message_of(receiver.db(), chat, message).await.map_err(store)? else {
    return Err(absent(chat, message));
  };

  if !filed_by_agent(&receiver, &kept).await {
    return Err(absent(chat, message));
  };

  let mut whole = kept;
  whole.files = reads::files(receiver.db(), chat, message).await.map_err(store)?;

  answered(json!(whole))
}

/// One page of a room, oldest first, ending just before the cursor the reader holds.
pub async fn page(State(served): State<Served>, Path(chat): Path<i64>, query: Query<Paged>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let bound = window(query.limit);
  let found = match query.before {
    Some(before) => reads::before(receiver.db(), chat, before, i64::try_from(bound).unwrap_or(0)).await,
    None => reads::latest(receiver.db(), chat, i64::try_from(bound).unwrap_or(0)).await,
  }
  .map_err(store)?;
  let has_more = found.len() > bound - 1;
  let kept = if has_more { found.into_iter().skip(1).collect() } else { found };

  pages(&receiver, chat, kept)
    .await
    .map(|pages| json!({ "messages": pages, "hasMore": has_more }))
    .map_err(store)
    .and_then(answered)
}

/// Where a day begins in a room: the first message a person said at or after a moment.
pub async fn day_start(
  State(served): State<Served>,
  Path(chat): Path<i64>,
  query: Query<DayStart>,
) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let found = reads::from(receiver.db(), chat, query.from, DAY_LOOK).await.map_err(store)?;
  let one_to_one = receiver.roads().room_facts(chat).await.private_to.is_some();
  let begins = found
    .iter()
    .filter(|page| spoken_by_person(one_to_one, page))
    .map(|page| page.message_id)
    .collect::<Vec<i64>>();
  let start = begins.first();

  answered(json!({ "messageId": start }))
}

/// The messages of a room past a number the reader already holds, oldest first.
pub async fn after(
  State(served): State<Served>,
  Path(chat): Path<i64>,
  query: Query<AfterCursor>,
) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let bound = window(query.limit);
  let found = reads::after(receiver.db(), chat, query.after, i64::try_from(bound).unwrap_or(0))
    .await
    .map_err(store)?;
  let has_more = found.len() > bound - 1;
  let kept = if has_more { found.into_iter().take(bound - 1).collect() } else { found };

  pages(&receiver, chat, kept)
    .await
    .map(|pages| json!({ "messages": pages, "hasMore": has_more }))
    .map_err(store)
    .and_then(answered)
}

/// The one message as a page of one.
pub async fn one(State(served): State<Served>, Path((chat, message)): Path<(i64, i64)>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let none: Vec<reads::Page> = vec![];
  let Some(kept) = reads::message_of(receiver.db(), chat, message).await.map_err(store)? else {
    return answered(json!({ "messages": none, "hasMore": false }));
  };
  let one_to_one = receiver.roads().room_facts(chat).await.private_to.is_some();
  let kept_lines = if spoken_by_person(one_to_one, &kept) { vec![kept] } else { none };

  pages(&receiver, chat, kept_lines)
    .await
    .map(|pages| json!({ "messages": pages, "hasMore": false }))
    .map_err(store)
    .and_then(answered)
}

/// How long the chat agent's window in a room has left, in seconds, and when it closes; a room that never opened
/// one closes at no time.
pub async fn room_window(State(served): State<Served>, Path(chat): Path<i64>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let until = agents::window_until(receiver.db(), chat, &receiver.deployment().agent)
    .await
    .map_err(failed)?;
  let closes_at = (until > 0).then_some(until);

  answered(json!({ "openFor": i64::max(until - now_ms() / 1000, 0), "closesAt": closes_at }))
}

/// The pinned order set whole, answered as it now stands.
pub async fn pin(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<Pinning>(&body)?;
  let db = served.receiver.db();

  for room in &asked.rooms {
    unheld(db, *room).await?;
  }

  pinned::pin(db, &asked.rooms)
    .await
    .map(|rooms| json!({ "rooms": rooms }))
    .map_err(store)
    .and_then(answered)
}

/// A room stood last in the pinned order, answered as the order now stands.
pub async fn pin_one(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<PinnedRoom>(&body)?;
  let db = served.receiver.db();

  unheld(db, asked.chat).await?;

  pinned::add(db, asked.chat)
    .await
    .map(|rooms| json!({ "rooms": rooms }))
    .map_err(store)
    .and_then(answered)
}

/// A room taken out of the pinned order, answered as the order now stands.
pub async fn unpin(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<PinnedRoom>(&body)?;

  pinned::remove(served.receiver.db(), asked.chat)
    .await
    .map(|rooms| json!({ "rooms": rooms }))
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

/// The page's messages with the files each carried kept beside it.
async fn pages(receiver: &Receiver, chat: i64, kept: Vec<reads::Page>) -> Result<Vec<reads::Page>, Fault> {
  let mut lines: Vec<reads::Page> = vec![];

  for mut page in kept {
    page.files = reads::files(receiver.db(), chat, page.message_id).await?;
    lines.push(page);
  }

  Ok(lines)
}

/// Whether a screen stands this line: the protocol's opening a client sent for a person is left out.
fn spoken_by_person(one_to_one: bool, page: &reads::Page) -> bool {
  let said = page.text.trim();

  !(one_to_one && (said == OPENING || said.starts_with(&format!("{OPENING} "))))
}

/// Whether an agent filed this message: its sender is one a road gave an agent, or it stands under the name the
/// deployment files the chat agent's own words under.
async fn filed_by_agent(receiver: &Receiver, page: &reads::Page) -> bool {
  let listed = receiver.units().agency.agents().await.ok();
  let names: Vec<String> = match listed {
    Some(agents) => agents.agents.iter().map(|agent| agent.name.clone()).collect(),
    None => vec![],
  };
  let numbers = reads::agent_numbers(receiver.db(), &names).await.ok().unwrap_or(vec![]);

  page.sender_name == receiver.deployment().sender_name
    || page.sender_id.is_some_and(|sender| numbers.contains(&sender))
}

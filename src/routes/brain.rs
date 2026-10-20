//! What a room's record is read for, across every caller (`docs/api.md`, 「People」, 「Answerers」 and
//! 「Read marks」): the people a room has heard, the agents answering it, an agent's marks of what it has read,
//! the room's road, and the readings a turn is held to. Every route is a resource read in the common wire format:
//! camelCase keys, epoch milliseconds, signed 64-bit ids and one page of items and more.

use axum::extract::{Path, RawQuery, State};
use axum::http::StatusCode;
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use room_calls::promises::waits_on_happening;

use super::{Accepted, Answered, Refusal, Served, answered, query, read, refused, store};
use crate::Fault;
use crate::chat::instructions::{Queued, queue};
use crate::receiver::notices::frame;
use crate::receiver::wake::agents;
use crate::receiver::{NotReceived, Receiver, reads};

// What an owner notice's line is for: the brain writes no sentence, its agent says the fact.
const OWNER_INTENT: &str = "tell the owner what the system recorded.";

// A read of the room that could not be made because a unit it reaches did not answer.
fn reached(why: NotReceived) -> Refusal {
  refused(StatusCode::BAD_GATEWAY, "upstream-failed", &why.to_string())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CountsAsk {
  agent_name: String,

  #[serde(default)]
  since: Vec<RoomSince>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoomSince {
  room: i64,
  since: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MarkAsk {
  agent_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KeepAsk {
  agent_name: String,
  message_ids: Vec<i64>,
}

#[derive(Deserialize)]
struct RoomAsked {
  #[serde(default)]
  room: Option<i64>,
}

#[derive(Deserialize)]
struct PeopleAsked {
  #[serde(default)]
  ids: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpokeAsk {
  person: i64,
  from: i64,
  to: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OwnNumberAsk {
  agent_name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OwnerSpokeAsk {
  person: i64,
  before: i64,
  window_ms: i64,
}

#[derive(Deserialize)]
struct WaitsAsk {
  text: String,
}

#[derive(Deserialize)]
struct SenderAsk {
  #[serde(default)]
  _unused: Option<()>,
}

/// The whole numbers a comma-separated query lists.
fn whole_list(text: &str) -> Result<Vec<i64>, Refusal> {
  text
    .split(',')
    .filter(|one| !one.trim().is_empty())
    .map(|one| {
      one.trim().parse::<i64>().map_err(|_| {
        refused(
          StatusCode::BAD_REQUEST,
          "bad-request",
          &format!("`{}` is not a whole number.", one.trim()),
        )
      })
    })
    .collect()
}

/// Every room with messages past an agent's read marks, each with how many; a room named in `since` counts only
/// the messages sent at or after its moment.
pub async fn unread_counts(State(served): State<Served>, body: axum::body::Bytes) -> Answered {
  let asked = read::<CountsAsk>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();
  let since: Vec<(i64, i64)> = asked.since.iter().map(|one| (one.room, one.since / 1_000)).collect();

  reads::unread_counts(receiver.db(), &asked.agent_name, since)
    .await
    .map(|rooms| {
      json!({
        "items": rooms
          .iter()
          .map(|(room, count)| json!({ "room": room, "count": count }))
          .collect::<Vec<Value>>(),
        "more": false,
      })
    })
    .map_err(store)
    .and_then(answered)
}

/// How many of a room's messages an agent has marked, of the ones that still stand.
pub async fn read_count(State(served): State<Served>, Path(room): Path<i64>, raw: RawQuery) -> Answered {
  let asked = query::<MarkAsk>(raw)?;
  let receiver: Receiver = (*served.receiver).clone();

  reads::read_count(receiver.db(), &asked.agent_name, room)
    .await
    .map(|count| json!({ "count": count }))
    .map_err(store)
    .and_then(answered)
}

/// The room's messages marked as read into an agent's book.
pub async fn keep(
  State(served): State<Served>,
  Path(room): Path<i64>,
  body: axum::body::Bytes,
) -> Accepted {
  let asked = read::<KeepAsk>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();

  reads::keep_marks(receiver.db(), &asked.agent_name, room, &asked.message_ids)
    .await
    .map(|_| (StatusCode::NO_CONTENT, axum::Json(json!({}))))
    .map_err(store)
}

/// Every mark of an agent's book let go, off one room; how many were.
pub async fn forget(State(served): State<Served>, Path(room): Path<i64>, raw: RawQuery) -> Answered {
  let asked = query::<MarkAsk>(raw)?;
  let receiver: Receiver = (*served.receiver).clone();

  reads::forget_marks(receiver.db(), &asked.agent_name, Some(room))
    .await
    .map(|forgotten| json!({ "forgotten": forgotten }))
    .map_err(store)
    .and_then(answered)
}

/// Every room, and the agents answering it: the awake agents it names, or the default answerer where it names
/// none awake.
pub async fn all_answerers(State(served): State<Served>, raw: RawQuery) -> Answered {
  let asked = query::<RoomAsked>(raw)?;
  let receiver: Receiver = (*served.receiver).clone();
  let rooms = match asked.room {
    Some(room) => vec![room],
    None => reads::rooms(receiver.db()).await.map_err(store)?,
  };

  listed_answerers(&receiver, rooms)
    .await
    .map(|listed| json!({ "items": listed, "more": false }))
    .map_err(reached)
    .and_then(answered)
}

/// Each room with the agents answering it.
async fn listed_answerers(receiver: &Receiver, rooms: Vec<i64>) -> Result<Vec<Value>, NotReceived> {
  let mut listed: Vec<Value> = vec![];

  for room in rooms {
    listed.push(json!({ "room": room, "agents": answering(receiver, room).await? }));
  }

  Ok(listed)
}

/// The names of the agents answering a room, as the wake judges them: the awake agents it names, or the default
/// answerer where it names none awake.
async fn answering(receiver: &Receiver, room: i64) -> Result<Vec<String>, NotReceived> {
  let named = agents::named_designated(receiver, room).await?;

  if named.is_empty() {
    return Ok(vec![agents::default_designated(receiver).await?.name]);
  }

  Ok(named.iter().map(|agent| agent.name.clone()).collect())
}

/// Whether a person sent a message in a room between two moments, both included.
pub async fn spoke_between(State(served): State<Served>, Path(room): Path<i64>, raw: RawQuery) -> Answered {
  let asked = query::<SpokeAsk>(raw)?;
  let receiver: Receiver = (*served.receiver).clone();

  reads::spoke_between(receiver.db(), room, asked.person, asked.from / 1_000, asked.to / 1_000)
    .await
    .map(|spoke| json!({ "spoke": spoke }))
    .map_err(store)
    .and_then(answered)
}

/// The sender the agent's own words stand under in a room, or null.
pub async fn own_number(State(served): State<Served>, Path(room): Path<i64>, raw: RawQuery) -> Answered {
  let asked = query::<OwnNumberAsk>(raw)?;
  let receiver: Receiver = (*served.receiver).clone();
  let Some(road) = receiver.roads().room_road(room).await else {
    return answered(json!({ "number": null }));
  };
  let Some(designated) = agents::registered(&receiver, &asked.agent_name).await.map_err(reached)? else {
    return answered(json!({ "number": null }));
  };
  let identity = receiver.roads().identity_in(room).await;

  agents::own_number(&receiver, &designated, &road, identity)
    .await
    .map(|number| json!({ "number": number }))
    .map_err(reached)
    .and_then(answered)
}

/// Of those asked, the people who have stood as a sender in the room.
pub async fn spoken_in(State(served): State<Served>, Path(room): Path<i64>, raw: RawQuery) -> Answered {
  let asked = query::<PeopleAsked>(raw)?;
  let people = whole_list(&asked.ids)?;
  let receiver: Receiver = (*served.receiver).clone();

  let stood = reads::spoken_in(receiver.db(), room, &people).await.map_err(store)?;
  let mut items: Vec<Value> = vec![];

  for person in stood {
    items.push(
      json!({ "id": person, "name": reads::display_name(receiver.db(), person).await.map_err(store)? }),
    );
  }

  answered(json!({ "items": items, "more": false }))
}

/// Each person's name as their latest message went under, or null.
pub async fn names(State(served): State<Served>, raw: RawQuery) -> Answered {
  let asked = query::<PeopleAsked>(raw)?;
  let people = whole_list(&asked.ids)?;
  let receiver: Receiver = (*served.receiver).clone();

  listed_names(receiver.db(), people)
    .await
    .map(|listed| json!({ "items": listed, "more": false }))
    .map_err(store)
    .and_then(answered)
}

/// Each person asked with the name they last went by.
async fn listed_names(db: &DatabaseConnection, people: Vec<i64>) -> Result<Vec<Value>, Fault> {
  let mut listed: Vec<Value> = vec![];

  for person in people {
    listed.push(json!({ "id": person, "name": reads::display_name(db, person).await? }));
  }

  Ok(listed)
}

/// Whether the text waits for something to happen first, by the receiver's reading of promises.
pub async fn waits(State(_served): State<Served>, body: axum::body::Bytes) -> Answered {
  let asked = read::<WaitsAsk>(&body)?;

  answered(json!({ "waits": waits_on_happening(&asked.text) }))
}

/// The room's road: whether it is the owner's own, and whether the room is a one-to-one.
pub async fn roads(State(served): State<Served>, Path(room): Path<i64>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let facts = receiver.roads().room_facts(room).await;
  let owner_room = receiver.roads().is_owner_room(room).await;

  answered(json!({
    "kind": if owner_room { "owner" } else { "group" },
    "oneToOne": facts.private_to.is_some(),
  }))
}

/// Whether the message's sender is the owner in that room.
pub async fn sender_is_owner(
  State(served): State<Served>,
  Path((room, message)): Path<(i64, i64)>,
  raw: RawQuery,
) -> Answered {
  let _: SenderAsk = query(raw)?;
  let receiver: Receiver = (*served.receiver).clone();
  let road = receiver.roads().room_road(room).await.unwrap_or_default();

  reads::sender_is_owner(receiver.db(), &road, room, message)
    .await
    .map(|owner| json!({ "owner": owner }))
    .map_err(store)
    .and_then(answered)
}

/// Whether the person is the owner, and whether the owner spoke in the room in that span.
pub async fn owner_spoke(State(served): State<Served>, Path(room): Path<i64>, raw: RawQuery) -> Answered {
  let asked = query::<OwnerSpokeAsk>(raw)?;
  let receiver: Receiver = (*served.receiver).clone();
  let road = receiver.roads().room_road(room).await.unwrap_or_default();
  let owner = reads::owner_of_sender(receiver.db(), &road, room, asked.person).await.map_err(store)?;
  let spoke = owner
    && reads::spoke_between(
      receiver.db(),
      room,
      asked.person,
      (asked.before - asked.window_ms) / 1_000,
      asked.before / 1_000,
    )
    .await
    .map_err(store)?;

  answered(json!({ "owner": owner, "spoke": spoke }))
}

/// The code and its parameters queued as an instruction for the chat agent in the owner's private chat.
pub async fn notice(State(served): State<Served>, body: axum::body::Bytes) -> Accepted {
  let asked = read::<Map<String, Value>>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();
  let Some(code) = asked.get("code").and_then(|code| code.as_str()) else {
    return Err(refused(StatusCode::BAD_REQUEST, "bad-request", "The body holds no `code` to notice."));
  };
  let fact = json!(asked).to_string();
  let queued = Queued {
    kind: Some(code),
    sort: Some("notice"),
    summary: Some(code),
    ..Queued::default()
  };
  let id = queue(
    receiver.db(),
    &receiver.deployment().agent,
    receiver.owner_id(),
    &frame(&fact, OWNER_INTENT, None, false),
    &queued,
  )
  .await
  .map_err(store)?;

  if let Some(pass) = receiver.pass() {
    pass.kick();
  }

  Ok((StatusCode::CREATED, axum::Json(json!({ "id": id }))))
}

//! The brain's reads of a room (`docs/boundary-api.md`, 「For the brain」): every one a `POST` in the brain's
//! camelCase shape, answered as the brain's document states it, so the brain reads the receiver as its own
//! document says. The brain keeps what the agents remember; the marks of an agent's book of a room are the
//! receiver's rows, which the brain moves through these routes. An owner notice and a quiet instruction are the
//! two ways the brain puts a row into a room's session.

use axum::{body::Bytes, extract::State, http::StatusCode};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use room_calls::promises::waits_on_happening;

use super::{Answered, Refusal, Served, answered, read, refused, store};
use crate::Fault;
use crate::chat::instructions::{Queued, queue};
use crate::receiver::notices::frame;
use crate::receiver::wake::agents;
use crate::receiver::{NotReceived, Receiver, reads};

// What an owner notice's line is for: the brain writes no sentence, its agent says the fact.
const OWNER_INTENT: &str = "tell the owner what the system recorded.";

// A read of the room that could not be made because a unit it reaches did not answer.
fn reached(why: NotReceived) -> Refusal {
  refused(StatusCode::INTERNAL_SERVER_ERROR, "failed", &why.to_string())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnreadAsk {
  agent: String,
  chat_id: i64,
  limit: i64,

  #[serde(default)]
  since: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CountsAsk {
  agent: String,

  #[serde(default)]
  since: Vec<RoomSince>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoomSince {
  chat_id: i64,
  since: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadAsk {
  agent: String,
  chat_id: i64,
  from: i64,
  limit: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CountAsk {
  agent: String,
  chat_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KeepAsk {
  agent: String,
  chat_id: i64,
  message_ids: Vec<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForgetAsk {
  agent: String,

  #[serde(default)]
  chat_id: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnswerersAsk {
  #[serde(default)]
  chat_id: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpokeAsk {
  chat_id: i64,
  person: i64,
  from: i64,
  to: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OwnNumberAsk {
  agent: String,
  chat_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpokenInAsk {
  chat_id: i64,
  people: Vec<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NamesAsk {
  people: Vec<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WaitsAsk {
  text: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoadsAsk {
  chat_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SenderAsk {
  chat_id: i64,
  message_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OwnerSpokeAsk {
  chat_id: i64,
  person: i64,
  before: i64,
  seconds: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuietAsk {
  agent: String,
  chat_id: i64,
  text: String,
  kind: String,
}

/// A route that takes no body, or one that names nothing, reads as the shape's defaults.
fn taken<T: serde::de::DeserializeOwned>(body: Bytes) -> Result<T, Refusal> {
  let whole: &[u8] = if body.is_empty() { b"{}" } else { &body };

  serde_json::from_slice(whole).map_err(|error| {
    let why = format!("The body is not the shape this route takes: {error}.");

    refused(StatusCode::BAD_REQUEST, "bad-request", &why)
  })
}

/// The room's messages not in an agent's book yet, oldest first.
pub async fn unread(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<UnreadAsk>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();

  reads::unread(receiver.db(), &asked.agent, asked.chat_id, asked.since, asked.limit)
    .await
    .map(|messages| json!({ "messages": messages }))
    .map_err(store)
    .and_then(answered)
}

/// Every room holding messages past an agent's marks, each with how many.
pub async fn unread_counts(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<CountsAsk>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();
  let since: Vec<(i64, i64)> = asked.since.iter().map(|one| (one.chat_id, one.since)).collect();

  reads::unread_counts(receiver.db(), &asked.agent, since)
    .await
    .map(|rooms| json!({ "rooms": rooms.iter().map(|(chat_id, count)| json!({ "chatId": chat_id, "count": count })).collect::<Vec<Value>>() }))
    .map_err(store).and_then(answered)
}

/// The room's messages an agent has marked, after the one given in the room's line.
pub async fn read_messages(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<ReadAsk>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();

  reads::read_messages(receiver.db(), &asked.agent, asked.chat_id, asked.from, asked.limit)
    .await
    .map(|messages| json!({ "messages": messages }))
    .map_err(store)
    .and_then(answered)
}

/// How many of a room's messages an agent has marked, of the ones that still stand.
pub async fn read_count(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<CountAsk>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();

  reads::read_count(receiver.db(), &asked.agent, asked.chat_id)
    .await
    .map(|count| json!({ "count": count }))
    .map_err(store)
    .and_then(answered)
}

/// The room's messages marked as read into an agent's book.
pub async fn keep(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<KeepAsk>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();

  reads::keep_marks(receiver.db(), &asked.agent, asked.chat_id, &asked.message_ids)
    .await
    .map(|_| json!({ "ok": true }))
    .map_err(store)
    .and_then(answered)
}

/// Every mark of an agent's book let go, off one room or off every room; how many were.
pub async fn forget(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<ForgetAsk>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();

  reads::forget_marks(receiver.db(), &asked.agent, asked.chat_id)
    .await
    .map(|forgotten| json!({ "forgotten": forgotten }))
    .map_err(store)
    .and_then(answered)
}

/// Every room, its title and the people that stood as a sender in it.
pub async fn titles(State(served): State<Served>, _: Bytes) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let db = receiver.db();
  let rooms = reads::rooms(db).await.map_err(store)?;
  let listed = listed_titles(db, rooms).await.map_err(store)?;

  answered(json!({ "rooms": listed }))
}

/// Each room with its title and the people that stood as a sender in it.
async fn listed_titles(db: &DatabaseConnection, rooms: Vec<i64>) -> Result<Vec<Value>, Fault> {
  let mut listed: Vec<Value> = vec![];

  for chat_id in rooms {
    listed.push(json!({
      "chatId": chat_id,
      "title": reads::title(db, chat_id).await?,
      "senders": reads::senders(db, chat_id).await?,
    }));
  }

  Ok(listed)
}

/// Every room, and the agents answering it: the awake agents it names, or the default answerer where it names
/// none awake.
pub async fn answerers(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = taken::<AnswerersAsk>(body)?;
  let receiver: Receiver = (*served.receiver).clone();
  let rooms = match asked.chat_id {
    Some(chat_id) => vec![chat_id],
    None => reads::rooms(receiver.db()).await.map_err(store)?,
  };
  listed_answerers(&receiver, rooms)
    .await
    .map(|listed| json!({ "rooms": listed }))
    .map_err(reached)
    .and_then(answered)
}

/// Each room with the agents answering it.
async fn listed_answerers(receiver: &Receiver, rooms: Vec<i64>) -> Result<Vec<Value>, NotReceived> {
  let mut listed: Vec<Value> = vec![];

  for chat_id in rooms {
    listed.push(json!({ "chatId": chat_id, "agents": answering(receiver, chat_id).await? }));
  }

  Ok(listed)
}

/// The names of the agents answering a room, as the wake judges them: the awake agents it names, or the default
/// answerer where it names none awake.
async fn answering(receiver: &Receiver, chat_id: i64) -> Result<Vec<String>, NotReceived> {
  let named = agents::named_designated(receiver, chat_id).await?;

  if named.is_empty() {
    return Ok(vec![agents::default_designated(receiver).await?.name]);
  }

  Ok(named.iter().map(|agent| agent.name.clone()).collect())
}

/// Whether a person sent a message in a room between two moments, both included.
pub async fn spoke_between(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<SpokeAsk>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();

  reads::spoke_between(receiver.db(), asked.chat_id, asked.person, asked.from, asked.to)
    .await
    .map(|spoke| json!({ "spoke": spoke }))
    .map_err(store)
    .and_then(answered)
}

/// The sender the agent's own words stand under in a room, or null.
pub async fn own_number(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<OwnNumberAsk>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();
  let Some(road) = receiver.roads().room_road(asked.chat_id).await else {
    return answered(json!({ "number": null }));
  };
  let Some(designated) = agents::registered(&receiver, &asked.agent).await.map_err(reached)? else {
    return answered(json!({ "number": null }));
  };
  let identity = receiver.roads().identity_in(asked.chat_id).await;

  agents::own_number(&receiver, &designated, &road, identity)
    .await
    .map(|number| json!({ "number": number }))
    .map_err(reached)
    .and_then(answered)
}

/// Of those asked, the people who have stood as a sender in the room.
pub async fn spoken_in(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<SpokenInAsk>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();

  reads::spoken_in(receiver.db(), asked.chat_id, &asked.people)
    .await
    .map(|people| json!({ "people": people }))
    .map_err(store)
    .and_then(answered)
}

/// Each person's name as their latest message went under, or null.
pub async fn names(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<NamesAsk>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();

  listed_names(receiver.db(), asked.people)
    .await
    .map(|listed| json!({ "names": listed }))
    .map_err(store)
    .and_then(answered)
}

/// Each person asked with the name they last went by.
async fn listed_names(db: &DatabaseConnection, people: Vec<i64>) -> Result<Vec<Value>, Fault> {
  let mut listed: Vec<Value> = vec![];

  for person in people {
    listed.push(json!({ "person": person, "name": reads::display_name(db, person).await? }));
  }

  Ok(listed)
}

/// Whether the text waits for something to happen first, by the receiver's reading of promises.
pub async fn waits(State(_served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<WaitsAsk>(&body)?;

  answered(json!({ "waits": waits_on_happening(&asked.text) }))
}

/// The room's road: whether it is the owner's own, and whether the room is a one-to-one.
pub async fn roads(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<RoadsAsk>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();
  let room = receiver.roads().room_facts(asked.chat_id).await;
  let owner_room = receiver.roads().is_owner_room(asked.chat_id).await;

  answered(json!({
    "kind": if owner_room { "owner" } else { "group" },
    "oneToOne": room.private_to.is_some(),
  }))
}

/// Whether the message's sender is the owner in that room.
pub async fn sender_is_owner(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<SenderAsk>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();
  let road = receiver.roads().room_road(asked.chat_id).await.unwrap_or_default();

  reads::sender_is_owner(receiver.db(), &road, asked.chat_id, asked.message_id)
    .await
    .map(|owner| json!({ "owner": owner }))
    .map_err(store)
    .and_then(answered)
}

/// Whether the person is the owner, and whether the owner spoke in the room in that span.
pub async fn owner_spoke(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<OwnerSpokeAsk>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();
  let road = receiver.roads().room_road(asked.chat_id).await.unwrap_or_default();
  let owner = reads::owner_of_sender(receiver.db(), &road, asked.chat_id, asked.person)
    .await
    .map_err(store)?;
  let spoke = owner
    && reads::spoke_between(
      receiver.db(),
      asked.chat_id,
      asked.person,
      asked.before - asked.seconds,
      asked.before,
    )
    .await
    .map_err(store)?;

  answered(json!({ "owner": owner, "spoke": spoke }))
}

/// The code and its parameters queued as an instruction for the chat agent in the owner's private chat.
pub async fn notice(State(served): State<Served>, body: Bytes) -> Answered {
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

  queue(
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

  answered(json!({ "ok": true }))
}

/// The quiet instruction queued for the agent in the room, under the sort its kind names, and its turn woken.
pub async fn quiet(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<QuietAsk>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();

  if asked.kind != "watch" && asked.kind != "reminder" {
    return Err(refused(
      StatusCode::BAD_REQUEST,
      "bad-request",
      &format!("`kind` is neither `watch` nor `reminder` but {}.", asked.kind),
    ));
  }
  let queued = Queued {
    sort: Some(&asked.kind),
    ..Queued::default()
  };

  queue(receiver.db(), &asked.agent, asked.chat_id, &asked.text, &queued)
    .await
    .map_err(store)?;

  if let Some(pass) = receiver.pass() {
    pass.kick();
  }

  answered(json!({ "ok": true }))
}

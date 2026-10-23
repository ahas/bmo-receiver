//! A room as the receiver holds it (`docs/api.md`, 「Rooms」): the deployment's owner, one room, the agents
//! answering it, one message, the chat providers it is on, and a room added on a chat service in a mode. Any chat
//! reads as a room; one never registered reads as one whose registration is gone, and the work filed from it
//! reaches the owner's private chat instead. What a provider offers a room added on it is read off its declaration,
//! never off its name.

use axum::{
  body::Bytes,
  extract::{Path, RawQuery, State},
  http::StatusCode,
};
use bmo_chat_client::{AccountRoomsQuery, ChatDeclaration, ClientFailure, ErrorCode, RoomKind};
use receiver_codes::RoomAdditionRefusal;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Accepted, Answered, Refusal, Served, answered, code, query, read, refused, store};
use crate::receiver::wake::agents;
use crate::receiver::{NotReceived, Receiver, reads, rows};
use crate::roads::added::{self, RoomMode};
use crate::roads::{Road, providers};
use crate::routes::say::known;

// A read of the room that could not be made because a unit it reaches did not answer.
fn reached(why: NotReceived) -> Refusal {
  refused(StatusCode::BAD_GATEWAY, "upstream-failed", &why.to_string())
}

/// The deployment's owner: the owner as a sender, and the owner's private chat.
pub async fn owner(State(served): State<Served>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();

  answered(json!({ "senderId": receiver.owner_id(), "room": receiver.owner_id() }))
}

/// What a room is to the deployment, in the shape the room list answers.
pub async fn room(State(served): State<Served>, Path(room): Path<i64>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();

  room_read(&receiver, room).await.and_then(answered)
}

// A room as `GET /rooms/{roomId}` answers it.
async fn room_read(receiver: &Receiver, room: i64) -> Result<Value, Refusal> {
  let db = receiver.db();
  let owner_room = receiver.roads().is_owner_room(room).await;
  let registered = rows::is_registered(db, room).await.map_err(store)?;
  let work = rows::KeptWorkRoom::of(room, receiver.owner_id(), registered);
  let shown = reads::shown(db).await.map_err(store)?;
  let line = shown.iter().find(|line| line.chat_id == room);
  let pinned = crate::receiver::pinned::pinned(db).await.map_err(store)?;
  let answerers = agents::named_designated(receiver, room).await.map_err(reached)?;
  let companions: Vec<String> = answerers
    .iter()
    .filter(|agent| agent.companion())
    .map(|agent| agent.name.clone())
    .collect();
  let senders = reads::senders(db, room).await.map_err(store)?;
  let mode = added::mode_of(db, room).await.map_err(store)?;

  Ok(json!({
    "id": room,
    "title": line.and_then(|line| line.title.clone()),
    "messageCount": line.map_or(0, |line| line.message_count),
    "lastMessageAt": line.and_then(|line| line.last_message_at),
    "pinned": pinned.contains(&room),
    "senders": senders,
    "companions": companions,
    "ownerRoom": owner_room,
    "registered": registered,
    "workRoom": work.chat_id,
    "mode": mode.map(RoomMode::as_str),
  }))
}

/// The agents answering a room: the default answerer, the voice of the room, and every agent named to answer
/// there that is not resting.
pub async fn answerers(State(served): State<Served>, Path(room): Path<i64>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let default = agents::default_designated(&receiver).await.map_err(reached)?;
  let voice = agents::voice_of(&receiver, room).await.map_err(reached)?;
  let named = agents::named_designated(&receiver, room).await.map_err(reached)?;
  let listed: Vec<Value> = named
    .iter()
    .map(|agent| json!({ "agentName": agent.name, "companion": agent.companion() }))
    .collect();

  answered(json!({
    "default": default.name,
    "voice": voice,
    "items": listed,
    "more": false,
  }))
}

/// A message said in a room; nothing found where the receiver does not keep it.
pub async fn message(State(served): State<Served>, Path((room, message)): Path<(i64, i64)>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let Some(kept) = reads::message_of(receiver.db(), room, message).await.map_err(store)? else {
    return Err(refused(
      StatusCode::NOT_FOUND,
      "not-found",
      &format!("The receiver keeps no message {message} in room {room}."),
    ));
  };

  let files: Vec<Value> = reads::files(receiver.db(), room, message)
    .await
    .map_err(store)?
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

  answered(json!({
    "id": kept.message_id,
    "room": kept.chat_id,
    "senderId": kept.sender_id,
    "senderName": kept.sender_name,
    "sentAt": kept.sent_at,
    "text": kept.text,
    "imagePath": kept.image_path,
    "replyToMessageId": kept.reply_to_message_id,
    "answerSeconds": kept.answer_seconds,
    "round": kept.round,
    "album": kept.album,
    "via": kept.via,
    "files": files,
  }))
}

/// Every chat provider a room may be on, each by its key and what it is called (docs/receiver.md, 「A room's chat
/// providers」).
pub async fn providers(State(served): State<Served>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let items: Vec<Value> = providers::every(receiver.roads())
    .await
    .into_iter()
    .map(|provider| {
      let modes: Vec<&str> = provider.room_modes.iter().map(|mode| mode.as_str()).collect();

      json!({
        "key": provider.key,
        "label": provider.label,
        "listsRooms": provider.lists_rooms,
        "roomModes": modes,
      })
    })
    .collect();

  answered(json!({ "items": items, "more": false }))
}

// A call on a chat service that did not answer.
fn unanswered(key: &str, why: impl std::fmt::Display) -> Refusal {
  refused(
    StatusCode::BAD_GATEWAY,
    "upstream-failed",
    &format!("The {key} road did not answer: {why}"),
  )
}

// What a chat service declares of itself now; a service that does not answer is no service to add a room on.
async fn declared_now(receiver: &Receiver, key: &str) -> Result<ChatDeclaration, Refusal> {
  receiver
    .roads()
    .declaration_now(key)
    .await
    .ok_or_else(|| unanswered(key, "its declaration"))
}

/// The rooms a provider's account is in, relayed from its own list page by page, each with the mode the receiver
/// added it in. Refused where no provider stands under the key, and where the provider does not declare that it
/// lists them.
pub async fn account_rooms(
  State(served): State<Served>,
  Path(key): Path<String>,
  raw: RawQuery,
) -> Answered {
  let asked = query::<AccountRoomsQuery>(raw)?;
  let receiver: Receiver = (*served.receiver).clone();

  if !providers::stands(receiver.roads(), &key) {
    return Err(refused(StatusCode::NOT_FOUND, "not-found", &format!("No provider stands under {key}.")));
  }

  let Some(service) = receiver.roads().chats().road(&key) else {
    let label = receiver.roads().facts(&Road::Record).await.label;

    return Err(code(&RoomAdditionRefusal::RoomsUnlisted { label }));
  };
  let declared = declared_now(&receiver, &key).await?;

  if !added::lists_rooms(&declared) {
    return Err(code(&RoomAdditionRefusal::RoomsUnlisted {
      label: declared.label,
    }));
  }

  let page = match service.account_rooms(&asked).await {
    Ok(page) => page,
    Err(failure) if failure.code() == Some(ErrorCode::UnsupportedCapability) => {
      return Err(code(&RoomAdditionRefusal::RoomsUnlisted {
        label: declared.label,
      }));
    }
    Err(why) => return Err(unanswered(&key, why)),
  };
  let modes = added::modes(receiver.db()).await.map_err(store)?;
  let items: Vec<Value> = page
    .items
    .iter()
    .map(|room| {
      json!({
        "id": room.id,
        "title": room.title,
        "kind": match room.kind {
          RoomKind::Owner => "owner",
          RoomKind::Group => "group",
        },
        "privateTo": room.private_to,
        "mode": modes.get(&room.id).map(|mode| mode.as_str()),
      })
    })
    .collect();

  answered(json!({ "items": items, "more": page.more }))
}

/// A room to add: the chat service it is chosen on, the room as that service numbers it, its mode, and its title
/// where the caller names one.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Adding {
  provider: String,
  room_id: i64,
  mode: RoomMode,
  #[serde(default)]
  title: Option<String>,
}

/// A room added on a chat service in a mode, kept with its title and its list of providers set to that service and
/// the record; answered with the room as it now stands. A room added before, one the service does not say is its
/// own and a mode the service does not offer are refused, and nothing is written.
pub async fn add_room(State(served): State<Served>, body: Bytes) -> Accepted {
  let asked = read::<Adding>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();
  let (key, room) = (asked.provider.as_str(), asked.room_id);
  let Some(service) = receiver.roads().chats().road(key) else {
    return Err(refused(
      StatusCode::BAD_REQUEST,
      "bad-request",
      &format!("No chat service stands under {key}."),
    ));
  };

  if added::mode_of(receiver.db(), room).await.map_err(store)?.is_some() {
    return Err(code(&RoomAdditionRefusal::RoomAdded { room }));
  }

  let declared = declared_now(&receiver, key).await?;
  let unknown = || {
    code(&RoomAdditionRefusal::RoomUnknown {
      label: declared.label.clone(),
      room,
    })
  };
  let held = match service.room(room).await {
    Ok(held) if held.owns => held,
    Ok(_) | Err(ClientFailure::Answered { status: 404, .. }) => return Err(unknown()),
    Err(why) => return Err(unanswered(key, why)),
  };

  if !added::offered(&declared).contains(&asked.mode) {
    return Err(code(&RoomAdditionRefusal::AvatarUnsupported {
      label: declared.label,
    }));
  }

  let title = asked.title.filter(|title| !title.trim().is_empty()).or(held.title);
  let made = added::add(receiver.db(), room, key, asked.mode, title.as_deref())
    .await
    .map_err(store)?;

  if !made {
    return Err(code(&RoomAdditionRefusal::RoomAdded { room }));
  }

  // The service has just said what it holds; what was kept of the room before is read again.
  receiver.roads().forget_room(room);

  Ok((StatusCode::CREATED, axum::Json(room_read(&receiver, room).await?)))
}

/// The list of providers a room is to be on, set whole.
#[derive(Deserialize)]
struct Choosing {
  providers: Vec<String>,
}

// A room the receiver does not hold is no room whose providers can be read or set.
async fn held(receiver: &Receiver, room: i64) -> Result<(), Refusal> {
  if known(receiver, room).await? {
    return Ok(());
  }

  Err(refused(
    StatusCode::NOT_FOUND,
    "not-found",
    &format!("The receiver holds no room {room}."),
  ))
}

// The room's providers as they stand.
async fn standing(receiver: &Receiver, room: i64) -> Answered {
  let listed = providers::listed(receiver.db(), receiver.roads(), room).await.map_err(store)?;

  answered(json!({ "providers": listed.providers, "chosen": listed.chosen }))
}

/// The providers a room is on, and whether its list was set.
pub async fn room_providers(State(served): State<Served>, Path(room): Path<i64>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();

  held(&receiver, room).await?;
  standing(&receiver, room).await
}

/// A room's providers set whole: a list of providers that stand, each once, and at least one.
pub async fn choose_providers(
  State(served): State<Served>,
  Path(room): Path<i64>,
  body: Bytes,
) -> Answered {
  let asked = read::<Choosing>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();

  held(&receiver, room).await?;

  if asked.providers.is_empty() {
    return Err(refused(StatusCode::BAD_REQUEST, "bad-request", "A room is on one provider at least."));
  }

  for (at, key) in asked.providers.iter().enumerate() {
    if !providers::stands(receiver.roads(), key) {
      return Err(refused(
        StatusCode::BAD_REQUEST,
        "bad-request",
        &format!("No provider stands under {key}."),
      ));
    }

    if asked.providers[..at].contains(key) {
      return Err(refused(StatusCode::BAD_REQUEST, "bad-request", &format!("{key} is named twice.")));
    }
  }

  providers::choose(receiver.db(), room, &asked.providers).await.map_err(store)?;
  standing(&receiver, room).await
}

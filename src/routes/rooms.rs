//! A room as the receiver holds it (`docs/api.md`, 「Rooms」): the deployment's owner, one room, the agents
//! answering it and one message. Any chat reads as a room; one never registered reads as one whose registration is
//! gone, and the work filed from it reaches the owner's private chat instead.

use axum::{
  extract::{Path, State},
  http::StatusCode,
};
use serde_json::{Value, json};

use super::{Answered, Refusal, Served, answered, refused, store};
use crate::receiver::wake::agents;
use crate::receiver::{NotReceived, Receiver, reads, rows};

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
  let db = receiver.db();
  let owner_room = receiver.roads().is_owner_room(room).await;
  let registered = rows::is_registered(db, room).await.map_err(store)?;
  let work = rows::kept_work_room(db, room, receiver.owner_id()).await.map_err(store)?;
  let shown = reads::shown(db).await.map_err(store)?;
  let line = shown.iter().find(|line| line.chat_id == room);
  let pinned = crate::receiver::pinned::pinned(db).await.map_err(store)?;
  let answerers = agents::named_designated(&receiver, room).await.map_err(reached)?;
  let companions: Vec<String> = answerers
    .iter()
    .filter(|agent| agent.companion())
    .map(|agent| agent.name.clone())
    .collect();
  let senders = reads::senders(db, room).await.map_err(store)?;

  answered(json!({
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

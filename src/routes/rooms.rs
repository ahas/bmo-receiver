//! What a room is to the deployment, as bmo-job reads it and as the console's owner reads it
//! (`docs/boundary-api.md`, 「For bmo-job」 and 「For the web unit's console」). Any chat reads as a room; one never
//! registered reads as one whose registration is gone, and the work filed from it reaches the owner's private chat
//! instead.

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
  refused(StatusCode::INTERNAL_SERVER_ERROR, "failed", &why.to_string())
}

/// The deployment's owner: the owner as a sender, and the owner's private chat.
pub async fn owner(State(served): State<Served>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();

  answered(json!({ "sender": receiver.owner_id(), "chat": receiver.owner_id() }))
}

/// What a room is to the deployment.
pub async fn room(State(served): State<Served>, Path(chat): Path<i64>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let owner_room = receiver.roads().is_owner_room(chat).await;
  let registered = rows::is_registered(receiver.db(), chat).await.map_err(store)?;
  let work = rows::kept_work_room(receiver.db(), chat, receiver.owner_id()).await.map_err(store)?;

  answered(json!({
    "chat": chat,
    "owner_room": owner_room,
    "registered": registered,
    "work_chat": work.chat_id,
  }))
}

/// The agents answering a room: the default answerer, the voice of the room, and every agent named to answer
/// there that is not resting.
pub async fn answerers(State(served): State<Served>, Path(chat): Path<i64>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let default = agents::default_designated(&receiver).await.map_err(reached)?;
  let voice = agents::voice_of(&receiver, chat).await.map_err(reached)?;
  let named = agents::named_designated(&receiver, chat).await.map_err(reached)?;
  let listed: Vec<Value> = named
    .iter()
    .map(|agent| json!({ "agent": agent.name, "companion": agent.companion() }))
    .collect();

  answered(json!({ "default": default.name, "voice": voice, "answerers": listed }))
}

/// A message said in a room; nothing found where the receiver does not keep it.
pub async fn message(State(served): State<Served>, Path((chat, message)): Path<(i64, i64)>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let Some(kept) = reads::message_of(receiver.db(), chat, message).await.map_err(store)? else {
    return Err(refused(
      StatusCode::NOT_FOUND,
      "not-found",
      &format!("The receiver keeps no message {message} in room {chat}."),
    ));
  };

  answered(json!({
    "sender": kept.sender_id,
    "sender_name": kept.sender_name,
    "sent_at": kept.sent_at,
    "text": kept.text,
  }))
}

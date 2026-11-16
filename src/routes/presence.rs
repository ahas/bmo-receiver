//! The console as a source of typing and presence (docs/api.md, 「Typing and presence」; docs/receiver.md, 「Typing and
//! presence」): an account's presence as the console draws it from its tabs, and an account's typing in a room,
//! told on the stream and handed to the room's chat services in the writer's own voice. Nothing of either is kept.

use axum::{
  body::Bytes,
  extract::{Path, State},
  http::StatusCode,
};
use bmo_chat_client::{ChatPresence, ChatPresenceState};
use serde::Deserialize;

use super::say::known;
use super::writable::WritableRoom;
use super::{Refusal, Served, read, refused, store};
use crate::presence::taken;
use crate::receiver::Receiver;

/// An account's presence as the console tells it: the state, none taking the console's word back, and the last
/// moment a visible tab of it reported.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountPresence {
  #[serde(default)]
  state: Option<ChatPresenceState>,
  #[serde(default)]
  last_seen_at: Option<i64>,
}

/// An account writing in a room's box in the console, or no longer.
#[derive(Deserialize)]
struct AccountTyping {
  on: bool,
}

/// `PUT /accounts/{account}/presence`: the account's presence as the console tells it, one source among its own.
pub async fn account_presence(
  State(served): State<Served>,
  Path(account): Path<String>,
  body: Bytes,
) -> Result<StatusCode, Refusal> {
  let account = account.trim().parse::<i64>().map_err(|_| {
    refused(StatusCode::BAD_REQUEST, "bad-request", "an account is named by its key, a number")
  })?;
  let asked = read::<AccountPresence>(&body)?;
  let presence = asked.state.map(|state| ChatPresence {
    state,
    last_seen_at: asked.last_seen_at,
  });
  let receiver: Receiver = (*served.receiver).clone();

  taken::console_presence(&receiver, account, presence).await.map_err(store)?;

  Ok(StatusCode::NO_CONTENT)
}

/// `POST /rooms/{roomId}/typing`: an account writing in the room, or no longer, through the guard every write to a
/// room passes, so another account's private room refuses it.
pub async fn account_typing(
  State(served): State<Served>,
  writable: WritableRoom,
) -> Result<StatusCode, Refusal> {
  let room = writable.room;
  let asked = read::<AccountTyping>(writable.writer.body())?;
  let Some(account) = writable.writer.account() else {
    return Err(refused(StatusCode::BAD_REQUEST, "bad-request", "account (a number) is required"));
  };
  let receiver: Receiver = (*served.receiver).clone();

  if !known(&receiver, room).await? {
    return Err(refused(
      StatusCode::NOT_FOUND,
      "not-found",
      &format!("The receiver holds no room {room}."),
    ));
  }

  taken::console_typing(&receiver, room, account, asked.on).await.map_err(store)?;

  Ok(StatusCode::NO_CONTENT)
}

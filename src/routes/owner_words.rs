//! The owner's words from the console to a room's session (docs/boundary-api.md, 「The owner's words」): said to
//! the session the way every other instruction is — quietly, inside the conversation the room already has, framed
//! as his own words in that chat. A chat the receiver does not serve is not woken, whoever asks; the owner's own
//! chat always is.

use axum::{body::Bytes, extract::State, http::StatusCode};
use serde::Deserialize;
use serde_json::json;

use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::fill_marks;

use super::{Accepted, Served, read, refused, store};
use crate::chat::instructions::{Queued, queue};
use crate::receiver::{Receiver, rows, wake};
use crate::routes::say::known;

// The instruction the words go in as.
fn instruction(text: &str) -> String {
  CHAT_ROUND
    .console
    .instruction
    .iter()
    .map(|line| fill_marks(line, &[("text", text)]))
    .collect::<Vec<String>>()
    .join("\n")
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstructBody {
  chat_id: i64,
  text: String,
}

pub async fn instruct(State(served): State<Served>, body: Bytes) -> Accepted {
  let asked = read::<InstructBody>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();
  let text = asked.text.trim();

  if text.is_empty() {
    return Err(refused(
      StatusCode::BAD_REQUEST,
      "bad-request",
      "chatId (number) and text (non-empty string) are required",
    ));
  }

  if !known(&receiver, asked.chat_id).await? {
    return Err(refused(StatusCode::NOT_FOUND, "no-route", &format!("unknown chat {}", asked.chat_id)));
  }

  // A chat the service does not serve is not woken, whoever asks; the owner's own room always is.
  if asked.chat_id != receiver.owner_room() && !rows::is_chat_allowed(asked.chat_id) {
    return Err(refused(
      StatusCode::FORBIDDEN,
      "forbidden",
      &format!("chat {} is not allowed", asked.chat_id),
    ));
  }

  let agent = wake::agents::voice_of(&receiver, asked.chat_id)
    .await
    .unwrap_or(receiver.deployment().agent.clone());
  let queued = Queued {
    sort: Some(&CHAT_ROUND.console.sort),
    summary: Some(text),
    ..Queued::default()
  };

  queue(receiver.db(), &agent, asked.chat_id, &instruction(text), &queued)
    .await
    .map_err(store)?;

  if let Some(pass) = receiver.pass() {
    pass.kick();
  }

  Ok((StatusCode::NO_CONTENT, axum::Json(json!({}))))
}

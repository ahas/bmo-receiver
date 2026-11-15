//! The owner's words from the console to a room's session (docs/boundary-api.md, 「The owner's words」): said to
//! the session the way every other instruction is — quietly, inside the conversation the room already has, framed
//! as his own words in that chat. A chat the receiver does not serve is not woken, whoever asks; the owner's own
//! chat always is.

use axum::{extract::State, http::StatusCode};
use serde::Deserialize;
use serde_json::json;

use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::fill_marks;

use super::writable::Writer;
use super::{Accepted, Served, read, refused, store};
use crate::chat::instructions::{Queued, queue};
use crate::receiver::{Receiver, rows, wake};
use crate::routes::say::known;
use crate::secrets;

// The instruction the words go in as: the console's marker and the owner's text as data.
fn instruction(text: &str) -> String {
  let body = CHAT_ROUND
    .console
    .instruction
    .iter()
    .map(|line| fill_marks(line, &[("text", text)]))
    .collect::<Vec<String>>()
    .join("\n");

  format!("{}\n{body}", CHAT_ROUND.console.marker)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstructBody {
  chat_id: i64,
  text: String,
}

/// `POST /owner/instructions`: the owner's instruction into a room, on his own road, which writes in no other
/// account's private room.
pub async fn instruct(State(served): State<Served>, writer: Writer) -> Accepted {
  let asked = read::<InstructBody>(writer.body())?;
  let receiver: Receiver = (*served.receiver).clone();

  writer.owners().room(&receiver, asked.chat_id).await?;

  let text = secrets::redacted(asked.text.trim());
  let text = text.as_str();

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
  if asked.chat_id != receiver.owner_room() && !rows::is_numbered_whole(asked.chat_id) {
    return Err(refused(
      StatusCode::FORBIDDEN,
      "forbidden",
      &format!("chat {} is not allowed", asked.chat_id),
    ));
  }

  let agent = wake::agents::voice_of(&receiver, asked.chat_id)
    .await
    .unwrap_or(receiver.default_answerer());
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

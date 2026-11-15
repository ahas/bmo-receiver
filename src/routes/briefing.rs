//! The voice briefing's routes (docs/api.md, 「The voice briefing」; docs/voiced-notices.md, 「The voice
//! briefing」): its two switches read and moved one at a time from the console, and a line's full reading asked for
//! from its chip, which writes into the line's room and so is never asked for another account's private room.

use axum::{
  body::Bytes,
  extract::{Path, RawQuery, State},
  http::StatusCode,
};
use receiver_codes::VoiceRefusal;
use serde_json::{Value, json};

use crate::chat::briefing::{self, AUTOPLAY, SWITCH, Unread};
use crate::routes::accounts::{Acting, acting, writable};
use crate::routes::{Answered, Refusal, Served, answered, code, read, refused};

async fn standing(served: &Served) -> Answered {
  let (on, autoplay) = briefing::switches(&served.receiver).await;

  answered(json!({ "on": on, "autoplay": autoplay }))
}

/// `GET /voice-briefing`: both switches as they stand.
pub async fn switches(State(served): State<Served>) -> Answered {
  standing(&served).await
}

/// `PUT /voice-briefing`: exactly one switch moved, answered with both as they now stand.
pub async fn switch(State(served): State<Served>, body: Bytes) -> Answered {
  let asked: Value = serde_json::from_slice(&body).map_err(|_| code(&VoiceRefusal::OneSwitch))?;
  let on = asked.get("on");
  let autoplay = asked.get("autoplay");
  let (key, value) = match (on.map(Value::as_bool), autoplay.map(Value::as_bool)) {
    (Some(Some(value)), None) => (SWITCH, value),
    (None, Some(Some(value))) => (AUTOPLAY, value),
    _ => return Err(code(&VoiceRefusal::OneSwitch)),
  };

  briefing::set(&served.receiver, key, value)
    .await
    .map_err(|why| refused(StatusCode::BAD_GATEWAY, "upstream-failed", &why.to_string()))?;

  standing(&served).await
}

/// `POST /rooms/{roomId}/messages/{messageId}/reading`: a line's full reading, launched where none stands: a turn of
/// the line's agent in the room writing its script, and a briefing the voice engine keeps for the room.
pub async fn reading(
  State(served): State<Served>,
  Path((chat_id, message_id)): Path<(i64, i64)>,
  raw: RawQuery,
  body: Bytes,
) -> Answered {
  let in_body = if body.is_empty() { None } else { read::<Acting>(&body)?.account };

  writable(&served.receiver, chat_id, acting(raw, in_body)?).await?;

  match briefing::reading(&served.receiver, chat_id, message_id).await {
    Ok(reading) => answered(json!({
      "state": if reading.ready { "ready" } else { "making" },
      "briefingId": reading.briefing,
    })),
    Err(Unread::Refused(refusal)) => Err(code(&refusal)),
    Err(Unread::NoLine) => Err(refused(
      StatusCode::NOT_FOUND,
      "not-found",
      &format!("No agent's line {message_id} in chat {chat_id} to read."),
    )),
    Err(Unread::Failed(why)) => Err(failed(why)),
  }
}

fn failed(why: crate::receiver::NotReceived) -> Refusal {
  refused(StatusCode::BAD_GATEWAY, "upstream-failed", &why.to_string())
}

//! The agency's hooks the receiver answers, and the cache warmer's ping (`docs/api.md`, 「Hooks」). The agency
//! posts `{"point", "ask"}` and reads the `answer` of a 2xx
//! answer; the warmer posts its ping with no header at all, the token standing in the hook's URL the receiver
//! registered instead, so this route alone reads the token from the query.

use axum::{
  body::Bytes,
  extract::{Path, Query, State},
  http::StatusCode,
};
use receiver_codes::SkipReason;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Answered, Refusal, Served, answered, read, refused, store};
use crate::Fault;
use crate::chat::touch::{Ping, TouchAnswer, TouchOutcome};
use crate::now_ms;
use crate::receiver::{Receiver, reads};
use crate::verbs::sends::Sends;

#[derive(Deserialize)]
struct Hooked {
  ask: Value,
}

#[derive(Deserialize)]
struct AskLine {
  text: String,

  #[serde(rename = "sessionId")]
  session_id: String,
}

#[derive(Deserialize)]
struct AskRoad {
  chat: i64,

  #[serde(default)]
  via: Option<String>,
}

#[derive(Deserialize)]
pub struct Tokened {
  token: String,
}

fn bad(what: &str) -> Refusal {
  refused(
    StatusCode::BAD_REQUEST,
    "bad-request",
    &format!("The ask is not the shape this point takes: {what}."),
  )
}

fn read_ask<T: serde::de::DeserializeOwned>(ask: &Value) -> Result<T, Refusal> {
  serde_json::from_value::<T>(ask.clone()).map_err(|why| bad(&why.to_string()))
}

/// The agency's ask answered: the point's own answer under `answer`.
pub async fn hook(State(served): State<Served>, Path(point): Path<String>, body: Bytes) -> Answered {
  let asked = read::<Hooked>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();

  match point.as_str() {
    "room-title" => title(&receiver, &asked.ask).await,
    "typed-line" => typed(&receiver, &asked.ask).await,
    "road-verbs" => verbs(&receiver, &asked.ask).await,
    point => Err(refused(
      StatusCode::BAD_REQUEST,
      "bad-request",
      &format!("No hook answers the point {point}."),
    )),
  }
}

/// A room's title, or null where it has none.
async fn title(receiver: &Receiver, ask: &Value) -> Answered {
  let Some(chat) = ask.as_i64() else { return Err(bad("the ask is no room number")) };

  answered(json!({ "answer": reads::title(receiver.db(), chat).await.ok().flatten() }))
}

/// The context a line typed into the chat agent's program goes on with, or null where none was composed.
async fn typed(receiver: &Receiver, ask: &Value) -> Answered {
  let asked = read_ask::<AskLine>(ask)?;
  let Some(typing) = receiver.typing() else {
    return answered(json!({ "answer": null }));
  };
  let context = typing
    .typed_line(receiver, &receiver.deployment().agent, &asked.text, &asked.session_id, now_ms())
    .await;

  answered(json!({ "answer": context.ok() }))
}

/// Every verb the road the room is answered on performs.
async fn verbs(receiver: &Receiver, ask: &Value) -> Answered {
  let asked = read_ask::<AskRoad>(ask)?;
  let sends = Sends::new(receiver.clone(), None, asked.via.clone());

  match sends.answering_can(asked.chat).await {
    Ok(can) => answered(json!({ "answer": json!({ "can": can }) })),
    Err(why) => Err(refused(StatusCode::CONFLICT, "no-route", &why.to_string())),
  }
}

/// The warmer's ping answered: the touch of the chat agent's one session, where the ping is of that target.
pub async fn ping(State(served): State<Served>, query: Query<Tokened>, body: Bytes) -> Answered {
  let asked = read::<Ping>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();

  if query.token != *served.token {
    return Err(refused(
      StatusCode::UNAUTHORIZED,
      "unauthorized",
      "The hook URL carries no token the receiver knows.",
    ));
  }

  touched(&receiver, &asked)
    .await
    .map(|answer| json!(answer))
    .map_err(store)
    .and_then(answered)
}

/// The touch the ping asks for, as the conversations hold it.
async fn touched(receiver: &Receiver, ping: &Ping) -> Result<TouchAnswer, Fault> {
  if !ping.is_chat_touch() {
    return Ok(TouchAnswer {
      outcome: TouchOutcome::Skipped,
      code: Some(SkipReason::NotChatTouch),
      reason: Some("the ping is not of the chat touch".to_owned()),
    });
  }
  let Some(touching) = receiver.touching() else {
    return Ok(TouchAnswer {
      outcome: TouchOutcome::Skipped,
      code: Some(SkipReason::NoConversation),
      reason: Some("no conversations stand to be touched".to_owned()),
    });
  };

  touching.touch(receiver, &receiver.deployment().agent, Some(ping.bound_ms)).await
}

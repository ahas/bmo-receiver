//! The instruction queue bmo-job reaches (`docs/boundary-api.md`, 「For bmo-job」): an instruction queued for an
//! agent's session in a room, its state read, one not yet taken withdrawn, a notice voiced as one of those rows,
//! and a unit declared a holder of rows naming a room. Bodies are snake_case and times epoch milliseconds, as
//! bmo-job's document states them; a refusal carries `error` and `code` in that shape.
//!
//! An instruction whose `said` holds a fact is framed the way every voiced notice is, so the room's voice says it;
//! one whose `said` holds an order goes to the session as the words it was given.

use axum::{
  body::Bytes,
  extract::{Path, State},
  http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{Answered, Refusal, Served, answered, read, refused, store};
use crate::chat::instructions::{Queued, queue};
use crate::receiver::notices::frame;
use crate::receiver::wake::agents;
use crate::receiver::{NotReceived, Receiver};
use crate::store::settings;
use crate::{one, run};

// What a fact an instruction carries is said for, where the call names no intent of its own.
const FACT_INTENT: &str = "tell this chat what the system recorded.";

// The family a voiced notice is listed under.
const NOTICE_SORT: &str = "notice";

// A unit declared as a holder of rows naming a room, kept under this key beside the unit's name.
pub const HOLDER_KEY: &str = "room_holder:";

/// A read of the room that could not be made because a unit it reaches did not answer.
fn reached(why: NotReceived) -> Refusal {
  refused(StatusCode::INTERNAL_SERVER_ERROR, "failed", &why.to_string())
}

/// The room, message, instruction or run named does not exist.
fn absent(what: &str) -> Refusal {
  refused(StatusCode::NOT_FOUND, "not-found", &format!("Nothing stands under {what}."))
}

/// An answer with its status beside the body, for the routes that answer 201 or 204.
type Accepted = Result<(StatusCode, axum::Json<serde_json::Value>), Refusal>;

/// What an instruction hands the session: a fact, or the words of an order.
#[derive(Deserialize)]
struct Said {
  #[serde(default)]
  fact: Option<Value>,

  #[serde(default)]
  order: Option<String>,
}

/// One instruction as bmo-job queues it.
#[derive(Deserialize)]
struct Instruction {
  #[serde(default)]
  agent: Option<String>,
  #[serde(default)]
  chat: Option<i64>,
  said: Said,
  #[serde(default)]
  kind: Option<String>,
  sort: String,
  #[serde(default)]
  summary: Option<String>,
  #[serde(default)]
  expires_at: Option<i64>,
  #[serde(default)]
  events: Vec<String>,
  #[serde(default)]
  rides_until: Option<i64>,
  #[serde(default)]
  via: Option<String>,
  #[serde(default)]
  run: Option<i64>,
}

/// One notice as bmo-job voices it.
#[derive(Deserialize)]
struct Notice {
  fact: Value,
  intent: String,
  #[serde(default)]
  follow_up: Option<String>,
  #[serde(default)]
  optional: bool,
  #[serde(default)]
  kind: Option<String>,
  #[serde(default)]
  expires_at: Option<i64>,
  #[serde(default)]
  events: Vec<String>,
  #[serde(default)]
  rides_until: Option<i64>,
}

/// How far an instruction has gone.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stood {
  Pending,
  Handed,
  Delivered,
}

/// A unit's declaration as a holder.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct Holder {
  address: String,

  #[serde(default)]
  holdings: Vec<Hold>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
struct Hold {
  name: String,
  effect: Effect,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Effect {
  Kept,
  Warns,
}

/// The instruction queued, answered with its id.
pub async fn queue_instruction(State(served): State<Served>, body: Bytes) -> Accepted {
  let asked = read::<Instruction>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();
  let agent = asked.agent.clone().unwrap_or_else(|| receiver.deployment().agent.clone());
  let chat_id = asked.chat.unwrap_or(receiver.owner_id());
  let text = said(&asked.said);
  let queued = Queued {
    kind: asked.kind.as_deref(),
    expires_at_ms: asked.expires_at,
    events: asked.events.clone(),
    rides_until_ms: asked.rides_until,
    rides_session: false,
    sort: Some(&asked.sort),
    summary: asked.summary.as_deref(),
    via: asked.via.as_deref(),
    run_id: asked.run,
  };

  queue(receiver.db(), &agent, chat_id, &text, &queued)
    .await
    .map(|id| (StatusCode::CREATED, axum::Json(json!({ "id": id }))))
    .map_err(store)
}

/// The words the session is handed: an order as it was given, a fact in the notice's frame.
fn said(said: &Said) -> String {
  match said {
    Said {
      order: Some(order),
      fact: _,
    } => order.clone(),
    Said {
      order: _,
      fact: Some(fact),
    } => frame(&json!(fact).to_string(), FACT_INTENT, None, false),
    Said { order: _, fact: _ } => String::new(),
  }
}

/// How far the instruction named has gone; nothing found where it was withdrawn, replaced or lapsed.
pub async fn instruction_state(State(served): State<Served>, Path(id): Path<i64>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();

  let row = one(
    receiver.db(),
    r#"SELECT "handed_at", "delivered_at" FROM "instructions" WHERE "id" = $1"#,
    vec![id.into()],
  )
  .await
  .map_err(store)?;

  let Some(row) = row else { return Err(absent(&format!("instruction {id}"))) };
  let handed = row.try_get::<Option<i64>>("", "handed_at").ok().flatten();
  let delivered = row.try_get::<Option<i64>>("", "delivered_at").ok().flatten();
  let stood = match (handed, delivered) {
    (_, Some(_)) => Stood::Delivered,
    (Some(_), None) => Stood::Handed,
    (None, None) => Stood::Pending,
  };

  answered(json!({ "state": stood }))
}

/// The instruction not yet taken withdrawn; false where it was handed already or is gone.
pub async fn withdraw(State(served): State<Served>, Path(id): Path<i64>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();

  let taken = run(
    receiver.db(),
    r#"DELETE FROM "instructions" WHERE "id" = $1 AND "handed_at" IS NULL AND "delivered_at" IS NULL"#,
    vec![id.into()],
  )
  .await
  .map_err(store)?;

  answered(json!({ "withdrawn": taken > 0 }))
}

/// The notice queued for the room's voice, answered with the id of the instruction it was queued as.
pub async fn notice(State(served): State<Served>, Path(chat): Path<i64>, body: Bytes) -> Accepted {
  let asked = read::<Notice>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();
  let agent = agents::voice_of(&receiver, chat).await.map_err(reached)?;
  let text = frame(
    &json!(asked.fact).to_string(),
    &asked.intent,
    asked.follow_up.as_deref(),
    asked.optional,
  );
  let queued = Queued {
    kind: asked.kind.as_deref(),
    expires_at_ms: asked.expires_at,
    events: asked.events.clone(),
    rides_until_ms: asked.rides_until,
    sort: Some(NOTICE_SORT),
    ..Queued::default()
  };

  queue(receiver.db(), &agent, chat, &text, &queued)
    .await
    .map(|id| (StatusCode::CREATED, axum::Json(json!({ "id": id }))))
    .map_err(store)
}

/// The unit declared a holder of rows naming a room; its earlier declaration replaced.
pub async fn declare(State(served): State<Served>, Path(unit): Path<String>, body: Bytes) -> Accepted {
  let asked = read::<Holder>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();

  settings::set(receiver.db(), &key(&unit), &json!(asked).to_string())
    .await
    .map(|_| (StatusCode::NO_CONTENT, axum::Json(json!({}))))
    .map_err(store)
}

pub fn key(unit: &str) -> String {
  format!("{HOLDER_KEY}{unit}")
}

//! The instruction queue (`docs/api.md`, 「Instructions」): an instruction queued for an agent's session in a room,
//! its state read, one not yet taken withdrawn, and the notices a room was voiced. Bodies are camelCase, times are
//! epoch milliseconds and every refusal is the common error shape.
//!
//! An instruction whose `said` holds a fact is framed the way every voiced notice is, so the room's voice says it;
//! one whose `said` holds an order goes to the session as the words it was given. One for a room whose
//! registration is gone goes to the owner's private chat instead, naming the room, never into the platform room;
//! one whose agent no longer answers its room goes to the room's voice.

use axum::{
  body::Bytes,
  extract::{Path, RawQuery, State},
  http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use receiver_codes::{Code, FixedLine};
use receiver_policy::chat_round::CHAT_ROUND;

use super::{Accepted, Answered, Refusal, Served, answered, query, read, refused, store};
use crate::chat::instructions::{Cause, Queued, queue, queue_told};
use crate::receiver::notices::frame;
use crate::receiver::wake::agents;
use crate::receiver::{NotReceived, Receiver, rows};
use crate::routes::writable::WritableRoom;
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
  refused(StatusCode::BAD_GATEWAY, "upstream-failed", &why.to_string())
}

/// The room, message, instruction or run named does not exist.
fn absent(what: &str) -> Refusal {
  refused(StatusCode::NOT_FOUND, "not-found", &format!("Nothing stands under {what}."))
}

/// What an instruction hands the session: a fact, or the words of an order.
#[derive(Deserialize)]
struct Said {
  #[serde(default)]
  fact: Option<Value>,

  #[serde(default)]
  order: Option<String>,
}

/// One instruction as a unit queues it.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Instruction {
  #[serde(default)]
  agent_name: Option<String>,
  #[serde(default)]
  room: Option<i64>,
  said: Said,
  #[serde(default)]
  kind: Option<String>,
  sort: String,
  #[serde(default)]
  summary: Option<String>,
  #[serde(default)]
  cause: Option<String>,
  #[serde(default)]
  events: Vec<String>,
  #[serde(default)]
  agent_only: bool,
  #[serde(default)]
  rides_until: Option<i64>,
  /// Whether it rides its room's rounds for good, opening no turn of its own.
  #[serde(default)]
  rides_for_good: bool,
  #[serde(default)]
  via: Option<String>,
  #[serde(default)]
  run: Option<i64>,
  /// The unit that published it, kept with the row and named in every turn it rides.
  #[serde(default)]
  publisher: Option<String>,
  /// A fixed line the room hears at once as the service's own, as the row is written, for a fact a person must not
  /// wait for a round to hear.
  #[serde(default)]
  line: Option<Value>,
}

/// One notice as a room voices it.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
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
pub struct Holder {
  address: String,

  #[serde(default)]
  holdings: Vec<Hold>,
}

#[derive(Deserialize, Serialize)]
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

// Where a job's status notice goes: the room's answerer, then the requester's private room, then the default answerer.
// An `agentName` that answers the room is kept, and one that does not is replaced by this order.
async fn destination(
  receiver: &Receiver,
  room: &rows::KeptWorkRoom,
  asked: Option<&str>,
) -> Result<(i64, String), NotReceived> {
  let said = room.chat_id;

  if let Some(agent) = asked
    && agents::answers(receiver, agent, said).await?
  {
    return Ok((said, agent.to_owned()));
  }

  if agents::answerer(receiver, said).await?.is_some() {
    return Ok((said, agents::voice_of(receiver, said).await?));
  }

  let private = receiver.owner_room();

  if private != said && agents::answerer(receiver, private).await?.is_some() {
    return Ok((private, agents::voice_of(receiver, private).await?));
  }

  Ok((private, receiver.default_answerer()))
}

/// The instruction queued, answered with its id. A job's status notice goes to the room's answerer; where the
/// room names none, to the requester's private room, and where that names none, to the default answerer. Every other
/// instruction keeps the room it names. One for a room whose registration is gone goes to the owner's private
/// room, opening with the room's number (docs/receiver.md, 「Removing a room's registration」).
pub async fn queue_instruction(State(served): State<Served>, body: Bytes) -> Accepted {
  let asked = read::<Instruction>(&body)?;
  let line = asked.line.clone().map(asked_line).transpose()?;
  let receiver: Receiver = (*served.receiver).clone();
  let room_id = asked.room.unwrap_or(receiver.owner_room());
  let room = rows::kept_work_room(receiver.db(), room_id, receiver.owner_room())
    .await
    .map_err(store)?;
  let mut text = said(&asked.said);
  let mut queued = Queued {
    kind: asked.kind.as_deref(),
    events: asked.events.clone(),
    rides_until_ms: asked.rides_until,
    rides_session: false,
    rides_for_good: asked.rides_for_good,
    agent_only: asked.agent_only,
    cause: Cause::of_word(asked.cause.as_deref().unwrap_or_default()),
    sort: Some(&asked.sort),
    summary: asked.summary.as_deref(),
    via: asked.via.as_deref(),
    run_id: asked.run,
    publisher: asked.publisher.as_deref(),
  };

  if let Some(removed) = room.removed {
    let told_here = CHAT_ROUND.instructions.room_removed.replace("{room}", &removed.to_string());

    text = format!("{told_here}\n\n{text}");
    // The removed room's road is not the owner's, and its kind must not replace a pending row of the owner's own.
    queued.via = None;
    queued.kind = None;
  }

  // A job's status notice follows the requester; every other instruction keeps the room it names, whose voice is
  // the room's answerer or the default answerer.
  let (chat_id, agent) = if asked.sort == "job" && asked.said.order.is_some() {
    destination(&receiver, &room, asked.agent_name.as_deref()).await.map_err(reached)?
  } else {
    let agent = match room.removed {
      None => match asked.agent_name.clone() {
        None => agents::voice_of(&receiver, room.chat_id).await.map_err(reached)?,
        Some(asker) if agents::answers(&receiver, &asker, room.chat_id).await.map_err(reached)? => asker,
        Some(_) => agents::voice_of(&receiver, room.chat_id).await.map_err(reached)?,
      },
      Some(_) => agents::voice_of(&receiver, room.chat_id).await.map_err(reached)?,
    };

    (room.chat_id, agent)
  };
  let (id, written) = queue_told(receiver.db(), &agent, chat_id, &text, &queued).await.map_err(store)?;

  // The line is said once, with the row written, into the room the row names: a repeat says nothing again, and a
  // removed room's row goes to the owner's room, where the line is not his to hear.
  if let Some(line) = line.filter(|_| written && room.removed.is_none())
    && let Err(why) = receiver.speech().say_line(chat_id, &line).await
  {
    println!("chat {chat_id}: the {} line beside instruction {id} went unsaid: {why}", line.code());
  }

  // A room reply, or a quiet instruction, opens the agent's turn there at once; housekeeping and a row that rides
  // wait for the pass.
  let replies = queued.cause == Cause::Reply
    && !queued.agent_only
    && queued.rides_until_ms.is_none()
    && !queued.rides_for_good;

  if (replies || matches!(asked.sort.as_str(), "watch" | "reminder"))
    && let Some(pass) = receiver.pass()
  {
    pass.kick();
  }

  Ok((StatusCode::CREATED, axum::Json(json!({ "id": id }))))
}

/// A fixed line an instruction carries, refused 400 where it is no line a unit may ask the room to hear.
fn asked_line(line: Value) -> Result<FixedLine, Refusal> {
  match serde_json::from_value::<FixedLine>(line) {
    Ok(line) if line.asked_by_unit() => Ok(line),
    Ok(line) => Err(refused(
      StatusCode::BAD_REQUEST,
      "bad-request",
      &format!("the {} line is the receiver's own to say", line.code()),
    )),
    Err(why) => Err(refused(
      StatusCode::BAD_REQUEST,
      "bad-request",
      &format!("the line does not read: {why}"),
    )),
  }
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

/// How far the instruction named has gone; nothing found where it was let go: withdrawn, replaced or lapsed, its
/// row kept all the same.
pub async fn instruction_state(State(served): State<Served>, Path(id): Path<i64>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();

  let row = one(
    receiver.db(),
    r#"SELECT "handed_at", "delivered_at" FROM "instructions" WHERE "id" = $1 AND "let_go" IS NULL"#,
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

  answered(json!({ "id": id, "state": stood }))
}

/// The instruction not yet taken withdrawn. One handed already or gone is no refusal: it is no longer pending.
pub async fn withdraw(State(served): State<Served>, Path(id): Path<i64>) -> Accepted {
  let receiver: Receiver = (*served.receiver).clone();

  run(
    receiver.db(),
    r#"UPDATE "instructions" SET "delivered_at" = $2, "let_go" = $3
       WHERE "id" = $1 AND "handed_at" IS NULL AND "delivered_at" IS NULL"#,
    vec![id.into(), crate::now_ms().into(), crate::chat::instructions::why::WITHDRAWN.into()],
  )
  .await
  .map_err(store)?;

  Ok((StatusCode::NO_CONTENT, axum::Json(json!({}))))
}

/// The notice queued for the room's voice, answered with the id of the instruction it was queued as.
pub async fn notice(State(served): State<Served>, writable: WritableRoom) -> Accepted {
  let room = writable.room;
  let asked = read::<Notice>(writable.writer.body())?;
  let receiver: Receiver = (*served.receiver).clone();
  let agent = agents::voice_of(&receiver, room).await.map_err(reached)?;
  let text = frame(
    &json!(asked.fact).to_string(),
    &asked.intent,
    asked.follow_up.as_deref(),
    asked.optional,
  );
  let queued = Queued {
    kind: asked.kind.as_deref(),
    events: asked.events.clone(),
    rides_until_ms: asked.rides_until,
    sort: Some(NOTICE_SORT),
    ..Queued::default()
  };

  queue(receiver.db(), &agent, room, &text, &queued)
    .await
    .map(|id| (StatusCode::CREATED, axum::Json(json!({ "id": id }))))
    .map_err(store)
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NoticeQuery {
  #[serde(default)]
  before: Option<i64>,

  #[serde(default)]
  after: Option<i64>,

  #[serde(default)]
  limit: Option<i64>,

  #[serde(default)]
  kind: Option<String>,

  /// The span a notice was queued in, in epoch milliseconds, both ends included.
  #[serde(default)]
  from: Option<i64>,

  #[serde(default)]
  to: Option<i64>,
}

/// The notices a room was voiced, oldest first, by `before` or `after` and `limit`.
pub async fn notices(State(served): State<Served>, Path(room): Path<i64>, raw: RawQuery) -> Answered {
  let asked = query::<NoticeQuery>(raw)?;
  let bound = i64::min(asked.limit.unwrap_or(50), 500) + 1;
  let mut conditions = vec![r#""chat_id" = $1"#.to_owned(), r#""sort" = 'notice'"#.to_owned()];
  let mut values: Vec<sea_orm::Value> = vec![room.into()];

  if let Some(kind) = &asked.kind {
    values.push(kind.clone().into());
    conditions.push(format!(r#""kind" = ${}"#, values.len()));
  }
  if let Some(from) = asked.from {
    values.push(from.into());
    conditions.push(format!(r#""created_at" >= ${}"#, values.len()));
  }
  if let Some(to) = asked.to {
    values.push(to.into());
    conditions.push(format!(r#""created_at" <= ${}"#, values.len()));
  }

  let (order, cursor) = match (asked.before, asked.after) {
    (Some(before), _) => {
      values.push(before.into());
      conditions.push(format!(r#""id" < ${}"#, values.len()));
      ("DESC", true)
    }
    (None, Some(after)) => {
      values.push(after.into());
      conditions.push(format!(r#""id" > ${}"#, values.len()));
      ("ASC", false)
    }
    (None, None) => ("DESC", true),
  };

  values.push(bound.into());
  let sql = format!(
    r#"SELECT "id", "kind", "summary", "text", "created_at" FROM "instructions"
       WHERE {} ORDER BY "id" {} LIMIT ${}"#,
    conditions.join(" AND "),
    order,
    values.len(),
  );
  let receiver: Receiver = (*served.receiver).clone();
  let rows = crate::all(receiver.db(), &sql, values).await.map_err(store)?;
  let more = i64::try_from(rows.len()).unwrap_or(0) == bound;
  let kept: Vec<_> = if cursor {
    rows.into_iter().take(usize::try_from(bound - 1).unwrap_or(0)).collect()
  } else {
    rows
  };
  let mut items: Vec<Value> = kept
    .iter()
    .map(|row| {
      let created: i64 = row.try_get("", "created_at").unwrap_or(0);

      json!({
        "id": row.try_get::<i64>("", "id").ok(),
        "kind": row.try_get::<Option<String>>("", "kind").ok().flatten(),
        "summary": row.try_get::<Option<String>>("", "summary").ok().flatten(),
        "text": row.try_get::<String>("", "text").unwrap_or_default(),
        "createdAt": created,
      })
    })
    .collect();
  if cursor {
    items.reverse();
  }

  answered(json!({ "items": items, "more": more }))
}

/// The unit declared a holder of rows naming a room; its earlier declaration replaced.
pub async fn declare(State(served): State<Served>, Path(holder): Path<String>, body: Bytes) -> Accepted {
  let asked = read::<Holder>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();

  settings::set(receiver.db(), &key(&holder), &json!(asked).to_string())
    .await
    .map(|_| (StatusCode::NO_CONTENT, axum::Json(json!({}))))
    .map_err(store)
}

pub fn key(unit: &str) -> String {
  format!("{HOLDER_KEY}{unit}")
}

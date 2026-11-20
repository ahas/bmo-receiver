//! A room's tone layer as the console reads and sets it (docs/receiver.md, 「A room's tone layer」; docs/api.md,
//! 「Rooms」): the layer the room is set to and the layers the deployment offers, and the administrator's change
//! of it. The layers offered are the configuration's, so no route names one.

use axum::{
  extract::{Path, RawQuery, State},
  http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Answered, Refusal, Served, answered, query, read, refused, store};
use crate::chat::tone;
use crate::receiver::Receiver;
use crate::roads::added;
use crate::routes::say::known;
use crate::routes::writable::WritableRoom;

/// The administrator a change is asked by, as the console names him.
#[derive(Deserialize, Default)]
struct ByAdministrator {
  #[serde(default)]
  by: String,
}

/// The tone layer a room is to stand in: a layer's name, or null for none. The field is required, so a body
/// that leaves it out is refused rather than read as none; the account the write acts for may stand beside it,
/// which the write guard reads.
#[derive(Deserialize)]
struct Setting {
  tone: Value,
}

// The room's tone layer as the read and the change answer it, with the layers the deployment offers.
async fn standing(receiver: &Receiver, room: i64) -> Answered {
  let set = tone::tone_of(receiver.db(), room).await.map_err(store)?;

  answered(json!({ "room": room, "tone": set, "tones": receiver.deployment().tones }))
}

// A room the receiver holds: one its record keeps, or one added.
async fn held(receiver: &Receiver, room: i64) -> Result<(), Refusal> {
  if known(receiver, room).await? || added::mode_of(receiver.db(), room).await.map_err(store)?.is_some() {
    return Ok(());
  }

  Err(refused(
    StatusCode::NOT_FOUND,
    "not-found",
    &format!("The receiver holds no room {room}."),
  ))
}

// The layer a body names, where the deployment offers it; none for null.
fn named(receiver: &Receiver, setting: &Setting) -> Result<Option<String>, Refusal> {
  match &setting.tone {
    Value::Null => Ok(None),
    Value::String(name) if receiver.deployment().tones.contains(name) => Ok(Some(name.clone())),
    Value::String(name) => Err(refused(
      StatusCode::BAD_REQUEST,
      "unknown-tone",
      &format!("the deployment offers no tone layer {name}"),
    )),
    _ => Err(refused(StatusCode::BAD_REQUEST, "bad-request", "a tone is a layer's name or null")),
  }
}

/// `GET /rooms/{roomId}/tone`: the tone layer a room is set to, and the layers it may be set to.
pub async fn room_tone(State(served): State<Served>, Path(room): Path<i64>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();

  standing(&receiver, room).await
}

/// `PUT /rooms/{roomId}/tone?by=`: a room set to a tone layer the deployment offers, or to none, by the
/// administrator alone. The room's block carries the new layer from its next turn.
pub async fn change_room_tone(
  State(served): State<Served>,
  raw: RawQuery,
  writable: WritableRoom,
) -> Answered {
  let room = writable.room;
  let asked = query::<ByAdministrator>(raw)?;
  let setting = read::<Setting>(writable.writer.body())?;
  let receiver: Receiver = (*served.receiver).clone();

  if !super::is_administrator(&receiver, asked.by.trim()).await {
    return Err(refused(
      StatusCode::FORBIDDEN,
      "not-administrator",
      "only the administrator changes a room's tone layer",
    ));
  }

  held(&receiver, room).await?;

  let tone = named(&receiver, &setting)?;

  tone::set(receiver.db(), room, tone.as_deref(), crate::now_ms()).await.map_err(store)?;

  standing(&receiver, room).await
}

//! A line another unit asks to be kept in an agent's owner's private room (docs/api.md, 「Kept lines」): one fixed
//! line that unit composes, kept in that room's record alone and sent to no chat service, so a screen reading the
//! room shows it and no person is messaged. The room is the agent's own room on the record, where it has one,
//! else the owner's private room.

use axum::{
  body::Bytes,
  extract::{Path, State},
  http::StatusCode,
  response::{IntoResponse, Json, Response},
};
use receiver_codes::{Code, FixedLine};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::accounts::room_of_supervisor;
use crate::receiver::speech::NotSaid;
use crate::routes::{Refusal, Served, read, refused, store};

#[derive(Deserialize)]
pub struct Asked {
  line: Value,
}

/// 201 with the room the line was kept in and its number there, or none where no sentence could be made of it.
pub async fn keep(
  State(served): State<Served>,
  Path(agent): Path<String>,
  body: Bytes,
) -> Result<Response, Refusal> {
  let asked = read::<Asked>(&body)?;
  let line = match serde_json::from_value::<FixedLine>(asked.line) {
    Ok(line) if line.kept_by_unit() => line,
    Ok(line) => {
      return Err(refused(
        StatusCode::BAD_REQUEST,
        "bad-request",
        &format!("the {} line is not one a unit asks to be kept", line.code()),
      ));
    }
    Err(why) => {
      return Err(refused(
        StatusCode::BAD_REQUEST,
        "bad-request",
        &format!("the line does not read: {why}"),
      ));
    }
  };
  let receiver = served.receiver.as_ref();
  let room = room_of_supervisor(receiver.db(), &agent)
    .await
    .map_err(store)?
    .unwrap_or_else(|| receiver.owner_room());

  match receiver.speech().keep_line(room, &line).await {
    Ok(number) => {
      println!("{agent}: a {} line kept in the room {room}", line.code());

      Ok((StatusCode::CREATED, Json(json!({ "room": room, "messageId": number }))).into_response())
    }
    Err(why @ (NotSaid::Archived { .. } | NotSaid::NoRoad { .. })) => {
      Err(refused(StatusCode::CONFLICT, "conflict", &why.to_string()))
    }
    Err(why) => Err(refused(StatusCode::BAD_GATEWAY, "upstream-failed", &why.to_string())),
  }
}

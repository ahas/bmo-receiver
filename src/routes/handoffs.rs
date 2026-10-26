//! A handoff asked for ahead of a fresh session (docs/api.md, 「Handoffs」; docs/unified-bmo.md, 「Replacing the
//! session」): the agency, before it opens a fresh session for an agent that had one, asks the receiver to have the
//! agent's standing session write its handoff file as a replacement does, and the file is kept for the next fresh
//! session's first turn.

use axum::{
  body::Bytes,
  extract::{Path, State},
  http::StatusCode,
  response::{IntoResponse, Json, Response},
};
use serde::Deserialize;
use serde_json::json;

use crate::chat::ask::Carry;
use crate::chat::handoff::Asking;
use crate::routes::{Refusal, Served, read, refused};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asked {
  reason: String,
  #[serde(default)]
  room: Option<i64>,
}

/// 200 with the file's path and size where the standing session wrote it, 204 where the agent has no session to
/// ask or it wrote nothing, 409 while a handoff of the same agent is under way.
pub async fn hand_off(
  State(served): State<Served>,
  Path(agent): Path<String>,
  body: Bytes,
) -> Result<Response, Refusal> {
  let asked = read::<Asked>(&body)?;

  if asked.reason.trim().is_empty() {
    return Err(refused(StatusCode::BAD_REQUEST, "bad-request", "reason (non-empty string) is required"));
  }

  let Some(_asking) = Asking::begin(&agent) else {
    return Err(refused(
      StatusCode::CONFLICT,
      "conflict",
      &format!("A handoff of {agent} is already under way."),
    ));
  };

  println!("{agent}: a handoff asked ahead of a fresh session ({})", asked.reason.trim());

  let carry = Carry::new(served.receiver.as_ref().clone());
  let written = carry
    .hand_off(&agent, asked.room)
    .await
    .map_err(|why| refused(StatusCode::BAD_GATEWAY, "upstream-failed", &why.reason()))?;

  Ok(match written {
    Some(handed) => Json(json!({
      "path": handed.path.to_string_lossy(),
      "bytes": handed.text.len(),
    }))
    .into_response(),
    None => StatusCode::NO_CONTENT.into_response(),
  })
}

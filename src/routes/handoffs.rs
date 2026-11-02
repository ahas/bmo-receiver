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
use crate::chat::handoff::{self, Asking};
use crate::routes::{Refusal, Served, read, refused};
use receiver_policy::handover::HANDOVER;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asked {
  reason: String,
  #[serde(default)]
  room: Option<i64>,
}

/// 200 with the file's path and size where the standing session wrote it, 204 where the agent has no session to
/// ask or it wrote nothing. An asking while a handoff of the same agent is under way waits for it in line, and is
/// then answered by the handoff kept, as an asking again within the policy's span is.
pub async fn hand_off(
  State(served): State<Served>,
  Path(agent): Path<String>,
  body: Bytes,
) -> Result<Response, Refusal> {
  let asked = read::<Asked>(&body)?;

  if asked.reason.trim().is_empty() {
    return Err(refused(StatusCode::BAD_REQUEST, "bad-request", "reason (non-empty string) is required"));
  }

  let _asking = Asking::begin(&agent).await;

  let receiver = served.receiver.as_ref();
  let reason = asked.reason.trim();

  // A handoff is asked at most once in the policy's span: an asking again within it is answered with the one kept,
  // and counted, so a road that cannot keep its session shows instead of asking turn after turn.
  let written = match handoff::in_a_row(receiver, &agent, "handoff", false).await {
    Some(times) => {
      eprintln!(
        "{agent}: a handoff asked again ({reason}), {times} in a row within {} minutes; the session is not asked \
         again and the one kept stands: the road its program stands on may not keep a session",
        HANDOVER.again_minutes
      );
      handoff::kept(receiver, &agent).await
    }
    None => {
      println!("{agent}: a handoff asked ahead of a fresh session ({reason})");

      Carry::new(receiver.clone())
        .hand_off(&agent, asked.room)
        .await
        .map_err(|why| refused(StatusCode::BAD_GATEWAY, "upstream-failed", &why.reason()))?
    }
  };

  Ok(match written {
    Some(handed) => Json(json!({
      "path": handed.path.to_string_lossy(),
      "bytes": handed.text.len(),
    }))
    .into_response(),
    None => StatusCode::NO_CONTENT.into_response(),
  })
}

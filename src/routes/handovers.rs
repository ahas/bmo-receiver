//! A handover asked for an agent's one session (docs/api.md, 「Handovers」; docs/unified-bmo.md, 「Replacing the
//! session」): the owner's word to start the session afresh is written with the reason `asked`, and the session is
//! replaced through its handoff once the agent is quiet: at once where it is, else when the round it is in ends.
//! The route waits on no turn, so the agent may ask it of its own session from inside its own turn.

use axum::{
  body::Bytes,
  extract::{Path, State},
  http::StatusCode,
  response::{IntoResponse, Json, Response},
};
use receiver_codes::SessionReason;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::chat::cache::{force_reset, owe_replacement};
use crate::routes::{Refusal, Served, read, refused, store, unit};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asked {
  reason: String,
}

/// 202 with the agent and the reason its replacement goes by, once the word is written; the agency's listing
/// says whether the agent stands and keeps one session across its rooms.
pub async fn hand_over(
  State(served): State<Served>,
  Path(agent): Path<String>,
  body: Bytes,
) -> Result<Response, Refusal> {
  let asked = read::<Asked>(&body)?;
  let reason = asked.reason.trim();

  if reason.is_empty() {
    return Err(refused(StatusCode::BAD_REQUEST, "bad-request", "reason (non-empty string) is required"));
  }

  let receiver = served.receiver.as_ref();
  let listed = receiver.agents().await.map_err(unit)?;
  let Some(found) = listed.agents.iter().find(|one| one.name == agent) else {
    return Err(refused(
      StatusCode::NOT_FOUND,
      "not-found",
      &format!("The agency registers no agent {agent}."),
    ));
  };

  if found.row.get("sessionScope").and_then(Value::as_str) != Some("agent") {
    return Err(refused(
      StatusCode::CONFLICT,
      "conflict",
      &format!("{agent} keeps a session in each room; the agency resumes those."),
    ));
  }

  force_reset(receiver.db(), &agent, SessionReason::Asked).await.map_err(store)?;
  owe_replacement(receiver.db(), &agent, SessionReason::Asked).await.map_err(store)?;
  println!("{agent}: a handover asked ({reason}); the session hands over once the agent is quiet");

  // A quiet agent's session is replaced now, apart from this answer; one in a round hands over as the round ends.
  if let Some(touching) = receiver.touching() {
    let receiver = served.receiver.clone();
    let agent = agent.clone();

    tokio::spawn(async move {
      touching.settle_owed(&receiver, &agent).await;
    });
  }

  Ok(
    (
      StatusCode::ACCEPTED,
      Json(json!({
        "agentName": agent,
        "sessionReason": SessionReason::Asked.to_string(),
      })),
    )
      .into_response(),
  )
}

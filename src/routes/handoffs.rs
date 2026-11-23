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
/// ask or it wrote nothing. A round doing its own work between two turns is waited for, one that itself waits on
/// the agency is not, and a session already in a turn is waited for until that turn ends. An asking while a
/// handoff of the same agent is under way waits for it in line, and is then answered by the handoff kept, as an
/// asking again within the policy's span is; one made while that handoff's turn waits on the agency comes from
/// inside it and is answered 204 at once. An asking still waiting as the service is told to stop is answered
/// 503 `stopping`.
pub async fn hand_off(
  State(served): State<Served>,
  Path(agent): Path<String>,
  body: Bytes,
) -> Result<Response, Refusal> {
  let asked = read::<Asked>(&body)?;

  if asked.reason.trim().is_empty() {
    return Err(refused(StatusCode::BAD_REQUEST, "bad-request", "reason (non-empty string) is required"));
  }

  let receiver = served.receiver.as_ref();

  // The asking waits without a bound, so the service's stop ends it: a stop never waits on a handoff.
  let written = tokio::select! {
    written = asked_of(receiver, &agent, asked.reason.trim(), asked.room) => written?,
    () = receiver.stopped() => {
      return Err(refused(
        StatusCode::SERVICE_UNAVAILABLE,
        "stopping",
        "the receiver is stopping, so the handoff is not asked",
      ));
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

// The handoff asked in the agent's line: the standing session's file, the one kept where it was asked within the
// policy's span, none where there was nothing to ask.
async fn asked_of(
  receiver: &crate::receiver::Receiver,
  agent: &str,
  reason: &str,
  room: Option<i64>,
) -> Result<Option<handoff::Handoff>, Refusal> {
  // An asking from inside the handoff under way is answered at once: that handoff's note is kept for the fresh
  // session all the same.
  let Some(_asking) = Asking::begin(receiver, agent).await else {
    eprintln!(
      "{agent}: a handoff asked ({reason}) from inside the handoff under way, which waits on the agency; it is \
       answered at once and the handoff under way writes the note"
    );

    return Ok(None);
  };

  // A handoff is asked at most once in the policy's span: an asking again within it is answered with the one kept,
  // and counted, so a road that cannot keep its session shows instead of asking turn after turn.
  match handoff::in_a_row(receiver, agent, "handoff", false).await {
    Some(times) => {
      eprintln!(
        "{agent}: a handoff asked again ({reason}), {times} in a row within {} minutes; the session is not asked \
         again and the one kept stands: the road its program stands on may not keep a session",
        HANDOVER.again_minutes
      );

      Ok(handoff::kept(receiver, agent).await)
    }
    None => {
      println!("{agent}: a handoff asked ahead of a fresh session ({reason})");

      // The asking stands behind the agent's round, as an owed replacement does, wherever its conversations
      // stand.
      let handed = match receiver.touching() {
        Some(touching) => touching.hand_off(receiver, agent, room).await,
        None => Carry::new(receiver.clone()).hand_off(agent, room).await,
      };

      handed.map_err(|why| refused(StatusCode::BAD_GATEWAY, "upstream-failed", &why.reason()))
    }
  }
}

//! Who said a kept message, corrected on the record (docs/api.md, 「Messages」; docs/messages.md, 「The archive」): a
//! line standing under an agent's own number in its room filed again as that agent's, under the name its words
//! stand under and the voice its road answers the room by. Nothing goes to a chat service; the room's screens are
//! told the message changed.

use axum::{
  extract::{Path, State},
  http::StatusCode,
};
use receiver_codes::MessageActRefusal;
use serde::Deserialize;
use serde_json::json;

use super::rooms::message_shape;
use super::writable::WritableRoom;
use super::{Answered, Refusal, Served, answered, code, read, refused, store};
use crate::receiver::wake::agents;
use crate::receiver::{NotReceived, Receiver, agent_pictures, rows};
use crate::roads::added;

// A correction that could not be made because a unit it reaches did not answer.
fn reached(why: NotReceived) -> Refusal {
  refused(StatusCode::BAD_GATEWAY, "upstream-failed", &why.to_string())
}

/// The agent a kept line is to be filed as.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Speaking {
  agent_name: String,
}

// The number an agent's own words stand under in a room on one road, as `GET /rooms/{roomId}/own-number` reads it
// from the road: the room's own number for the default answerer on the room's own road, the number the road gave
// the agent otherwise; none for an agent the agency does not register.
async fn own_number(
  receiver: &Receiver,
  room: i64,
  via: &str,
  agent: &str,
) -> Result<Option<i64>, Refusal> {
  let Some(designated) = agents::registered(receiver, agent).await.map_err(reached)? else {
    return Ok(None);
  };
  let identity = if receiver.roads().room_road(room).await.as_deref() == Some(via) {
    receiver.roads().identity_in(room).await
  } else {
    None
  };

  agents::own_number(receiver, &designated, via, identity).await.map_err(reached)
}

/// `PUT /rooms/{roomId}/messages/{messageId}/speaker`: a kept line filed as the agent named, where it stands under
/// that agent's own number in the room on the road it came by; its sender, words and number are left as they are.
pub async fn change_speaker(
  State(served): State<Served>,
  Path((_, message)): Path<(i64, i64)>,
  writable: WritableRoom,
) -> Answered {
  let room = writable.room;
  let asked = read::<Speaking>(writable.writer.body())?;
  let receiver: Receiver = (*served.receiver).clone();
  let agent = asked.agent_name.trim();

  if agent.is_empty() {
    return Err(refused(StatusCode::BAD_REQUEST, "bad-request", "A speaker names an agent."));
  }

  let Some(kept) = rows::message(receiver.db(), room, message, "").await.map_err(store)? else {
    return Err(code(&MessageActRefusal::MessageUnknown { room, message }));
  };
  let own = own_number(&receiver, room, &kept.via, agent).await?;

  if own.is_none() || kept.sender_id != own {
    return Err(code(&MessageActRefusal::MessageNotAgents {
      room,
      message,
      agent: agent.to_owned(),
    }));
  }

  let name = receiver.speech().name_of(agent).await;
  let voice = added::voice_in(receiver.db(), receiver.roads(), &kept.via, room)
    .await
    .and_then(added::voice_key);

  rows::set_speaker(receiver.db(), room, message, &name, agent, voice.as_deref())
    .await
    .map_err(store)?;

  if let Some(number) = kept.sender_id {
    agent_pictures::worn_by(receiver.db(), receiver.news(), agent, &[number]).await;
  }
  receiver.news().publish(&crate::events::message_changed(room, message));

  answered(
    message_shape(&receiver, room, message)
      .await
      .map_err(store)?
      .unwrap_or_else(|| json!({})),
  )
}

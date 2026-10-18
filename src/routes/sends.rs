//! A send asked of the running receiver (docs/boundary-api.md, 「Sends」; docs/chat-mcp.md, 「How a call runs」):
//! a line said, a picture or an album, a reminder set, a reaction, a pin, a sticker or a fair draw, carried on the
//! receiver's own lines as the agent the asker names, on the road the turn names — since a sending tool's process
//! and a shell's hold none of them — and the verbs the road a room is answered on performs, which a procedure read
//! there is held to.
//!
//! A line asked to be said is said in a run of its own, whose launcher the receiver's binary fills; asked of a
//! service that stands without one, it is turned away before the words are put on the disk.

use axum::{body::Bytes, extract::State, http::StatusCode};
use serde::Deserialize;
use serde_json::json;

use room_calls::sends::{Act, Asked, Asker, RemindInput};
use room_calls::turn::{NO_AGENT, NO_ROOM};

use receiver_ports::brain::Reminder;
use receiver_ports::http::PortFailure;

use super::{Answered, Served, answered, read, refused};
use crate::receiver::Receiver;
use crate::verbs::sends::Sends;
use crate::verbs::{VerbError, draws, messages};

// One send carried as its asker, answering the lines it said, or those lines with the refusal it ended in.
async fn carried(receiver: &Receiver, asked: &Asked) -> Result<Vec<String>, VerbError> {
  let Asked { asker, act } = asked;
  let chat = asker.chat.ok_or_else(|| VerbError::Refused(NO_ROOM.to_owned()))?;
  let sends = Sends::new(receiver.clone(), asker.agent.clone(), asker.via.clone());

  match act {
    Act::Say(input) => {
      let asked = messages::SayAsked {
        chat_id: chat,
        text: input.text.clone(),
        reply: input.reply,
        started_by: asker.by.clone(),
      };

      messages::say(&sends, &asked, asker.via.as_deref()).await
    }
    Act::Photo(input) => {
      let pictures: Vec<messages::Picture> = input
        .pictures
        .iter()
        .map(|picture| messages::Picture {
          url: picture.url.clone(),
          label: picture.label.clone(),
        })
        .collect();

      messages::photo(&sends, receiver.data_root(), chat, &pictures, input.caption.as_deref(), asker.job)
        .await
    }
    Act::Remind(input) => remind(receiver, asker, chat, input).await,
    Act::React(input) => messages::react(&sends, chat, input.message, &input.emoji).await,
    Act::Pin(input) => messages::pin(&sends, chat, input.message, input.unpin).await,
    Act::Sticker(input) => messages::sticker(&sends, chat, &input.emoji).await,
    Act::RandomDraw(input) => {
      draws::draw(&sends, chat, Some(input.mode.word()), &input.request(), asker.via.as_deref()).await
    }
  }
}

// A reminder the asker's agent sets in its room, carried to the brain and answered with the reading it gives.
async fn remind(
  receiver: &Receiver,
  asker: &Asker,
  chat: i64,
  input: &RemindInput,
) -> Result<Vec<String>, VerbError> {
  let Some(agent) = asker.agent.clone() else {
    return Err(VerbError::Refused(NO_AGENT.to_owned()));
  };
  let reminder = Reminder {
    agent,
    chat_id: chat,
    at: input.at.clone(),
    every: input.every.clone(),
    reason: input.reason.clone(),
    memory: input.memory.clone(),
    user: input.user,
  };
  let set = receiver.units().brain.set_reminder(&reminder).await.map_err(port)?;

  set.map(|reading| reading.lines).map_err(|refusal| VerbError::Refused(refusal.message))
}

// A call on the brain that did not answer.
fn port(why: PortFailure) -> VerbError {
  VerbError::Refused(why.to_string())
}

pub async fn sends(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<Asked>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();

  let said = match carried(&receiver, &asked).await {
    Ok(lines) => json!({ "lines": lines }),
    Err(VerbError::Partly { lines, refusal }) => json!({ "lines": lines, "refusal": refusal }),
    Err(why) => json!({ "lines": [], "refusal": why.to_string() }),
  };

  answered(said)
}

#[derive(Deserialize)]
struct CanBody {
  chat: i64,

  #[serde(default)]
  via: Option<String>,
}

pub async fn can(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<CanBody>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();
  let sends = Sends::new(receiver.clone(), None, asked.via.clone());

  match sends.answering_can(asked.chat).await {
    Ok(can) => answered(json!({ "can": can })),
    Err(VerbError::Refused(why)) => Err(refused(StatusCode::CONFLICT, "no-route", &why)),
    Err(why) => Err(refused(StatusCode::CONFLICT, "no-route", &why.to_string())),
  }
}

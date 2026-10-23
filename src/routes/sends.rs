//! A send asked of the running receiver (docs/boundary-api.md, 「Sends」; docs/chat-mcp.md, 「How a call runs」):
//! a line said, a picture or an album, a file or an album of files, a voice note, a reaction, a pin, a sticker,
//! a fair draw or a fixed line, carried on the receiver's own lines as the agent the asker names, on the road the
//! turn names — since a sending tool's process and a shell's hold none of them — and the verbs the road a room is
//! answered on performs, which a procedure read there is held to.
//!
//! A line asked to be said is said in a run of its own, whose launcher the receiver's binary fills; asked of a
//! service that stands without one, it is turned away before the words are put on the disk.

use axum::{
  body::Bytes,
  extract::{Path, RawQuery, State},
  http::StatusCode,
};
use serde::Deserialize;
use serde_json::json;

use room_calls::sends::{Act, Asked};
use room_calls::turn::NO_ROOM;

use super::{Answered, Served, answered, query, read, refused};
use crate::receiver::Receiver;
use crate::verbs::sends::Sends;
use crate::verbs::{VerbError, draws, messages};

// One send carried as its asker, answering the lines it said, or those lines with the refusal it ended in.
async fn carried(receiver: &Receiver, asked: &Asked) -> Result<Vec<String>, VerbError> {
  let Asked { asker, act } = asked;
  let chat = asker.room.ok_or_else(|| VerbError::Refused(NO_ROOM.to_owned()))?;
  let sends = Sends::new(receiver.clone(), asker.agent_name.clone(), asker.via.clone());

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
    Act::File(input) => {
      let files: Vec<messages::SendFile> = input
        .files
        .iter()
        .map(|file| messages::SendFile {
          path: file.path.clone(),
          name: file.name.clone(),
        })
        .collect();

      messages::file(&sends, receiver.data_root(), chat, &files, input.caption.as_deref(), asker.job).await
    }
    Act::Voice(input) => {
      let note = messages::VoiceNote {
        path: &input.path,
        caption: input.caption.as_deref(),
        said: input.said.as_deref(),
        transcript: input.transcript.as_deref(),
      };

      messages::voice(&sends, receiver.data_root(), chat, note, asker.job).await
    }
    Act::React(input) => messages::react(&sends, chat, input.message, &input.emoji).await,
    Act::Pin(input) => messages::pin(&sends, chat, input.message, input.unpin).await,
    Act::Sticker(input) => messages::sticker(&sends, chat, &input.emoji).await,
    Act::RandomDraw(input) => draws::draw(&sends, chat, &input.request(), asker.via.as_deref()).await,
    Act::FixedLine(input) => messages::fixed_line(&sends, chat, &input.code, &input.fields).await,
  }
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

#[derive(Deserialize, Default)]
struct CanBody {
  #[serde(default)]
  via: Option<String>,
}

pub async fn can(State(served): State<Served>, Path(room): Path<i64>, raw: RawQuery) -> Answered {
  let asked = query::<CanBody>(raw)?;
  let receiver: Receiver = (*served.receiver).clone();
  let sends = Sends::new(receiver.clone(), None, asked.via.clone());

  match sends.answering_can(room).await {
    Ok(can) => answered(json!({ "items": can, "more": false })),
    Err(VerbError::Refused(why)) => Err(refused(StatusCode::CONFLICT, "conflict", &why)),
    Err(why) => Err(refused(StatusCode::CONFLICT, "conflict", &why.to_string())),
  }
}

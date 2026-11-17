//! A send asked of the running receiver (docs/boundary-api.md, 「Sends」; docs/chat-mcp.md, 「How a call runs」):
//! a picture or an album, a file or an album of files, a reaction, a pin, a sticker or a fair draw, carried on the
//! receiver's own lines as the agent the asker names, on the road the turn names — since a sending tool's process
//! and a shell's hold none of them — and the verbs the road a room is answered on performs, which a procedure
//! read there is held to.
//!
//! Words go into a room only as a round's message, which the draft process writes and reviews (docs/messages.md,
//! 「Words handed to the room's supervisor」). So only an act that holds no words, asked in its agent's own round,
//! is carried; an act holding words — a line, a spoken talk, a voice note, a caption — asked there into the room
//! that round answers is refused, and every other send is handed to the room's supervisor as an order, whose round
//! there writes it, and nothing goes out on that call. A send an agent asks into a room its voice briefing turn is
//! being carried in is refused before any of these.

use axum::{
  body::Bytes,
  extract::{Path, RawQuery, State},
  http::StatusCode,
};
use receiver_codes::SendRefusal;
use serde::Deserialize;
use serde_json::json;

use room_calls::sends::{Act, Asked};
use room_calls::turn::NO_ROOM;

use super::{Answered, Served, answered, query, read, refused};
use crate::chat::briefing_turn;
use crate::chat::cut_off;
use crate::chat::handed;
use crate::chat::relays::{self, Relay};
use crate::receiver::Receiver;
use crate::verbs::sends::Sends;
use crate::verbs::{VerbError, draws, messages};

// One send that holds no words carried as its asker, answering the lines it said, or those lines with the refusal
// it ended in.
async fn carried(receiver: &Receiver, asked: &Asked) -> Result<Vec<String>, VerbError> {
  let Asked { asker, act } = asked;
  let chat = asker.room.ok_or_else(|| VerbError::Refused(NO_ROOM.to_owned()))?;
  let sends = Sends::new(receiver.clone(), asker.agent_name.clone(), asker.via.clone());

  match act {
    Act::Photo(input) => {
      let pictures: Vec<messages::Picture> = input
        .pictures
        .iter()
        .map(|picture| messages::Picture {
          url: picture.url.clone(),
          label: picture.label.clone(),
        })
        .collect();

      messages::photo(&sends, receiver.data_root(), chat, &pictures, input.reply_to, asker.job).await
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

      messages::file(&sends, receiver.data_root(), chat, &files, input.reply_to, asker.job).await
    }
    Act::React(input) => messages::react(&sends, chat, input.message, &input.emoji).await,
    Act::Pin(input) => messages::pin(&sends, chat, input.message, input.unpin).await,
    Act::Sticker(input) => messages::sticker(&sends, chat, &input.emoji, input.reply_to).await,
    Act::RandomDraw(input) => draws::draw(&sends, chat, &input.request(), asker.via.as_deref()).await,
    // Words are never carried: `sends` refuses or hands an act holding them before it comes here.
    Act::Say(_) | Act::Speak(_) | Act::Voice(_) => Err(VerbError::Refused(words_refusal(asked, chat))),
  }
}

// The refusal of words asked in the agent's own round into the room that round answers.
fn words_refusal(asked: &Asked, chat: i64) -> String {
  SendRefusal::WordsInAnswer {
    agent: asked.asker.agent_name.clone().unwrap_or_default(),
    room: chat,
  }
  .to_string()
}

// One send handed to the room's supervisor, answering the line saying so and the order's number. A line asking
// for an answer back from a round of another room waits on the order for that answer.
async fn handed(receiver: &Receiver, asked: &Asked) -> Result<serde_json::Value, VerbError> {
  let Asked { asker, act } = asked;
  let chat = asker.room.ok_or_else(|| VerbError::Refused(NO_ROOM.to_owned()))?;
  let refused = |why: crate::receiver::NotReceived| VerbError::Refused(why.to_string());
  let agent = handed::supervisor(receiver, asker, chat).await.map_err(refused)?;
  let handing = handed::hand(receiver, asker, &agent, chat, &handed::asked(asker, act))
    .await
    .map_err(refused)?;
  let mut lines = vec![handing.line()];

  if let Act::Say(input) = act
    && input.reply
    && let Some(asking) = handed::turn_room(asker).filter(|asking| *asking != chat)
    && relays::wait_on_order(receiver.db(), chat, asking, handing.id, &input.text)
      .await
      .map_err(VerbError::Store)?
      == Relay::Dropped
  {
    lines.push(handed::relay_dropped());
  }

  Ok(json!({ "lines": lines, "handed": handing.id }))
}

pub async fn sends(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<Asked>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();

  if let Some(refusal) = cut_off::overtaken(&receiver, &asked.asker).await {
    return answered(json!({ "lines": [], "refusal": refusal }));
  }

  if let Some(refusal) = briefing_turn::refused(&receiver, &asked.asker) {
    return answered(json!({ "lines": [], "refusal": refusal }));
  }

  let own_round = handed::own_round(&receiver, &asked.asker);

  if own_round
    && asked.act.holds_words()
    && let Some(chat) = asked.asker.room
    && handed::turn_room(&asked.asker) == Some(chat)
  {
    return answered(json!({ "lines": [], "refusal": words_refusal(&asked, chat) }));
  }

  if !own_round || asked.act.holds_words() {
    return answered(match handed(&receiver, &asked).await {
      Ok(said) => said,
      Err(why) => json!({ "lines": [], "refusal": why.to_string() }),
    });
  }

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

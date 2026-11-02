//! The consult tool's routes (docs/consult.md, 「The consult tool」): a line said in the consult room under an
//! agent's own name, and the block an agent is shown of the other agents' work.

use axum::{
  body::Bytes,
  extract::{RawQuery, State},
  http::StatusCode,
};
use serde::Deserialize;
use serde_json::json;

use super::{Answered, Refusal, Served, answered, code, query, read, refused, store, unit};
use receiver_codes::ConsultRefusal;
use receiver_policy::consult::CONSULT;
use room_calls::sends::Asker;

use crate::chat::consult::consult_block;
use crate::chat::consult_say::say;
use crate::chat::cut_off;
use crate::chat::handed;
use crate::now_ms;
use crate::receiver::{NotReceived, Receiver};
use crate::store::settings;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SayBody {
  agent: String,
  text: String,
  #[serde(default)]
  reply_to: Option<i64>,
  #[serde(default)]
  began: Option<i64>,
  #[serde(default)]
  by: Option<String>,
  #[serde(default)]
  job: bool,
  #[serde(default)]
  from: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BlockBody {
  agent: String,
}

// Why a call the route needed on another unit or the store failed, in the receiver's error shape.
fn failed(why: NotReceived) -> Refusal {
  match why {
    NotReceived::Store(why) => store(why),
    NotReceived::Unit(why) => unit(why),
    NotReceived::Said(why) => {
      let sentence = why.to_string();

      refused(StatusCode::BAD_GATEWAY, "upstream-failed", &sentence)
    }
  }
}

// A line asked outside the agent's own round, handed to that agent in the consult room.
async fn handed_line(receiver: &Receiver, asker: &Asker, asked: &SayBody) -> Answered {
  let set = settings::get(receiver.db(), &CONSULT.room_setting)
    .await
    .map_err(store)?
    .and_then(|stored| stored.trim().parse::<i64>().ok());
  let Some(room) = set else { return Err(code(&ConsultRefusal::NoRoom)) };
  let lines = handed::consulted(asked.text.trim(), asked.reply_to);
  let handing = handed::hand(receiver, asker, asked.agent.trim(), room, &lines).await.map_err(failed)?;

  answered(json!({ "room": room, "id": null, "lines": [handing.line()], "handed": handing.id }))
}

pub async fn consult_say(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<SayBody>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();
  let asker = Asker {
    agent_name: Some(asked.agent.trim().to_owned()),
    began: asked.began,
    by: asked.by.clone(),
    job: asked.job,
    from: asked.from.clone(),
    ..Asker::default()
  };

  if cut_off::overtaken(&receiver, &asker).await.is_some() {
    return Err(code(&ConsultRefusal::Overtaken {
      agent: asked.agent.trim().to_owned(),
    }));
  }

  if !asked.text.trim().is_empty() && !handed::own_round(&receiver, &asker) {
    return handed_line(&receiver, &asker, &asked).await;
  }

  match say(receiver, asked.agent.trim(), &asked.text, asked.reply_to).await {
    Ok(Ok(said)) => answered(json!({
      "room": said.chat_id,
      "id": said.message_id,
      "lines": said.lines,
    })),
    Ok(Err(refusal)) => Err(code(&refusal)),
    Err(why) => Err(failed(why)),
  }
}

pub async fn block(State(served): State<Served>, raw: RawQuery) -> Answered {
  let asked = query::<BlockBody>(raw)?;
  let block = consult_block(&served.receiver, asked.agent.trim(), now_ms()).await.map_err(store)?;

  answered(json!({ "block": block }))
}

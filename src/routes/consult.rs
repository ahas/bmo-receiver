//! The consult tool's routes (docs/consult.md, 「The consult tool」): a line said in the consult room under an
//! agent's own name, and the block an agent is shown of the other agents' work.

use axum::{body::Bytes, extract::State, http::StatusCode};
use serde::Deserialize;
use serde_json::json;

use super::{Answered, Refusal, Served, answered, code, read, refused, store, unit};
use crate::companion::consult::consult_block;
use crate::companion::consult_say::say;
use crate::now_ms;
use crate::receiver::{NotReceived, Receiver};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SayBody {
  agent: String,
  text: String,
  reply_to: Option<i64>,
}

#[derive(Deserialize)]
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

      refused(StatusCode::BAD_GATEWAY, "unit-failed", &sentence)
    }
  }
}

pub async fn consult_say(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<SayBody>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();

  match say(receiver, asked.agent.trim(), &asked.text, asked.reply_to).await {
    Ok(Ok(said)) => answered(json!({
      "ok": true,
      "chatId": said.chat_id,
      "messageId": said.message_id,
      "lines": said.lines,
    })),
    Ok(Err(refusal)) => Err(code(&refusal)),
    Err(why) => Err(failed(why)),
  }
}

pub async fn block(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<BlockBody>(&body)?;
  let block = consult_block(&served.receiver, asked.agent.trim(), now_ms() / 1000)
    .await
    .map_err(store)?;

  answered(json!({ "ok": true, "block": block }))
}

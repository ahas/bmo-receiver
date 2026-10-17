//! A talk the owner has with the chat agent, handed over by bmo-job (docs/boundary-api.md, 「For bmo-job」,
//! `POST /talks`): the topic, the turns said so far and what he says now, taken as a round of the owner's
//! private chat and answered with what the agent said, or refused with why it came back with nothing.

use axum::{body::Bytes, extract::State, http::StatusCode};
use serde::Deserialize;
use serde_json::json;

use super::{Answered, Served, answered, code, read, ready, refused};
use crate::chat::talk::{DocumentTopic, ItemTopic, SaidTurn, TalkFailure, document_request, item_request};
use receiver_codes::TalkRefusal;

#[derive(Deserialize)]
struct TalkBody {
  topic: TopicBody,
  turns: Vec<Turn>,
  said: String,
}

/// What a talk is about, as bmo-job hands it.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum TopicBody {
  Item(ItemBody),
  Document(DocBody),
}

#[derive(Deserialize)]
struct ItemBody {
  id: i64,
  title: String,
  done: bool,
  merged_into: Option<i64>,
  held: bool,
  hold: Option<String>,
  stopping: bool,
  approved: bool,
  status: String,
}

#[derive(Deserialize)]
struct DocBody {
  document: String,
  label: String,
  writer: String,
  file: String,
  version: String,
  heading: Option<String>,
  passage: Option<String>,
  translated: bool,
}

#[derive(Deserialize)]
struct Turn {
  owner: bool,
  text: String,
}

/// Why a turn came back with nothing, as the receiver answers it.
fn talk_refusal(failure: TalkFailure) -> TalkRefusal {
  match failure {
    TalkFailure::Resting => TalkRefusal::Resting,
    TalkFailure::Limit => TalkRefusal::Limit,
    TalkFailure::Outage => TalkRefusal::Outage,
    TalkFailure::Other(_) => TalkRefusal::Unanswered,
  }
}

pub async fn talk(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<TalkBody>(&body)?;
  let said = asked.said.trim();

  if said.is_empty() {
    return Err(refused(StatusCode::BAD_REQUEST, "bad-request", "said (non-empty string) is required"));
  }

  let turns = asked
    .turns
    .iter()
    .map(|turn| SaidTurn {
      owner: turn.owner,
      text: turn.text.clone(),
    })
    .collect();
  let (request, key) = match asked.topic {
    TopicBody::Item(item) => (
      item_request(
        &ItemTopic {
          id: item.id,
          title: item.title.clone(),
          done: item.done,
          merged_into: item.merged_into,
          held: item.held,
          hold: item.hold.clone(),
          stopping: item.stopping,
          approved: item.approved,
          status: item.status.clone(),
        },
        turns,
        said.to_owned(),
      ),
      format!("item:{}", item.id),
    ),
    TopicBody::Document(document) => (
      document_request(
        &DocumentTopic {
          document: document.document.clone(),
          label: document.label.clone(),
          writer: document.writer.clone(),
          file: document.file.clone(),
          version: document.version.clone(),
          heading: document.heading.clone(),
          passage: document.passage.clone(),
          translated: document.translated,
        },
        turns,
        said.to_owned(),
      ),
      format!("document:{}", document.document),
    ),
  };
  let standing = served.receiver.talking();

  if !standing.begin(&key) {
    return Err(code(&TalkRefusal::Busy));
  }

  let Some(taker) = served.receiver.talks() else {
    let why = "the chat agent's conversations are not standing";

    standing.end(&key, Some(why.to_owned()));

    return Err(ready("the chat agent's conversations"));
  };
  let answer = taker.talk(&request).await;
  let why = match &answer {
    Err(failure) => Some(talk_refusal(failure.clone())),
    Ok(_) => None,
  };

  standing.end(&key, why.map(|refusal| refusal.to_string()));

  match answer {
    Ok(answer) => answered(json!({ "answer": answer })),
    Err(failure) => Err(code(&talk_refusal(failure))),
  }
}

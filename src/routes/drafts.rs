//! The draft process's table and questions as the console reads and sets them, and a round's draft log
//! (docs/drafting.md, 「The routes」 and 「The round log」).

use axum::{
  body::Bytes,
  extract::{Path, RawQuery, State},
  http::StatusCode,
};
use serde::Deserialize;
use serde_json::json;

use super::{Accepted, Answered, Refusal, Served, answered, query, read, refused, rounds_of, store};

use crate::chat::draft::log;
use crate::chat::draft::table::{self, PastCeiling, Process};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProcessSet {
  rounds: Vec<String>,
  #[serde(default)]
  send_from: String,
  light: Vec<String>,
  light_chars: u32,
  /// A row set before rounds were left without a ceiling may carry this; it is read and ignored.
  #[serde(default)]
  ceiling_seconds: Option<u32>,
  /// A turn's bound a row set before the bound was removed: read and ignored.
  #[serde(default)]
  #[allow(dead_code)]
  turn_seconds: Option<u32>,
  /// What a row set past that ceiling; read and ignored.
  #[serde(default)]
  past_ceiling: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuestionSet {
  text: String,
}

#[derive(Deserialize)]
pub struct Asked {
  room: i64,

  #[serde(default)]
  rounds: String,
}

fn bad(why: &str) -> Refusal {
  refused(StatusCode::BAD_REQUEST, "bad-request", why)
}

/// Every row of the table, the policy's and the console's.
pub async fn processes(State(served): State<Served>) -> Answered {
  let rows = table::rows(served.receiver.db()).await.map_err(store)?;

  answered(json!({ "items": rows.iter().map(Process::member).collect::<Vec<_>>(), "more": false }))
}

/// A row set by the console, answered as it now stands.
pub async fn set_process(
  State(served): State<Served>,
  Path((provider, tier, agent)): Path<(String, String, String)>,
  body: Bytes,
) -> Answered {
  let asked = read::<ProcessSet>(&body)?;
  let db = served.receiver.db();
  let past_ceiling = asked.past_ceiling.as_deref().and_then(PastCeiling::of).unwrap_or(PastCeiling::Send);
  let process = Process {
    provider,
    tier,
    agent,
    rounds: asked.rounds,
    send_from: asked.send_from,
    light: asked.light,
    light_chars: asked.light_chars,
    ceiling_seconds: asked.ceiling_seconds.unwrap_or(0),
    past_ceiling,
    set: true,
  };
  let catalogue = table::questions(db).await.map_err(store)?;

  if let Some(why) = table::refusal(&process, &catalogue) {
    return Err(bad(&why));
  }

  table::set(db, &process).await.map_err(store)?;

  answered(process.member())
}

/// The console's row of a key taken away, the policy's standing again where it holds one.
pub async fn remove_process(
  State(served): State<Served>,
  Path((provider, tier, agent)): Path<(String, String, String)>,
) -> Accepted {
  let gone = table::remove(served.receiver.db(), &provider, &tier, &agent).await.map_err(store)?;

  if !gone {
    return Err(refused(
      StatusCode::NOT_FOUND,
      "not-found",
      &format!("The console set no draft process row for {provider}/{tier}/{agent}."),
    ));
  }

  Ok((StatusCode::NO_CONTENT, axum::Json(json!({}))))
}

/// The catalogue of questions.
pub async fn questions(State(served): State<Served>) -> Answered {
  let questions = table::questions(served.receiver.db()).await.map_err(store)?;

  answered(
    json!({ "items": questions.iter().map(table::Question::member).collect::<Vec<_>>(), "more": false }),
  )
}

/// A question's text set by the console, answered as it now stands.
pub async fn set_question(State(served): State<Served>, Path(name): Path<String>, body: Bytes) -> Answered {
  let asked = read::<QuestionSet>(&body)?;

  if let Some(why) = table::question_refusal(&name, &asked.text) {
    return Err(bad(&why));
  }

  table::set_question(served.receiver.db(), &name, &asked.text).await.map_err(store)?;

  answered(
    table::Question {
      name,
      text: asked.text,
      set: true,
    }
    .member(),
  )
}

/// For a room and the rounds named, every row each round's draft process wrote.
pub async fn drafts(State(served): State<Served>, raw: RawQuery) -> Answered {
  let asked = query::<Asked>(raw)?;
  let rounds = rounds_of(asked.room, &asked.rounds);
  let items = log::read(served.receiver.db(), asked.room, &rounds).await.map_err(store)?;

  answered(json!({ "items": items, "more": false }))
}

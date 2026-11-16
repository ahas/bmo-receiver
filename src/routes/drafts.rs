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
  /// The questions of the list that are switched off; left out for none.
  #[serde(default)]
  off: Vec<String>,
  /// The stages after the message that are switched off; left out for none.
  #[serde(default)]
  stages_off: Vec<String>,
  /// A light list and bound an older console still sends: read and ignored, since every turn follows the rounds.
  #[serde(default)]
  #[allow(dead_code)]
  light: Option<Vec<String>>,
  #[serde(default)]
  #[allow(dead_code)]
  light_chars: Option<u32>,
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

/// The supervisor a read is asked for, where it names one.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Whose {
  #[serde(default)]
  agent_name: Option<String>,
}

#[derive(Deserialize)]
pub struct Asked {
  room: i64,

  #[serde(default)]
  rounds: String,

  /// How many of the room's newest rounds a read naming none answers.
  #[serde(default)]
  limit: Option<u64>,
}

// How many of a room's newest rounds a read naming none answers, unless it asks for another count, and the most
// it may ask for.
const NEWEST_ROUNDS: u64 = 20;
const NEWEST_ROUNDS_MOST: u64 = 200;

fn bad(why: &str) -> Refusal {
  refused(StatusCode::BAD_REQUEST, "bad-request", why)
}

/// Every row of the table, the policy's and each supervisor's own; for a supervisor named, the one row it reads.
pub async fn processes(State(served): State<Served>, raw: RawQuery) -> Answered {
  let whose = query::<Whose>(raw)?;
  let rows = table::rows(served.receiver.db()).await.map_err(store)?;
  let rows = match whose.agent_name.as_deref() {
    Some(agent) => table::resolve(&rows, agent).into_iter().collect(),
    None => rows,
  };

  answered(json!({ "items": rows.iter().map(Process::member).collect::<Vec<_>>(), "more": false }))
}

/// A supervisor's row set by the console, answered as it now stands.
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
    off: asked.off,
    stages_off: asked.stages_off,
    ceiling_seconds: asked.ceiling_seconds.unwrap_or(0),
    past_ceiling,
    set: true,
  };
  let catalogue = table::questions(db, Some(&process.agent)).await.map_err(store)?;

  if let Some(why) = table::refusal(&process, &catalogue) {
    return Err(bad(&why));
  }

  let process = process.settled();

  table::set(db, &process).await.map_err(store)?;

  answered(process.member())
}

/// A supervisor's row taken away, the supervisor reading the policy's again.
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

/// A supervisor's catalogue of questions, or the policy's alone where none is named.
pub async fn questions(State(served): State<Served>, raw: RawQuery) -> Answered {
  let whose = query::<Whose>(raw)?;
  let questions = table::questions(served.receiver.db(), whose.agent_name.as_deref())
    .await
    .map_err(store)?;

  answered(
    json!({ "items": questions.iter().map(table::Question::member).collect::<Vec<_>>(), "more": false }),
  )
}

/// A question's text set by the console for a supervisor, answered as it now stands.
pub async fn set_question(
  State(served): State<Served>,
  Path((agent, name)): Path<(String, String)>,
  body: Bytes,
) -> Answered {
  let asked = read::<QuestionSet>(&body)?;

  if agent == table::ANY || agent.trim().is_empty() {
    return Err(bad("A question's text is set for one supervisor, by its name."));
  }

  if let Some(why) = table::question_refusal(&name, &asked.text) {
    return Err(bad(&why));
  }

  table::set_question(served.receiver.db(), &agent, &name, &asked.text)
    .await
    .map_err(store)?;

  answered(
    table::Question {
      name,
      text: asked.text,
      set: true,
    }
    .member(),
  )
}

/// The console's text of a supervisor's question taken away: one the console added leaves the supervisor's
/// catalogue, and one the policy holds stands again in the policy's text. A question the console added that a
/// list of the supervisor's row still names is refused.
pub async fn remove_question(
  State(served): State<Served>,
  Path((agent, name)): Path<(String, String)>,
) -> Accepted {
  let db = served.receiver.db();

  if !table::policy_holds(&name) {
    let rows = table::rows(db).await.map_err(store)?;
    let named = rows.iter().any(|row| row.own() && row.agent == agent && row.rounds.contains(&name));

    if named {
      return Err(refused(
        StatusCode::CONFLICT,
        "conflict",
        &format!("A list of {agent}'s draft process row still names the question {name}."),
      ));
    }
  }

  let gone = table::remove_question(db, &agent, &name).await.map_err(store)?;

  if !gone {
    return Err(refused(
      StatusCode::NOT_FOUND,
      "not-found",
      &format!("The console set {agent} no text for the question {name}."),
    ));
  }

  Ok((StatusCode::NO_CONTENT, axum::Json(json!({}))))
}

/// For a room and the rounds named, or its newest rounds where a read names none, every row each round's draft
/// process wrote (docs/drafting.md, 「The round log」).
pub async fn drafts(State(served): State<Served>, raw: RawQuery) -> Answered {
  let asked = query::<Asked>(raw)?;
  let db = served.receiver.db();
  let (rounds, more) = if asked.rounds.trim().is_empty() {
    let limit = asked.limit.unwrap_or(NEWEST_ROUNDS).clamp(1, NEWEST_ROUNDS_MOST);

    log::newest_rounds(db, asked.room, limit).await.map_err(store)?
  } else {
    (rounds_of(asked.room, &asked.rounds), false)
  };
  let items = log::read(db, asked.room, &rounds).await.map_err(store)?;

  answered(json!({ "items": items, "more": more }))
}

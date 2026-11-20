//! The instruction stage chain and its texts as the console reads and sets them, and a round's pipeline log
//! (docs/instruction.md, 「The routes」 and 「The round log」).

use axum::{
  body::Bytes,
  extract::{Path, RawQuery, State},
  http::StatusCode,
};
use serde::Deserialize;
use serde_json::json;

use super::{Accepted, Answered, Refusal, Served, answered, query, read, refused, rounds_of, store};

use crate::chat::stages::log;
use crate::chat::stages::table::{self, Chain, Stage};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChainSet {
  /// The chain's stages, in the order they are asked once the message stands.
  stages: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextSet {
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

/// Every chain of the table, the policy's and each supervisor's own; for a supervisor named, the one chain it follows.
pub async fn chains(State(served): State<Served>, raw: RawQuery) -> Answered {
  let whose = query::<Whose>(raw)?;
  let all = table::chains(served.receiver.db()).await.map_err(store)?;
  let rows: Vec<&Chain> = match whose.agent_name.as_deref() {
    Some(agent) => table::followed(&all, agent).into_iter().collect(),
    None => all.iter().collect(),
  };

  answered(json!({ "items": rows.iter().map(|chain| chain.member()).collect::<Vec<_>>(), "more": false }))
}

/// A supervisor's own chain set by the console, answered as it now stands.
pub async fn set_chain(State(served): State<Served>, Path(agent): Path<String>, body: Bytes) -> Answered {
  let asked = read::<ChainSet>(&body)?;

  if agent == table::POLICY || agent.trim().is_empty() {
    return Err(bad("A chain is one supervisor's: its key is the supervisor's name."));
  }

  let db = served.receiver.db();
  let catalogue = table::texts(db, Some(&agent)).await.map_err(store)?;
  let names = catalogue.iter().map(|stage| stage.name.clone()).collect::<Vec<String>>();

  if let Some(why) = table::chain_refusal(&asked.stages, &names) {
    return Err(bad(why.as_str()));
  }

  table::set(db, &agent, &asked.stages).await.map_err(store)?;

  let chain = Chain {
    agent,
    stages: asked.stages,
    set: true,
  }
  .dedup();

  answered(chain.member())
}

/// A supervisor's own chain taken away, the supervisor reading the policy's again.
pub async fn remove_chain(State(served): State<Served>, Path(agent): Path<String>) -> Accepted {
  let gone = table::remove_chain(served.receiver.db(), &agent).await.map_err(store)?;

  if !gone {
    return Err(refused(
      StatusCode::NOT_FOUND,
      "not-found",
      &format!("The console set no instruction stage chain for {agent}."),
    ));
  }

  Ok((StatusCode::NO_CONTENT, axum::Json(json!({}))))
}

/// A supervisor's stage catalogue, or the policy's alone where none is named.
pub async fn texts(State(served): State<Served>, raw: RawQuery) -> Answered {
  let whose = query::<Whose>(raw)?;
  let stages = table::texts(served.receiver.db(), whose.agent_name.as_deref()).await.map_err(store)?;

  answered(json!({
    "items": stages.iter().map(Stage::member).collect::<Vec<_>>(),
    "more": false,
  }))
}

/// A stage's text set by the console for a supervisor, answered as it now stands.
pub async fn set_text(
  State(served): State<Served>,
  Path((agent, name)): Path<(String, String)>,
  body: Bytes,
) -> Answered {
  let asked = read::<TextSet>(&body)?;

  if agent == table::POLICY || agent.trim().is_empty() {
    return Err(bad("A stage's text is set for one supervisor, by its name."));
  }

  if let Some(why) = table::text_refusal(&name, &asked.text) {
    return Err(bad(why.as_str()));
  }

  table::set_text(served.receiver.db(), &agent, &name, &asked.text).await.map_err(store)?;

  answered(
    Stage {
      name,
      text: asked.text,
      set: true,
    }
    .member(),
  )
}

/// The console's text of a supervisor's stage taken away: one the console added leaves the supervisor's catalogue,
/// and one the policy holds stands again in the policy's text. A stage the console added that a chain of its
/// supervisor still names is refused.
pub async fn remove_text(
  State(served): State<Served>,
  Path((agent, name)): Path<(String, String)>,
) -> Accepted {
  let db = served.receiver.db();

  if !table::policy_holds(&name) {
    let chains = table::chains(db).await.map_err(store)?;
    let named = chains
      .iter()
      .any(|chain| chain.own() && chain.agent == agent && chain.stages.contains(&name));

    if named {
      return Err(refused(
        StatusCode::CONFLICT,
        "conflict",
        &format!("A chain of {agent} still names the stage {name}."),
      ));
    }
  }

  let gone = table::remove_text(db, &agent, &name).await.map_err(store)?;

  if !gone {
    return Err(refused(
      StatusCode::NOT_FOUND,
      "not-found",
      &format!("The console set {agent} no text for the stage {name}."),
    ));
  }

  Ok((StatusCode::NO_CONTENT, axum::Json(json!({}))))
}

/// For a room and the rounds named, or its newest rounds where a read names none, every row each round's pipeline
/// wrote (docs/instruction.md, 「The round log」).
pub async fn rounds(State(served): State<Served>, raw: RawQuery) -> Answered {
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

//! A room's figures for the console (`docs/boundary-api.md`, 「A room's figures」): what the receiver's record counts
//! of the room, and each of its agents with the session it keeps there. What an agent's row and session say is the
//! agency's, read through its routes; an agency that does not answer leaves those fields null and the figures stand.
//! The room's runs and what they spent are the agency's to tally and no route of its answers them by room, so they
//! stand null.

use axum::{
  extract::{Path, Query, State},
  http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};

use receiver_ports::agency::{Agent, Scope, Session, SessionState};

use super::{Answered, Served, answered, refused, store};
use crate::receiver::figures::{self, Filed};
use crate::receiver::{Receiver, reads};

// How many of the people who said most a room's figures name.
const TOP_SPEAKERS: i64 = 3;

// The zone the hours are counted in where none is asked for.
const UTC: &str = "UTC";

#[derive(Deserialize)]
pub struct Zoned {
  #[serde(default)]
  zone: Option<String>,
}

// The session an agent keeps for a room: its session of the room's own scope, or its session across rooms where
// the record says that has been given the room.
fn kept_for<'a>(sessions: &'a [Session], agent: &str, chat: i64, given: &[String]) -> Option<&'a Session> {
  let own = |session: &&Session| session.agent == agent;

  sessions
    .iter()
    .filter(own)
    .find(|session| session.scope == Scope::Room { room: chat })
    .or_else(|| {
      sessions
        .iter()
        .filter(own)
        .find(|session| session.scope == Scope::Agent && given.contains(&session.id))
    })
}

// The room's agents: those answering in it, then every agent keeping a session for it, each once.
fn room_agents(answerers: Vec<String>, sessions: &[Session], chat: i64, given: &[String]) -> Vec<String> {
  let keeping = sessions
    .iter()
    .filter(|session| kept_for(sessions, &session.agent, chat, given).is_some());
  let mut agents: Vec<String> = vec![];

  for agent in answerers.into_iter().chain(keeping.map(|session| session.agent.clone())) {
    if !agents.contains(&agent) {
      agents.push(agent);
    }
  }

  agents
}

// One field of an agent's row as the agency gives it, null where it gives none.
fn row_field(row: Option<&Agent>, field: &str) -> Value {
  row.and_then(|row| row.row.get(field)).cloned().unwrap_or(Value::Null)
}

/// What a room is made of, and each of its agents with the session it keeps there.
pub async fn figures(State(served): State<Served>, Path(chat): Path<i64>, query: Query<Zoned>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let db = receiver.db();
  let zone = query.zone.clone().unwrap_or_else(|| UTC.to_owned());

  if !figures::zone_known(db, &zone).await.map_err(store)? {
    return Err(refused(
      StatusCode::BAD_REQUEST,
      "bad-request",
      &format!("No time zone is named {zone}."),
    ));
  }

  let agency = &receiver.units().agency;
  let listed: Vec<Agent> = receiver.agents().await.map(|agents| agents.agents).unwrap_or_default();
  let sessions: Vec<Session> = agency.sessions().await.unwrap_or_default();
  let given = figures::sessions_given(db, chat).await.map_err(store)?;
  let answerers = figures::answerers(db, chat).await.map_err(store)?;
  let agents = room_agents(answerers, &sessions, chat, &given);
  let deployment = receiver.deployment();

  // Every agent's words, the listed and the room's own, are an agent's and not a person's.
  let every: Vec<String> =
    listed.iter().map(|agent| agent.name.clone()).chain(agents.iter().cloned()).collect();
  let anyone = Filed {
    name: Some(deployment.sender_name.clone()),
    numbers: reads::agent_numbers(db, &every).await.map_err(store)?,
  };
  let counted = figures::counted(db, chat, &anyone).await.map_err(store)?;
  let top = figures::top(db, chat, &anyone, TOP_SPEAKERS).await.map_err(store)?;
  let by_hour = figures::by_hour(db, chat, &zone).await.map_err(store)?;
  let mut rows: Vec<Value> = vec![];

  for agent in &agents {
    let own = Filed {
      name: (*agent == deployment.agent).then(|| deployment.sender_name.clone()),
      numbers: reads::agent_numbers(db, std::slice::from_ref(agent)).await.map_err(store)?,
    };
    let (messages, last_answer_at) = figures::filed(db, chat, &own).await.map_err(store)?;
    let session = kept_for(&sessions, agent, chat, &given);
    let state: Option<SessionState> = match session {
      Some(session) => agency.state(&session.id).await.ok(),
      None => None,
    };
    let row = listed.iter().find(|listed| listed.name == *agent);

    rows.push(json!({
      "agentName": agent,
      "messages": messages,
      "lastAnswerAt": last_answer_at,
      "session": session.map(|session| session.id.clone()),
      "label": row.and_then(|row| row.label.clone()),
      "lastMode": row_field(row, "lastMode"),
      "lastModel": row_field(row, "lastModel"),
      "lastEffort": row_field(row, "lastEffort"),
      "sessionMode": state.as_ref().and_then(|state| state.provider.clone()),
      "model": state.as_ref().and_then(|state| state.model.clone()),
      "contextTokens": state.as_ref().and_then(|state| state.context.get("carried").cloned()),
      "runs": Value::Null,
      "lastRunAt": Value::Null,
      "weekTokens": Value::Null,
      "totalTokens": Value::Null,
    }));
  }

  answered(json!({
    "room": chat,
    "messages": counted.messages,
    "fromPeople": counted.from_people,
    "speakers": counted.speakers,
    "firstAt": counted.first_at,
    "lastAt": counted.last_at,
    "lastAnswerAt": counted.last_answer_at,
    "top": top.iter().map(|(name, messages)| json!({ "name": name, "messages": messages })).collect::<Vec<_>>(),
    "byHour": by_hour,
    "agents": rows,
  }))
}

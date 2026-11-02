//! An agent added on the console and removed there (docs/api.md, 「Agents added on the console」): its room
//! of the console's own, its number there and the room's answerers, written in one go once every field has
//! passed; and an agent taken out again, its room half given up, never a seeded one. The agent's row, its
//! settings and its place are the agency's, and go through it; its gate options are the gate's, copied from
//! the chat agent's as it stands.

use axum::{
  body::Bytes,
  extract::{Path, RawQuery, State},
  http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};

use receiver_codes::{AddedAgentFact, AddedAgentRefusal, RoomRemovalWarning};
use receiver_policy::roads::ROADS;
use receiver_ports::agency::{Agent, ConsoleAgentRow, FieldRefusal};

use sea_orm::DatabaseConnection;

use crate::receiver::Receiver;
use crate::roads::record::{next_room, next_sender};
use crate::room_record::{Facing, RoomHolding, Says};
use crate::routes::{Answered, Refusal, Served, answered, code, read, refused, store, unit};
use crate::store::Fault;
use crate::{all, one, run};

#[derive(Deserialize)]
pub struct Added {
  name: String,
  label: String,
  provider: String,
  model: String,
  worker: String,
  room: String,
}

#[derive(Deserialize, Default)]
pub struct Removed {
  #[serde(default)]
  name: String,
  #[serde(default)]
  by: Option<String>,
}

/// A field the agency refused, as the refusal of that field: the console renders the code, never the agency's
/// sentence. A field the receiver has no refusal for is the agency's failure to answer the call.
fn refused_field(refusal: FieldRefusal, provider: &str, worker: &str, agent: &str) -> Refusal {
  match refusal.field.as_str() {
    "name" => code(&AddedAgentRefusal::Name),
    "label" => code(&AddedAgentRefusal::Label),
    "provider" => code(&AddedAgentRefusal::Provider {
      provider: provider.to_owned(),
    }),
    "model" => code(&AddedAgentRefusal::Model),
    "worker" => code(&AddedAgentRefusal::Worker {
      worker: worker.to_owned(),
    }),
    "agent" => code(&AddedAgentRefusal::NotAdded {
      agent: agent.to_owned(),
    }),
    field => refused(
      StatusCode::BAD_GATEWAY,
      "upstream-failed",
      &format!("The agency refused the field {field}: {}", refusal.error),
    ),
  }
}

// The road a console room is numbered under: the receiver's own record, which no chat service carries.
fn console_road() -> String {
  ROADS.record.key.clone()
}

/// Whether an agent's row is the shape of turn the agency gives an agent added on the console.
fn added_shape(agent: &Agent) -> bool {
  agent.row.get("turnShape").is_some_and(|shape| shape.as_str() == Some("pieces"))
}

/// The added agents as the console's screen shows them: the agents the agency registers with the added agent's
/// shape of turn, each with the room the receiver stood up for it where it did.
async fn listed(receiver: &Receiver) -> Result<Vec<Value>, Refusal> {
  let agents = receiver.agents().await.map_err(unit)?;
  let mut shown: Vec<Value> = vec![];

  for agent in agents.agents.iter().filter(|one| added_shape(one)) {
    shown.push(json!({
      "name": agent.name,
      "label": agent.label,
      "id": room_of(receiver.db(), &agent.name).await.ok().flatten(),
    }));
  }

  Ok(shown)
}

/// The console room already numbered for an added agent's name, where one stands.
async fn room_of(db: &DatabaseConnection, name: &str) -> Result<Option<i64>, Fault> {
  Ok(
    one(
      db,
      r#"SELECT "chat_id" FROM "provider_rooms" WHERE "provider" = $1 AND "room_id" = $2"#,
      vec![console_road().into(), name.into()],
    )
    .await?
    .map(|row| row.try_get::<i64>("", "chat_id"))
    .transpose()?,
  )
}

/// An agent added with a room of the console's own, answered with every added agent as it then stands and
/// the room it answers.
pub async fn add(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<Added>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();

  // What a name may be is the agency's to say: a name it does not take is refused as the field `name`, before
  // anything of the room is written.
  if asked.name.trim().is_empty() {
    return Err(code(&AddedAgentRefusal::Name));
  }

  if asked.label.trim().is_empty() {
    return Err(code(&AddedAgentRefusal::Label));
  }

  if asked.room.trim().is_empty() {
    return Err(code(&AddedAgentRefusal::Room));
  }

  let agents = receiver.agents().await.map_err(unit)?;

  if agents.agents.iter().any(|one| one.name == asked.name) {
    return Err(code(&AddedAgentRefusal::NameTaken { name: asked.name }));
  }

  // The field `worker` names the repository the agent's items go to, one the agency holds.
  let repositories = receiver.units().agency.repositories().await.map_err(unit)?;

  if !repositories.iter().any(|one| one.name == asked.worker) {
    return Err(code(&AddedAgentRefusal::Worker {
      worker: asked.worker,
    }));
  }

  let road = console_road();
  let standing = room_of(receiver.db(), &asked.name).await.ok().flatten();
  let (chat_id, taken_back) = match standing {
    Some(room) => (room, true),
    None => (next_room(receiver.db()).await.map_err(store)?, false),
  };
  let row = ConsoleAgentRow {
    name: asked.name.clone(),
    label: asked.label.clone(),
    provider: asked.provider.clone(),
    model: asked.model.clone(),
    worker: asked.worker.clone(),
  };

  // The agency stands the agent's row and the gate its options; the receiver writes the room half.
  let added = receiver.units().agency.add_console_agent(&row).await.map_err(unit)?;

  receiver.forget_agents();

  if let Err(refusal) = added {
    return Err(refused_field(refusal, &asked.provider, &asked.worker, &asked.name));
  }

  let chat_agent = receiver.deployment().agent.as_str();

  if let Err(why) = receiver.units().gate.copy_options(&asked.name, chat_agent).await {
    eprintln!(
      "added agents: {} stands on the gate's own options, since they were not copied: {why}",
      asked.name
    );
  }

  let sender = next_sender(receiver.db(), &road).await.map_err(store)?;

  run(
    receiver.db(),
    r#"INSERT INTO "provider_rooms" ("provider", "room_id", "chat_id", "title") VALUES ($1, $2, $3, $4)"#,
    vec![
      road.clone().into(),
      asked.name.clone().into(),
      chat_id.into(),
      asked.room.clone().into(),
    ],
  )
  .await
  .map_err(store)?;
  // A console room is born on the record alone: its one provider is the record, so its answers stand there.
  crate::roads::providers::choose(receiver.db(), chat_id, std::slice::from_ref(&road))
    .await
    .map_err(store)?;
  run(
    receiver.db(),
    r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES ($1, $2) ON CONFLICT ("chat_id") DO UPDATE SET "title" = excluded."title""#,
    vec![chat_id.into(), asked.room.clone().into()],
  )
    .await.map_err(store)?;
  run(
    receiver.db(),
    r#"DELETE FROM "room_answerers" WHERE "chat_id" = $1"#,
    vec![chat_id.into()],
  )
  .await
  .map_err(store)?;
  run(
    receiver.db(),
    r#"INSERT INTO "room_answerers" ("chat_id", "agent") VALUES ($1, $2)"#,
    vec![chat_id.into(), asked.name.clone().into()],
  )
  .await
  .map_err(store)?;
  run(
    receiver.db(),
    r#"INSERT INTO "provider_users" ("provider", "user_id", "sender_id") VALUES ($1, $2, $3)"#,
    vec![road.clone().into(), asked.name.clone().into(), sender.into()],
  )
  .await
  .map_err(store)?;

  let said = taken_back.then(|| {
    json!(AddedAgentFact::RoomTakenBack {
      label: asked.label,
      title: asked.room
    })
  });

  answered(json!({
    "items": listed(&receiver).await.unwrap_or_default(),
    "more": false,
    "room": { "id": chat_id, "takenBack": taken_back, "said": said },
  }))
}

/// An agent the console added taken out, answered with every added agent as it then stands. A seeded one is
/// refused before the agency is asked anything, as the agency's own supervisor read says it is.
pub async fn remove(State(served): State<Served>, Path(name): Path<String>, raw: RawQuery) -> Answered {
  let mut asked = super::query::<Removed>(raw)?;
  asked.name = name;
  let receiver: Receiver = (*served.receiver).clone();

  if asked.name.trim().is_empty() {
    return Err(code(&AddedAgentRefusal::Action));
  }

  let Some(agent) = receiver.units().agency.supervisor(&asked.name).await.map_err(unit)? else {
    return Err(code(&AddedAgentRefusal::NotAdded { agent: asked.name }));
  };

  if agent.seeded() {
    return Err(code(&AddedAgentRefusal::Seeded {
      label: agent.label.clone().unwrap_or_else(|| agent.name.clone()),
    }));
  }

  if !added_shape(&agent) {
    return Err(code(&AddedAgentRefusal::NotAdded { agent: asked.name }));
  }

  let removed = receiver.units().agency.remove_console_agent(&asked.name).await.map_err(unit)?;

  receiver.forget_agents();

  if let Err(refusal) = removed {
    return Err(refused_field(refusal, "", "", &asked.name));
  }

  if let Some(by) = asked.by.as_deref().filter(|by| !by.trim().is_empty()) {
    println!("added agent {}: removed, as {by} asked", asked.name);
  }

  // A number a kept message is still filed under stays, so it is never handed to anybody else.
  let numbers: Vec<i64> = all(
    receiver.db(),
    r#"SELECT u.sender_id FROM provider_users u WHERE u.user_id = $1
       AND NOT EXISTS (SELECT 1 FROM messages m WHERE m.sender_id = u.sender_id)"#,
    vec![asked.name.clone().into()],
  )
  .await
  .map_err(store)?
  .iter()
  .map(|row| row.try_get::<i64>("", "sender_id"))
  .collect::<Result<Vec<i64>, _>>()
  .unwrap_or_default();

  for number in &numbers {
    let worn = one(
      receiver.db(),
      r#"DELETE FROM avatars WHERE kind = 'person' AND subject_id = $1 RETURNING path"#,
      vec![(*number).into()],
    )
    .await
    .map_err(store)?;

    if let Some(path) = worn.map(|row| row.try_get::<String>("", "path")).transpose().unwrap_or(None) {
      let _ = std::fs::remove_file(&path);
    }

    run(
      receiver.db(),
      r#"DELETE FROM "provider_users" WHERE "sender_id" = $1"#,
      vec![(*number).into()],
    )
    .await
    .map_err(store)?;
  }

  run(
    receiver.db(),
    r#"DELETE FROM "room_answerers" WHERE "agent" = $1"#,
    vec![asked.name.into()],
  )
  .await
  .map_err(store)?;

  answered(json!({
    "items": listed(&receiver).await.unwrap_or_default(),
    "more": false,
    "said": Value::Null,
  }))
}

// The other agents answering in the room: they stay with the removal, and no longer answer in a room that is gone.
async fn answering(receiver: &Receiver, room: i64) -> Vec<String> {
  let named: Vec<String> = all(
    receiver.db(),
    r#"SELECT "agent" FROM "room_answerers" WHERE "chat_id" = $1"#,
    vec![room.into()],
  )
  .await
  .unwrap_or_default()
  .iter()
  .map(|row| row.try_get::<String>("", "agent").unwrap_or_default())
  .collect();
  let agents: Vec<Agent> = receiver.agents().await.map(|listed| listed.agents).unwrap_or_default();

  named
    .into_iter()
    .filter(|name| agents.iter().any(|agent| agent.name.as_str() == name.as_str() && added_shape(agent)))
    .collect()
}

fn answering_warning<'a>(facing: &'a Facing<'a>) -> Says<'a, RoomRemovalWarning> {
  Box::pin(async move {
    let names = answering(facing.receiver, facing.room).await;

    Ok(match names.is_empty() {
      true => None,
      false => Some(RoomRemovalWarning::Answerers { names }),
    })
  })
}

crate::room_holding!(RoomHolding::Warns {
  what: "the other agents answering the room",
  warns: answering_warning,
});

//! The supervisor routes (docs/api.md, 「Supervisors」): a supervisor added with a room of the console's own, its
//! number there and the room's list naming it, written in one go once every field has passed; a supervisor taken
//! out again, its rooms' naming of it given up, never a seeded one nor the default answerer; a supervisor started
//! afresh with the rooms that name it; the rooms a supervisor answers; the default answerer setting; and a room's
//! whole list of supervisors. A supervisor's row, its settings and its place are the agency's, and go through it;
//! its gate options are the gate's, copied from the default answerer's as they stand.

use axum::{
  body::Bytes,
  extract::{Path, RawQuery, State},
  http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};

use receiver_codes::{RoomReadRefusal, RoomRemovalWarning, SupervisorFact, SupervisorRefusal};
use receiver_policy::roads::ROADS;
use receiver_ports::agency::{Agent, FieldRefusal, NewSupervisor, Removal};
use receiver_ports::http::PortFailure;

use sea_orm::DatabaseConnection;

use crate::receiver::wake::agents;
use crate::receiver::{NotReceived, Receiver, figures, reads};
use crate::roads::record::{next_room, next_sender};
use crate::room_record::{Facing, RoomHolding, Says};
use crate::routes::{
  Accepted, Answered, Refusal, Served, answered, code, query, read, refused, store, unit,
};
use crate::store::Fault;
use crate::{all, one, run};

use super::console;

/// A read of a group room's messages: the cursor the reader holds and how many it asks for.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MemberRead {
  #[serde(default)]
  before: Option<i64>,

  #[serde(default)]
  after: Option<i64>,

  #[serde(default)]
  limit: Option<i64>,
}

fn reached(why: NotReceived) -> Refusal {
  refused(StatusCode::BAD_GATEWAY, "upstream-failed", &why.to_string())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Added {
  name: String,
  label: String,
  #[serde(default)]
  provider: Option<String>,
  #[serde(default)]
  model: Option<String>,
  #[serde(default)]
  repository: Option<String>,
  room: String,
  #[serde(default)]
  order: Option<Value>,
  #[serde(default)]
  quota_share: Option<Value>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Removed {
  #[serde(default)]
  by: Option<String>,
  #[serde(default)]
  forget_brain: bool,
  #[serde(default)]
  force: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetAsked {
  #[serde(default)]
  conversation: Option<bool>,
  #[serde(default)]
  person: Option<bool>,
  #[serde(default)]
  dry_run: Option<bool>,
  #[serde(default)]
  by: Option<String>,
}

#[derive(Deserialize)]
pub struct Named {
  supervisor: String,
}

#[derive(Deserialize)]
pub struct Listed {
  supervisors: Vec<String>,
}

/// A field the agency refused, as the refusal of that field: the console renders the code, never the agency's
/// sentence. A field the receiver has no refusal for is the agency's failure to answer the call.
fn refused_field(refusal: FieldRefusal, provider: &str, repository: &str) -> Refusal {
  match refusal.field.as_str() {
    "name" => code(&SupervisorRefusal::Name),
    "label" => code(&SupervisorRefusal::Label),
    "provider" => code(&SupervisorRefusal::Provider {
      provider: provider.to_owned(),
    }),
    "model" => code(&SupervisorRefusal::Model),
    "repository" => code(&SupervisorRefusal::Repository {
      repository: repository.to_owned(),
    }),
    "quotaShare" => code(&SupervisorRefusal::QuotaShare),
    member if member == "order" || member.starts_with("order[") => code(&SupervisorRefusal::Order {
      member: member.to_owned(),
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

// What a supervisor is called on a screen: its label, else its name.
fn label(agent: &Agent) -> String {
  agent.label.clone().unwrap_or_else(|| agent.name.clone())
}

/// The console room already numbered for a supervisor's name, where one stands.
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

/// Every room that names the supervisor, each with how many supervisors it names, in room order.
async fn naming(db: &DatabaseConnection, name: &str) -> Result<Vec<(i64, i64)>, Fault> {
  let rows = all(
    db,
    r#"SELECT s."chat_id", (SELECT count(*) FROM "room_supervisors" o WHERE o."chat_id" = s."chat_id") AS "named"
       FROM "room_supervisors" s WHERE s."supervisor" = $1 ORDER BY s."chat_id""#,
    vec![name.into()],
  )
  .await?;

  rows
    .iter()
    .map(|row| Ok((row.try_get::<i64>("", "chat_id")?, row.try_get::<i64>("", "named")?)))
    .collect()
}

/// The supervisor the agency registers under the name, or the refusal of a name it does not.
pub(super) async fn registered(receiver: &Receiver, name: &str) -> Result<Agent, Refusal> {
  receiver.units().agency.supervisor(name).await.map_err(unit)?.ok_or_else(|| {
    code(&SupervisorRefusal::UnknownSupervisor {
      supervisor: name.to_owned(),
    })
  })
}

/// `POST /supervisors`: a supervisor added with a room of the console's own.
pub async fn add(State(served): State<Served>, body: Bytes) -> Accepted {
  let asked = read::<Added>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();

  // What a name may be is the agency's to say: a name it does not take is refused as the field `name`, before
  // anything of the room is written.
  if asked.name.trim().is_empty() {
    return Err(code(&SupervisorRefusal::Name));
  }

  if asked.label.trim().is_empty() {
    return Err(code(&SupervisorRefusal::Label));
  }

  if asked.room.trim().is_empty() {
    return Err(code(&SupervisorRefusal::Room));
  }

  let agents = receiver.agents().await.map_err(unit)?;

  if agents.agents.iter().any(|one| one.name == asked.name) {
    return Err(code(&SupervisorRefusal::NameTaken { name: asked.name }));
  }

  let repository = asked.repository.clone().filter(|named| !named.trim().is_empty());

  if let Some(named) = &repository {
    let repositories = receiver.units().agency.repositories().await.map_err(unit)?;

    if !repositories.iter().any(|one| &one.name == named) {
      return Err(code(&SupervisorRefusal::Repository {
        repository: named.clone(),
      }));
    }
  }

  let road = console_road();
  let standing = room_of(receiver.db(), &asked.name).await.ok().flatten();
  let (chat_id, taken_back) = match standing {
    Some(room) => (room, true),
    None => (next_room(receiver.db()).await.map_err(store)?, false),
  };
  let row = NewSupervisor {
    name: asked.name.clone(),
    label: asked.label.clone(),
    provider: asked.provider.clone(),
    model: asked.model.clone(),
    repository: repository.clone(),
    order: asked.order.clone(),
    quota_share: asked.quota_share.clone(),
  };

  // The agency stands the supervisor's row and the gate its options; the receiver writes the room half.
  let added = match receiver.units().agency.add_supervisor(&row).await {
    Ok(added) => added,
    Err(failure) if failure.code() == Some("name-taken") => {
      receiver.forget_agents();
      return Err(code(&SupervisorRefusal::NameTaken { name: asked.name }));
    }
    Err(failure) => return Err(unit(failure)),
  };

  receiver.forget_agents();

  let added = added.map_err(|refusal| {
    refused_field(
      refusal,
      asked.provider.as_deref().unwrap_or_default(),
      repository.as_deref().unwrap_or_default(),
    )
  })?;
  let default = receiver.default_answerer();

  if let Err(why) = receiver.units().gate.copy_options(&asked.name, &default).await {
    eprintln!(
      "supervisors: {} stands on the gate's own options, since they were not copied: {why}",
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
  .await
  .map_err(store)?;
  name_alone(receiver.db(), chat_id, std::slice::from_ref(&asked.name))
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
    json!(SupervisorFact::RoomTakenBack {
      label: asked.label,
      title: asked.room
    })
  });

  Ok((
    StatusCode::CREATED,
    axum::Json(json!({
      "supervisor": added.get("supervisor").cloned().unwrap_or(Value::Null),
      "brain": added.get("brain").cloned().unwrap_or(Value::Null),
      "room": { "id": chat_id, "takenBack": taken_back, "said": said },
    })),
  ))
}

/// A room's list of supervisors written whole, in place of the one it held.
async fn name_alone(db: &DatabaseConnection, chat_id: i64, names: &[String]) -> Result<(), Fault> {
  run(db, r#"DELETE FROM "room_supervisors" WHERE "chat_id" = $1"#, vec![chat_id.into()]).await?;

  for name in names {
    run(
      db,
      r#"INSERT INTO "room_supervisors" ("chat_id", "supervisor") VALUES ($1, $2) ON CONFLICT DO NOTHING"#,
      vec![chat_id.into(), name.clone().into()],
    )
    .await?;
  }

  Ok(())
}

/// `DELETE /supervisors/{supervisorName}`: a supervisor taken out. One the deployment seeds, and the one the
/// default answerer setting names, are refused before the agency is asked anything.
pub async fn remove(State(served): State<Served>, Path(name): Path<String>, raw: RawQuery) -> Accepted {
  let asked = super::query::<Removed>(raw)?;
  let receiver: Receiver = (*served.receiver).clone();

  if name.trim().is_empty() {
    return Err(code(&SupervisorRefusal::Action));
  }

  let agent = registered(&receiver, &name).await?;

  if agent.seeded() {
    return Err(code(&SupervisorRefusal::Seeded {
      label: label(&agent),
    }));
  }

  if receiver.default_answerer() == name {
    return Err(code(&SupervisorRefusal::DefaultAnswerer {
      label: label(&agent),
    }));
  }

  let removal = Removal {
    rooms: naming(receiver.db(), &name)
      .await
      .map_err(store)?
      .into_iter()
      .map(|(room, _)| room)
      .collect(),
    forget_brain: asked.forget_brain,
    force: asked.force,
  };
  let removed = receiver.units().agency.remove_supervisor(&name, &removal).await;

  receiver.forget_agents();

  match removed {
    Ok(()) => {}
    Err(failure) if failure.code() == Some("seeded") => {
      return Err(code(&SupervisorRefusal::Seeded {
        label: label(&agent),
      }));
    }
    Err(failure) if failure.code() == Some("running") => {
      return Err(code(&SupervisorRefusal::Running {
        label: label(&agent),
      }));
    }
    Err(failure) if failure.status() == Some(404) => {
      return Err(code(&SupervisorRefusal::UnknownSupervisor { supervisor: name }));
    }
    Err(failure) => return Err(unit(failure)),
  }

  if let Some(by) = asked.by.as_deref().filter(|by| !by.trim().is_empty()) {
    println!("supervisor {name}: removed, as {by} asked");
  }

  // A number a kept message is still filed under stays, so it is never handed to anybody else.
  let numbers: Vec<i64> = all(
    receiver.db(),
    r#"SELECT u.sender_id FROM provider_users u WHERE u.user_id = $1
       AND NOT EXISTS (SELECT 1 FROM messages m WHERE m.sender_id = u.sender_id)"#,
    vec![name.clone().into()],
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
    r#"DELETE FROM "room_supervisors" WHERE "supervisor" = $1"#,
    vec![name.into()],
  )
  .await
  .map_err(store)?;

  Ok((StatusCode::NO_CONTENT, axum::Json(json!({}))))
}

/// `POST /supervisors/{supervisorName}/resets`: the agency's reset, with the rooms that name the supervisor alone
/// and those that name it beside another.
pub async fn reset(State(served): State<Served>, Path(name): Path<String>, body: Bytes) -> Answered {
  let asked = read::<ResetAsked>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();

  registered(&receiver, &name).await?;

  let rooms = naming(receiver.db(), &name).await.map_err(store)?;
  let alone: Vec<i64> = rooms.iter().filter(|(_, named)| *named == 1).map(|(room, _)| *room).collect();
  let shared: Vec<i64> = rooms.iter().filter(|(_, named)| *named > 1).map(|(room, _)| *room).collect();
  let mut handed = json!({ "alone": alone, "shared": shared });

  for (field, value) in [
    ("conversation", asked.conversation.map(Value::Bool)),
    ("person", asked.person.map(Value::Bool)),
    ("dryRun", asked.dry_run.map(Value::Bool)),
    ("by", asked.by.map(Value::String)),
  ] {
    if let Some(value) = value {
      handed[field] = value;
    }
  }

  match receiver.units().agency.reset_supervisor(&name, &handed).await {
    Ok(answer) => answered(answer),
    Err(PortFailure::Answered {
      status: 400,
      answer: Some(answer),
      ..
    }) => Err(refused(StatusCode::BAD_REQUEST, "bad-request", &answer.error)),
    Err(failure) => Err(unit(failure)),
  }
}

/// `GET /supervisors/{supervisorName}/rooms`: the rooms the supervisor answers, those that name it and, for the
/// default answerer, every room that names none.
pub async fn rooms(State(served): State<Served>, Path(name): Path<String>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let db = receiver.db();
  let default = receiver.default_answerer() == name;
  let mut items: Vec<Value> = vec![];

  for room in reads::shown(db).await.map_err(store)? {
    let named = figures::answerers(db, room.chat_id).await.map_err(store)?;
    let answers = named.contains(&name) || (default && named.is_empty());

    if answers {
      items.push(json!({
        "id": room.chat_id,
        "title": room.title,
        "named": !named.is_empty(),
        "alone": named.len() <= 1,
      }));
    }
  }

  answered(json!({ "items": items, "more": false }))
}

/// `GET /supervisors/{supervisorName}/rooms/{roomId}/messages`: a group room's messages as a supervisor reads
/// them. The supervisor that answers the room reads it, and so does a member's own supervisor where one of the
/// member's senders has stood in the room. The read answers messages alone: it opens no window, wakes nobody and
/// makes no answerer of the supervisor.
pub async fn member_messages(
  State(served): State<Served>,
  Path((name, room)): Path<(String, i64)>,
  raw: RawQuery,
) -> Answered {
  let asked = query::<MemberRead>(raw)?;
  let receiver: Receiver = (*served.receiver).clone();

  membership(&receiver, &name, room).await?;

  let bound = console::window(asked.limit);
  let found = match (asked.before, asked.after) {
    (_, Some(after)) => reads::after(receiver.db(), room, after, bound).await,
    (Some(before), None) => reads::before(receiver.db(), room, before, bound).await,
    (None, None) => reads::latest(receiver.db(), room, bound).await,
  }
  .map_err(store)?;
  let more = i64::try_from(found.len()).unwrap_or(0) == bound;
  let kept = if !more {
    found
  } else if asked.after.is_some() && asked.before.is_none() {
    found.into_iter().take(usize::try_from(bound - 1).unwrap_or(0)).collect()
  } else {
    found.into_iter().skip(1).collect()
  };
  let items = console::pages(&receiver, room, kept).await.map_err(store)?;

  answered(json!({ "items": items, "more": more }))
}

/// Whether a supervisor may read a group room: the room is a group, and the supervisor answers it or belongs to
/// an account one of whose senders has stood in the room.
async fn membership(receiver: &Receiver, agent: &str, room: i64) -> Result<(), Refusal> {
  let facts = receiver.roads().room_facts(room).await;

  if facts.owner_room || facts.private_to.is_some() {
    return Err(code(&RoomReadRefusal::NotAGroup { room }));
  }

  let (answerers, _) = agents::room_designated(receiver, room, facts).await.map_err(reached)?;

  if answerers.iter().any(|one| one.name == agent) {
    return Ok(());
  }

  let Some(registered) = agents::registered(receiver, agent).await.map_err(reached)? else {
    return Err(code(&SupervisorRefusal::UnknownSupervisor {
      supervisor: agent.to_owned(),
    }));
  };
  let senders = match registered.owner_account {
    Some(account) => receiver.units().web.senders_of_user(&account.to_string()).await.unwrap_or_default(),
    None => Vec::new(),
  };

  if reads::spoken_in(receiver.db(), room, &senders).await.map_err(store)?.is_empty() {
    return Err(code(&RoomReadRefusal::NotAMember {
      agent: agent.to_owned(),
      room,
    }));
  }

  Ok(())
}

/// `GET /default-answerer`: the supervisor the setting names.
pub async fn default_answerer(State(served): State<Served>) -> Answered {
  answered(json!({ "supervisor": served.receiver.default_answerer() }))
}

/// `PUT /default-answerer`: the setting moved to a supervisor the agency registers.
pub async fn set_default_answerer(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<Named>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();
  let name = asked.supervisor.trim().to_owned();

  registered(&receiver, &name).await?;
  receiver.set_default_answerer(&name).await.map_err(store)?;
  println!("default answerer: {name}");

  answered(json!({ "supervisor": receiver.default_answerer() }))
}

/// `PUT /rooms/{roomId}/supervisors`: a room's whole list of supervisors, each one the agency registers.
pub async fn name_room(State(served): State<Served>, Path(room): Path<i64>, body: Bytes) -> Answered {
  let asked = read::<Listed>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();
  let agents = receiver.agents().await.map_err(unit)?;
  let mut names: Vec<String> = asked.supervisors.iter().map(|name| name.trim().to_owned()).collect();

  names.sort();
  names.dedup();

  if let Some(unknown) = names.iter().find(|name| !agents.agents.iter().any(|agent| &agent.name == *name)) {
    return Err(code(&SupervisorRefusal::UnknownSupervisor {
      supervisor: unknown.clone(),
    }));
  }

  name_alone(receiver.db(), room, &names).await.map_err(store)?;

  answered(json!({ "supervisors": figures::answerers(receiver.db(), room).await.map_err(store)? }))
}

// The supervisors the room names: they stay with the removal, and no longer answer in a room that is gone.
async fn answering(receiver: &Receiver, room: i64) -> Vec<String> {
  figures::answerers(receiver.db(), room).await.unwrap_or_default()
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
  what: "the supervisors the room names",
  warns: answering_warning,
});

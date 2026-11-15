//! The supervisor routes (docs/api.md, 「Supervisors」): the agency writes a supervisor's half and the gate copies
//! the default answerer's options before the receiver writes the room half; a removal hands the agency the rooms
//! that name the supervisor, and a seeded supervisor, the default answerer and one a turn holds are refused; a
//! field the agency refuses comes back as the receiver's code with nothing written; a reset is handed the rooms
//! that name the supervisor alone and beside another; and a room's list and the default answerer setting are
//! read and written.

use std::sync::Arc;

use axum::http::StatusCode;
use serde_json::{Value, json};

use receiver_ports::agency::{Agent, FieldRefusal, Repository};

use crate::integration::common::Scratch;
use crate::integration::common::queue::{Front, get, post, put, remove, stood, stood_on};
use crate::integration::common::units::{AGENT, OWNER, StandInChat};

use bmo_receiver::receiver::wake::agents;

const ADDED: &str = "agent-d";
const REPOSITORY: &str = "repo-a";

// The front, with the agency holding one repository a supervisor's items may go to.
async fn bench(scratch: &Scratch) -> Front {
  let front = stood(scratch, OWNER).await;

  *front.standing.agency.repositories.lock().unwrap() = vec![Repository {
    name: REPOSITORY.to_owned(),
    label: Some("Repo A".to_owned()),
  }];

  front
}

fn added() -> Value {
  json!({
    "name": ADDED,
    "label": "Agent D",
    "provider": "provider-a",
    "model": "model-a",
    "repository": REPOSITORY,
    "room": "A room of its own",
  })
}

fn agent(name: &str, label: &str) -> Agent {
  Agent {
    name: name.to_owned(),
    label: Some(label.to_owned()),
    awake: true,
    brain_folder: None,
    row: serde_json::Map::new(),
  }
}

async fn naming(scratch: &Scratch) -> i64 {
  scratch
    .count(r#"SELECT COUNT(*) FROM "room_supervisors" WHERE "supervisor" = $1"#, vec![ADDED.into()])
    .await
}

#[tokio::test]
async fn a_members_supervisor_reads_a_group_rooms_messages_without_answering_it() {
  let scratch = Scratch::new().await;
  let room = -151;
  let member = 500;
  let chat = Arc::new(StandInChat::new("road-a").holding(&[room]));
  let front = stood_on(&scratch, OWNER, vec![chat]).await;

  let mut reader = agent("agent-e", "Agent E");
  reader.row.insert("ownerAccount".to_owned(), json!(88));
  front.standing.agency.registering(vec![agent(AGENT, "Agent A"), reader]);
  front.standing.web.linking(member, "88".to_owned());
  scratch.message(room, 1, member).await;
  scratch.message(room, 2, OWNER).await;

  let (status, answer) = get(&front, &format!("/supervisors/agent-e/rooms/{room}/messages")).await;

  assert_eq!(status, StatusCode::OK, "{answer}");
  assert_eq!(answer["items"].as_array().unwrap().len(), 2);

  // Reading adds no answerer: the room is still the default answerer's.
  let (answerers, _) = agents::room_designated(
    &front.receiver(),
    room,
    bmo_receiver::receiver::Room {
      owner_room: false,
      identity: None,
      private_to: None,
      sealed: false,
    },
  )
  .await
  .unwrap();

  assert_eq!(answerers.len(), 1);
  assert_eq!(answerers[0].name, AGENT);
}

#[tokio::test]
async fn a_supervisor_whose_person_never_stood_in_the_room_reads_nothing_of_it() {
  let scratch = Scratch::new().await;
  let room = -152;
  let chat = Arc::new(StandInChat::new("road-a").holding(&[room]));
  let front = stood_on(&scratch, OWNER, vec![chat]).await;

  let mut stranger = agent("agent-f", "Agent F");
  stranger.row.insert("ownerAccount".to_owned(), json!(99));
  front.standing.agency.registering(vec![agent(AGENT, "Agent A"), stranger]);
  front.standing.web.linking(501, "99".to_owned());
  scratch.message(room, 1, OWNER).await;

  let (status, answer) = get(&front, &format!("/supervisors/agent-f/rooms/{room}/messages")).await;

  assert_eq!(status, StatusCode::FORBIDDEN, "{answer}");
  assert_eq!(answer["code"], "not-a-member");
}

#[tokio::test]
async fn a_supervisor_stands_with_its_gate_options_copied_and_its_rooms_go_to_the_agencys_removal() {
  let scratch = Scratch::new().await;
  let front = bench(&scratch).await;
  let (status, answer) = post(&front, "/supervisors", added()).await;

  assert_eq!(status, StatusCode::CREATED, "{answer}");
  assert_eq!(answer["supervisor"]["name"], ADDED);
  assert_eq!(answer["brain"], "made");

  let room = answer["room"]["id"].as_i64().unwrap();

  assert_eq!(
    *front.standing.gate.copied.lock().unwrap(),
    [(ADDED.to_owned(), AGENT.to_owned())],
    "the gate copies the default answerer's options for the new supervisor"
  );
  assert_eq!(naming(&scratch).await, 1);

  let (status, answer) = get(&front, &format!("/rooms/{room}")).await;

  assert_eq!(status, StatusCode::OK, "{answer}");
  assert_eq!(answer["supervisors"], json!([ADDED]), "the new room names the supervisor alone");

  // What the console set the supervisor goes with it: its draft process row and its question texts.
  let (status, answer) = put(
    &front,
    &format!("/draft-questions/{ADDED}/tone"),
    json!({ "text": "Is the tone right?" }),
  )
  .await;

  assert_eq!(status, StatusCode::OK, "{answer}");

  let (status, answer) = put(
    &front,
    &format!("/draft-processes/*/*/{ADDED}"),
    json!({ "rounds": ["tone", "final"], "light": ["final"], "lightChars": 12 }),
  )
  .await;

  assert_eq!(status, StatusCode::OK, "{answer}");

  let (status, answer) = remove(&front, &format!("/supervisors/{ADDED}?by=tester&forgetBrain=true")).await;

  assert_eq!(status, StatusCode::NO_CONTENT, "{answer}");

  let (_, row) = get(&front, &format!("/draft-processes?agentName={ADDED}")).await;
  let (_, catalogue) = get(&front, &format!("/draft-questions?agentName={ADDED}")).await;

  assert_eq!(row["items"][0]["source"], json!("default"), "{row}");
  assert!(
    !catalogue["items"].as_array().unwrap().iter().any(|one| one["name"] == "tone"),
    "{catalogue}"
  );

  let removals = front.standing.agency.removals.lock().unwrap().clone();

  assert_eq!(removals.len(), 1);
  assert_eq!(
    removals[0].0, ADDED,
    "the supervisor is removed by the name the agency registers it under"
  );
  assert_eq!(removals[0].1.rooms, [room], "every room naming it is handed as one it shares");
  assert!(removals[0].1.forget_brain && !removals[0].1.force);
  assert_eq!(naming(&scratch).await, 0);
}

#[tokio::test]
async fn a_seeded_supervisor_and_the_default_answerer_are_refused_and_the_agency_is_asked_no_removal() {
  let scratch = Scratch::new().await;
  let front = bench(&scratch).await;
  let (status, answer) = post(&front, "/supervisors", added()).await;

  assert_eq!(status, StatusCode::CREATED, "{answer}");

  for agent in front.standing.agency.agents.lock().unwrap().iter_mut() {
    agent.row.insert("seeded".to_owned(), json!(true));
  }

  let (status, answer) = remove(&front, &format!("/supervisors/{ADDED}?by=tester")).await;

  assert_eq!(
    (status, answer["code"].clone(), answer["error"].clone()),
    (
      StatusCode::CONFLICT,
      json!("seeded"),
      json!("Agent D is seeded by the deployment, so it cannot be removed")
    ),
    "{answer}"
  );
  assert_eq!(
    *front.standing.agency.supervisors_read.lock().unwrap(),
    [ADDED],
    "the supervisor is read afresh before anything is asked"
  );

  front
    .standing
    .agency
    .registering(vec![agent(AGENT, "Agent A"), agent(ADDED, "Agent D")]);

  let (status, answer) = remove(&front, &format!("/supervisors/{AGENT}")).await;

  assert_eq!(
    (status, answer["code"].clone()),
    (StatusCode::CONFLICT, json!("default-answerer")),
    "{answer}"
  );
  assert!(
    front.standing.agency.removals.lock().unwrap().is_empty(),
    "the agency is never asked to take out a seeded supervisor or the default answerer"
  );
  assert_eq!(naming(&scratch).await, 1, "its room half stays");

  *front.standing.agency.removal_refused.lock().unwrap() = Some("running".to_owned());

  let (status, answer) = remove(&front, &format!("/supervisors/{ADDED}")).await;

  assert_eq!((status, answer["code"].clone()), (StatusCode::CONFLICT, json!("running")), "{answer}");
  assert_eq!(naming(&scratch).await, 1, "a removal the agency refused takes nothing of the room half");

  let (status, answer) = remove(&front, "/supervisors/agent-z").await;

  assert_eq!(
    (status, answer["code"].clone()),
    (StatusCode::NOT_FOUND, json!("unknown-supervisor")),
    "{answer}"
  );
}

#[tokio::test]
async fn a_field_the_agency_refuses_comes_back_as_the_receivers_code_and_nothing_is_written() {
  let scratch = Scratch::new().await;
  let front = bench(&scratch).await;

  *front.standing.agency.refusing.lock().unwrap() = Some(FieldRefusal {
    field: "provider".to_owned(),
    error: "The provider does not carry the chat line.".to_owned(),
  });

  let (status, answer) = post(&front, "/supervisors", added()).await;

  assert_eq!((status, answer["code"].clone()), (StatusCode::BAD_REQUEST, json!("provider")));
  assert_eq!(answer["error"], "the agency refused the road on the provider provider-a");
  assert_eq!(naming(&scratch).await, 0);
  assert_eq!(
    scratch
      .count(r#"SELECT COUNT(*) FROM "provider_rooms" WHERE "room_id" = $1"#, vec![ADDED.into()])
      .await,
    0
  );
  assert!(
    front.standing.gate.copied.lock().unwrap().is_empty(),
    "no options are copied for a refused row"
  );

  let mut unheld = added();

  unheld["repository"] = json!("repo-z");
  *front.standing.agency.refusing.lock().unwrap() = None;

  let (status, answer) = post(&front, "/supervisors", unheld).await;

  assert_eq!(
    (status, answer["code"].clone()),
    (StatusCode::BAD_REQUEST, json!("repository")),
    "{answer}"
  );
  assert!(
    front.standing.agency.added_rows.lock().unwrap().len() == 1,
    "the agency is asked nothing more"
  );
}

#[tokio::test]
async fn the_whole_road_goes_to_the_agencys_one_add_and_an_order_is_not_handed_on() {
  let scratch = Scratch::new().await;
  let front = bench(&scratch).await;
  let (status, answer) = post(
    &front,
    "/supervisors",
    json!({
      "name": ADDED,
      "label": "Agent D",
      "room": "A room of its own",
      "provider": "provider-a",
      "model": "model-a",
      "effort": "high",
      "thinking": "on",
      "answerTokens": 8000,
      "order": [{ "provider": "provider-b", "model": "model-b" }],
      "quotaShare": 0.2,
    }),
  )
  .await;

  assert_eq!(status, StatusCode::CREATED, "{answer}");

  let rows = front.standing.agency.added_rows.lock().unwrap().clone();

  assert_eq!(rows.len(), 1, "the supervisor and its road go to the agency in one ask");
  assert_eq!(
    serde_json::to_value(&rows[0]).unwrap(),
    json!({
      "name": ADDED,
      "label": "Agent D",
      "provider": "provider-a",
      "model": "model-a",
      "effort": "high",
      "thinking": "on",
      "answerTokens": 8000,
    }),
    "the road is handed on as it came, and no order nor share the agency refuses"
  );
  assert_eq!(naming(&scratch).await, 1);
}

#[tokio::test]
async fn a_supervisor_added_for_an_account_has_its_room_kept_as_that_accounts_private_room() {
  let scratch = Scratch::new().await;
  let front = bench(&scratch).await;
  let (status, answer) = post(
    &front,
    "/supervisors",
    json!({ "name": ADDED, "label": "Agent D", "room": "A room of its own", "ownerAccount": 7 }),
  )
  .await;

  assert_eq!(status, StatusCode::CREATED, "{answer}");

  let room = answer["room"]["id"].as_i64().unwrap();
  let rows = front.standing.agency.added_rows.lock().unwrap().clone();

  assert_eq!(rows[0].owner_account, Some(7), "the agency is told whose it is");
  assert_eq!(
    scratch
      .texts(&format!(r#"SELECT "account"::text FROM "private_rooms" WHERE "chat_id" = {room}"#))
      .await,
    ["7"],
    "its room is kept as the account's private room from the start"
  );
}

#[tokio::test]
async fn a_reset_is_handed_the_rooms_that_name_the_supervisor_alone_and_beside_another() {
  let scratch = Scratch::new().await;
  let front = bench(&scratch).await;

  front
    .standing
    .agency
    .registering(vec![agent(AGENT, "Agent A"), agent(ADDED, "Agent D")]);
  scratch
    .run(
      r#"INSERT INTO "room_supervisors" ("chat_id", "supervisor") VALUES (-11, $1), (-12, $1), (-12, $2)"#,
      vec![ADDED.into(), AGENT.into()],
    )
    .await;

  let (status, answer) = post(
    &front,
    &format!("/supervisors/{ADDED}/resets"),
    json!({ "conversation": true, "dryRun": true, "by": "tester" }),
  )
  .await;

  assert_eq!(status, StatusCode::OK, "{answer}");
  assert_eq!(answer["agent"], ADDED, "the agency's answer is the route's");
  assert_eq!(
    *front.standing.agency.resets.lock().unwrap(),
    [(
      ADDED.to_owned(),
      json!({ "alone": [-11], "shared": [-12], "conversation": true, "dryRun": true, "by": "tester" })
    )]
  );

  let (status, answer) = post(&front, "/supervisors/agent-z/resets", json!({ "person": true })).await;

  assert_eq!((status, answer["code"].clone()), (StatusCode::NOT_FOUND, json!("unknown-supervisor")));
}

#[tokio::test]
async fn a_rooms_list_and_the_default_answerer_are_read_and_written_and_say_whose_rooms_are_whose() {
  let scratch = Scratch::new().await;
  let front = bench(&scratch).await;

  front
    .standing
    .agency
    .registering(vec![agent(AGENT, "Agent A"), agent(ADDED, "Agent D")]);
  scratch
    .run(
      r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES (-21, 'named'), (-22, 'unnamed')"#,
      vec![],
    )
    .await;

  let (status, answer) = put(&front, "/rooms/-21/supervisors", json!({ "supervisors": [ADDED] })).await;

  assert_eq!(
    (status, answer.clone()),
    (StatusCode::OK, json!({ "supervisors": [ADDED] })),
    "{answer}"
  );

  let (status, answer) = put(&front, "/rooms/-21/supervisors", json!({ "supervisors": ["agent-z"] })).await;

  assert_eq!((status, answer["code"].clone()), (StatusCode::NOT_FOUND, json!("unknown-supervisor")));

  let (_, answer) = get(&front, "/default-answerer").await;

  assert_eq!(
    answer,
    json!({ "supervisor": AGENT }),
    "the configuration's agent stands until one is named"
  );

  let (_, answer) = get(&front, &format!("/supervisors/{AGENT}/rooms")).await;
  let ids: Vec<i64> = answer["items"]
    .as_array()
    .unwrap()
    .iter()
    .map(|room| room["id"].as_i64().unwrap())
    .collect();

  assert_eq!(ids, [-22], "the default answerer answers the room naming none");
  assert_eq!(answer["items"][0]["named"], false);

  let (_, answer) = get(&front, &format!("/supervisors/{ADDED}/rooms")).await;

  assert_eq!(answer["items"], json!([{ "id": -21, "title": "named", "named": true, "alone": true }]));

  let (status, answer) = put(&front, "/default-answerer", json!({ "supervisor": "agent-z" })).await;

  assert_eq!((status, answer["code"].clone()), (StatusCode::NOT_FOUND, json!("unknown-supervisor")));

  let (status, answer) = put(&front, "/default-answerer", json!({ "supervisor": ADDED })).await;

  assert_eq!((status, answer.clone()), (StatusCode::OK, json!({ "supervisor": ADDED })), "{answer}");
  assert_eq!(front.standing.receiver.default_answerer(), ADDED);
  assert_eq!(
    scratch
      .texts(r#"SELECT "value" FROM "settings" WHERE "key" = 'default_answerer'"#)
      .await,
    [ADDED],
    "the setting is kept in the store"
  );

  let (_, answer) = get(&front, "/rooms/-22/answerers").await;

  assert_eq!(
    answer["default"], ADDED,
    "a room naming none is answered by the supervisor the setting names"
  );

  let (status, answer) = put(&front, "/rooms/-21/supervisors", json!({ "supervisors": [] })).await;

  assert_eq!(
    (status, answer),
    (StatusCode::OK, json!({ "supervisors": [] })),
    "an empty list hands the room back"
  );
}

#[tokio::test]
async fn a_new_supervisors_console_room_is_born_on_the_record_and_its_first_answer_is_kept_there() {
  use bmo_receiver::chat::reply::{OutgoingReply, send_reply};
  use bmo_receiver::roads::Road;

  let scratch = Scratch::new().await;
  let front = bench(&scratch).await;
  let (status, answer) = post(&front, "/supervisors", added()).await;

  assert_eq!(status, StatusCode::CREATED, "{answer}");

  let room = answer["room"]["id"].as_i64().unwrap();
  let receiver = &front.standing.receiver;

  assert_eq!(
    scratch
      .texts(&format!(r#"SELECT "provider" FROM "room_providers" WHERE "chat_id" = {room}"#))
      .await,
    ["record"],
    "the room is born with the record as its one provider"
  );
  assert_eq!(receiver.roads().answering(room, Some("record")).await, Some(Road::Record));
  assert_eq!(receiver.roads().answering(room, None).await, Some(Road::Record));
  assert_eq!(receiver.roads().unasked(room).await, Some(Road::Record));

  // The agency now registers the supervisor under its label.
  front.standing.agency.registering(vec![Agent {
    name: ADDED.to_owned(),
    label: Some("Agent D".to_owned()),
    awake: true,
    brain_folder: None,
    row: serde_json::Map::new(),
  }]);

  let sent = send_reply(
    receiver,
    &OutgoingReply {
      agent: Some(ADDED.to_owned()),
      chat_id: room,
      text: "The first answer.".to_owned(),
      via: Some("record".to_owned()),
      ..OutgoingReply::default()
    },
  )
  .await;

  assert!(sent.is_some(), "the answer has a road");
  assert_eq!(
    scratch
      .count(r#"SELECT COUNT(*) FROM "messages" WHERE "chat_id" = $1"#, vec![room.into()])
      .await,
    1,
    "the first answer is kept in the room"
  );

  // The answer is the supervisor's own: its name, its mark and the number the room gave it.
  let kept = scratch
    .texts(&format!(
      r#"SELECT "sender_name" || '|' || COALESCE("agent_name", '') || '|' || COALESCE("sender_id"::text, '') FROM "messages" WHERE "chat_id" = {room}"#
    ))
    .await;
  let given = scratch
    .texts(&format!(
      r#"SELECT "sender_id"::text FROM "provider_users" WHERE "provider" = 'record' AND "user_id" = '{ADDED}'"#
    ))
    .await;

  assert_eq!(
    kept,
    [format!("Agent D|{ADDED}|{}", given[0])],
    "the answer stands under the agent's own name"
  );
}

#[tokio::test]
async fn a_room_whose_providers_are_the_record_alone_answers_on_it_and_an_unknown_room_has_no_road() {
  use bmo_receiver::roads::{Road, providers};

  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;
  let receiver = &front.standing.receiver;

  providers::choose(receiver.db(), -77, &["record".to_owned()]).await.unwrap();

  assert_eq!(receiver.roads().answering(-77, Some("record")).await, Some(Road::Record));
  assert_eq!(receiver.roads().unasked(-77).await, Some(Road::Record));
  assert_eq!(
    receiver.roads().answering(-78, None).await,
    None,
    "a room the receiver never saw has no road"
  );
}

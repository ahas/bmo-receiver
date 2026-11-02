//! An agent added on the console and removed there (docs/api.md, 「Agents added on the console」): the agency
//! writes the agent's half and the gate copies the chat agent's options before the receiver writes the room
//! half; the agent's name goes to the agency's removal, a seeded one's never; and a field the agency
//! refuses comes back as the receiver's code with nothing written.

use axum::http::StatusCode;
use serde_json::{Value, json};

use receiver_ports::agency::{Agent, FieldRefusal, Repository};

use crate::integration::common::Scratch;
use crate::integration::common::queue::{Front, post, remove, stood};
use crate::integration::common::units::{AGENT, OWNER};

const ADDED: &str = "agent-d";
const WORKER: &str = "repo-a";

// The front, with the agency holding one repository an added agent's items may go to.
async fn bench(scratch: &Scratch) -> Front {
  let front = stood(scratch, OWNER).await;

  *front.standing.agency.repositories.lock().unwrap() = vec![Repository {
    name: WORKER.to_owned(),
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
    "worker": WORKER,
    "room": "A room of its own",
  })
}

async fn answerers(scratch: &Scratch) -> i64 {
  scratch
    .count(r#"SELECT COUNT(*) FROM "room_answerers" WHERE "agent" = $1"#, vec![ADDED.into()])
    .await
}

#[tokio::test]
async fn an_added_agent_stands_with_its_gate_options_copied_and_its_name_goes_to_the_agencys_removal() {
  let scratch = Scratch::new().await;
  let front = bench(&scratch).await;
  let (status, answer) = post(&front, "/companions", added()).await;

  assert_eq!(status, StatusCode::OK, "{answer}");

  assert_eq!(
    *front.standing.gate.copied.lock().unwrap(),
    [(ADDED.to_owned(), AGENT.to_owned())],
    "the gate copies the chat agent's options for the added agent"
  );
  assert_eq!(answerers(&scratch).await, 1);

  let (status, answer) = remove(&front, &format!("/companions/{ADDED}?by=tester")).await;

  assert_eq!(status, StatusCode::OK, "{answer}");
  assert_eq!(
    *front.standing.agency.removals.lock().unwrap(),
    [ADDED.to_owned()],
    "the agent is removed by the name the agency registers it under"
  );
  assert_eq!(answerers(&scratch).await, 0);
}

#[tokio::test]
async fn a_seeded_agent_is_refused_as_seeded_and_the_agency_is_asked_no_removal() {
  let scratch = Scratch::new().await;
  let front = bench(&scratch).await;
  let (status, answer) = post(&front, "/companions", added()).await;

  assert_eq!(status, StatusCode::OK, "{answer}");

  for agent in front.standing.agency.agents.lock().unwrap().iter_mut() {
    agent.row.insert("seeded".to_owned(), json!(true));
  }

  let (status, answer) = remove(&front, &format!("/companions/{ADDED}?by=tester")).await;

  assert_eq!(
    (status, answer["code"].clone(), answer["error"].clone()),
    (
      StatusCode::CONFLICT,
      json!("seeded-companion"),
      json!("Agent D is a seeded agent, so it cannot be removed")
    ),
    "{answer}"
  );
  assert!(
    front.standing.agency.removals.lock().unwrap().is_empty(),
    "the agency is never asked to take out a seeded agent"
  );
  assert_eq!(
    *front.standing.agency.supervisors_read.lock().unwrap(),
    [ADDED],
    "the agent's supervisor is read afresh before anything is asked"
  );
  assert_eq!(answerers(&scratch).await, 1, "its room half stays");
}

#[tokio::test]
async fn a_field_the_agency_refuses_comes_back_as_the_receivers_code_and_nothing_is_written() {
  let scratch = Scratch::new().await;
  let front = bench(&scratch).await;

  *front.standing.agency.refusing.lock().unwrap() = Some(FieldRefusal {
    field: "provider".to_owned(),
    error: "The provider carries no chat agent.".to_owned(),
  });

  let (status, answer) = post(&front, "/companions", added()).await;

  assert_eq!((status, answer["code"].clone()), (StatusCode::BAD_REQUEST, json!("provider")));
  assert_eq!(answer["error"], "the provider provider-a cannot carry a chat agent");
  assert_eq!(answerers(&scratch).await, 0);
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
}

#[tokio::test]
async fn an_order_handed_whole_goes_to_the_agencys_one_add_with_its_share() {
  let scratch = Scratch::new().await;
  let front = bench(&scratch).await;
  let order = json!([
    { "provider": "provider-a", "model": "model-a", "effort": "high", "thinking": "on" },
    { "provider": "provider-b", "model": "model-b" },
  ]);
  let (status, answer) = post(
    &front,
    "/companions",
    json!({
      "name": ADDED,
      "label": "Agent D",
      "worker": WORKER,
      "room": "A room of its own",
      "order": order,
      "quotaShare": 0.2,
    }),
  )
  .await;

  assert_eq!(status, StatusCode::OK, "{answer}");

  let rows = front.standing.agency.added_rows.lock().unwrap().clone();

  assert_eq!(rows.len(), 1, "the agent and its order go to the agency in one ask");
  assert_eq!(
    serde_json::to_value(&rows[0]).unwrap(),
    json!({
      "name": ADDED,
      "label": "Agent D",
      "worker": WORKER,
      "order": order,
      "quotaShare": 0.2,
    }),
    "the order and its share are handed on as they came, with no provider or model the console did not name"
  );
  assert_eq!(answerers(&scratch).await, 1);
}

#[tokio::test]
async fn an_order_or_a_share_the_agency_refuses_comes_back_as_its_own_code() {
  let scratch = Scratch::new().await;
  let front = bench(&scratch).await;
  let refusing = |field: &str| {
    *front.standing.agency.refusing.lock().unwrap() = Some(FieldRefusal {
      field: field.to_owned(),
      error: "Refused.".to_owned(),
    });
  };
  let ordered = || {
    json!({
      "name": ADDED,
      "label": "Agent D",
      "worker": WORKER,
      "room": "A room of its own",
      "order": [{ "provider": "provider-a", "model": "model-a" }],
      "quotaShare": 2,
    })
  };

  refusing("order[0]");

  let (status, answer) = post(&front, "/companions", ordered()).await;

  assert_eq!(
    (status, answer["code"].clone(), answer["error"].clone()),
    (
      StatusCode::BAD_REQUEST,
      json!("order"),
      json!("the agency refused the order at order[0]")
    ),
    "{answer}"
  );

  refusing("quotaShare");

  let (status, answer) = post(&front, "/companions", ordered()).await;

  assert_eq!(
    (status, answer["code"].clone()),
    (StatusCode::BAD_REQUEST, json!("quota-share")),
    "{answer}"
  );
  assert_eq!(answerers(&scratch).await, 0, "nothing of the room is written");
}

#[tokio::test]
async fn an_added_agents_console_room_is_born_on_the_record_and_its_first_answer_is_kept_there() {
  use bmo_receiver::chat::reply::{OutgoingReply, send_reply};
  use bmo_receiver::roads::Road;

  let scratch = Scratch::new().await;
  let front = bench(&scratch).await;
  let (status, answer) = post(&front, "/companions", added()).await;

  assert_eq!(status, StatusCode::OK, "{answer}");

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

  // The agency now registers the added agent under its label.
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

  // The answer is the added agent's own: its name, its mark and the number the room gave it.
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

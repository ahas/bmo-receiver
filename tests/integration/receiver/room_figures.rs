//! A room's figures as the console reads them (`docs/boundary-api.md`, 「A room's figures」): the messages counted,
//! the people who said most, the hours they were said in a time zone, and each of the room's agents — those
//! answering there and those keeping a session for it — with what the agency says of its row and its session; an
//! agency that answers nothing leaves its fields null, and a zone the database does not know is refused.

use axum::http::StatusCode;
use serde_json::{Value, json};

use receiver_ports::agency::{Agent, Scope, Session, SessionState};

use crate::integration::common::Scratch;
use crate::integration::common::queue::{Front, get, stood};
use crate::integration::common::units::{AGENT, OWNER, SENDER};

const ROOM: i64 = -911;
const OTHER_ROOM: i64 = -912;
// A second person of the room, and the number a road gave the other agent answering in it.
const PERSON: i64 = 900;
const OTHER: &str = "agent-b";
const OTHER_NUMBER: i64 = 77;
// Midnight of a day, UTC, in Unix seconds, and a minute.
const MIDNIGHT: i64 = 1_792_368_000;
const MINUTE: i64 = 60;

fn at(hour: i64, minute: i64) -> i64 {
  MIDNIGHT + (hour * 60 + minute) * MINUTE
}

// A message said at a moment counted in seconds, kept in the store's milliseconds.
async fn said(scratch: &Scratch, message: i64, sender: Option<i64>, name: &str, sent_at: i64) {
  scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text")
         VALUES ($1, $2, $3, $4, $5, 'words')"#,
      vec![
        ROOM.into(),
        message.into(),
        sender.into(),
        name.to_owned().into(),
        (sent_at * 1_000).into(),
      ],
    )
    .await;
}

fn agent(name: &str, label: &str, row: Value) -> Agent {
  Agent {
    name: name.to_owned(),
    label: Some(label.to_owned()),
    awake: true,
    brain_folder: None,
    row: row.as_object().unwrap().clone(),
  }
}

fn session(
  id: &str,
  agent: &str,
  scope: Scope,
  provider: &str,
  model: &str,
  carried: i64,
) -> (Session, SessionState) {
  (
    Session {
      id: id.to_owned(),
      agent: agent.to_owned(),
      scope,
    },
    SessionState {
      session: id.to_owned(),
      provider: Some(provider.to_owned()),
      model: Some(model.to_owned()),
      place: Value::Null,
      generation: Value::Null,
      context: json!({ "carried": carried, "window": 200_000 }),
      fold_ratio: None,
      folds_itself: false,
      supervisor: None,
      role: None,
      prefix_change_reason: None,
    },
  )
}

// Two people and two agents in a room: the chat agent filing under its name, keeping its session across rooms
// that has been given the room, and another agent answering there under the number its road gave it, keeping a
// session of the room's own.
async fn bench() -> (Scratch, Front) {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  said(&scratch, 1, Some(OWNER), "someone", at(0, 30)).await;
  said(&scratch, 2, Some(OWNER), "someone", at(1, 10)).await;
  said(&scratch, 3, Some(PERSON), "an old name", at(1, 20)).await;
  said(&scratch, 4, Some(PERSON), "a person", at(13, 0)).await;
  said(&scratch, 5, None, SENDER, at(13, 5)).await;
  said(&scratch, 6, Some(OTHER_NUMBER), "Agent B", at(23, 30)).await;
  said(&scratch, 7, Some(OWNER), "someone", at(23, 40)).await;

  for (sql, values) in [
    (
      r#"INSERT INTO "provider_users" ("provider", "user_id", "sender_id") VALUES ('road-a', $1, $2)"#,
      vec![OTHER.into(), OTHER_NUMBER.into()],
    ),
    (
      r#"INSERT INTO "room_answerers" ("chat_id", "agent") VALUES ($1, $2)"#,
      vec![ROOM.into(), OTHER.into()],
    ),
    (
      r#"INSERT INTO "bmo_session_rooms" ("session_id", "chat_id") VALUES ('shared-a', $1)"#,
      vec![ROOM.into()],
    ),
  ] {
    scratch.run(sql, values).await;
  }

  let agency = &front.standing.agency;

  agency.registering(vec![
    agent(
      AGENT,
      "Agent A",
      json!({ "provider": "provider-a", "model": "model-a", "effort": "high" }),
    ),
    agent(OTHER, "Agent B", json!({})),
  ]);
  agency.listing(vec![
    session("shared-a", AGENT, Scope::Agent, "provider-a", "model-a", 1_200),
    session("room-b", OTHER, Scope::Room { room: ROOM }, "provider-b", "model-b", 300),
    session("shared-b", OTHER, Scope::Agent, "provider-b", "model-b", 50),
    session("room-c", "agent-c", Scope::Room { room: OTHER_ROOM }, "provider-c", "model-c", 10),
  ]);

  (scratch, front)
}

async fn figures(front: &Front, query: &str) -> Value {
  let (status, figures) = get(front, &format!("/rooms/{ROOM}/figures{query}")).await;

  assert_eq!(status, StatusCode::OK, "{figures}");
  figures
}

fn hours(counted: &[(usize, i64)]) -> Value {
  let mut hours = [0_i64; 24];

  for (hour, count) in counted {
    hours[*hour] = *count;
  }

  json!(hours)
}

#[tokio::test]
async fn a_rooms_messages_are_counted_with_the_people_who_said_most() {
  let (_scratch, front) = bench().await;
  let figures = figures(&front, "").await;

  assert_eq!(figures["room"], ROOM);
  assert_eq!(figures["messages"], 7);
  assert_eq!(figures["fromPeople"], 5, "the agents' words are not a person's");
  assert_eq!(figures["speakers"], 2);
  assert_eq!(figures["firstAt"], at(0, 30) * 1_000);
  assert_eq!(figures["lastAt"], at(23, 40) * 1_000);
  assert_eq!(figures["lastAnswerAt"], at(23, 30) * 1_000);
  assert_eq!(
    figures["top"],
    json!([{ "name": "someone", "messages": 3 }, { "name": "a person", "messages": 2 }]),
    "each person under the name they last spoke under",
  );
}

#[tokio::test]
async fn the_hours_are_counted_in_the_zone_asked_for() {
  let (_scratch, front) = bench().await;

  assert_eq!(figures(&front, "").await["byHour"], hours(&[(0, 1), (1, 2), (13, 2), (23, 2)]));
  assert_eq!(
    figures(&front, "?zone=Asia/Tokyo").await["byHour"],
    hours(&[(9, 1), (10, 2), (22, 2), (8, 2)]),
  );

  let (status, refused) = get(&front, &format!("/rooms/{ROOM}/figures?zone=Nowhere/Atall")).await;

  assert_eq!((status, refused["code"].as_str()), (StatusCode::BAD_REQUEST, Some("bad-request")));
}

#[tokio::test]
async fn each_agent_stands_with_its_share_its_row_and_the_session_it_keeps_for_the_room() {
  let (_scratch, front) = bench().await;
  let figures = figures(&front, "").await;

  assert_eq!(
    figures["agents"],
    json!([
      {
        "agentName": OTHER,
        "messages": 1,
        "lastAnswerAt": at(23, 30) * 1_000,
        "session": "room-b",
        "label": "Agent B",
        "lastMode": null,
        "lastModel": null,
        "lastEffort": null,
        "sessionMode": "provider-b",
        "model": "model-b",
        "contextTokens": 300,
        "runs": null,
        "lastRunAt": null,
        "weekTokens": null,
        "totalTokens": null,
      },
      {
        "agentName": AGENT,
        "messages": 1,
        "lastAnswerAt": at(13, 5) * 1_000,
        "session": "shared-a",
        "label": "Agent A",
        "lastMode": "provider-a",
        "lastModel": "model-a",
        "lastEffort": "high",
        "sessionMode": "provider-a",
        "model": "model-a",
        "contextTokens": 1_200,
        "runs": null,
        "lastRunAt": null,
        "weekTokens": null,
        "totalTokens": null,
      },
    ]),
    "the answerer first, then the agent keeping a session given the room; another room's session names nobody",
  );
}

#[tokio::test]
async fn an_agency_that_answers_nothing_leaves_its_fields_null_and_the_figures_stand() {
  let (_scratch, front) = bench().await;

  front.standing.agency.going_down();

  let figures = figures(&front, "").await;

  assert_eq!(figures["fromPeople"], 5, "the other agent's number is the receiver's own record");
  assert_eq!(
    figures["agents"].as_array().unwrap().len(),
    1,
    "the answerer stands; no session is listed"
  );
  assert_eq!(figures["agents"][0]["agentName"], OTHER);
  assert_eq!(figures["agents"][0]["messages"], 1);

  for field in ["session", "label", "lastMode", "sessionMode", "model", "contextTokens"] {
    assert_eq!(figures["agents"][0][field], Value::Null, "{field}");
  }
}

#[tokio::test]
async fn a_room_holding_nothing_counts_nothing() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;
  let figures = figures(&front, "").await;

  assert_eq!(
    figures,
    json!({
      "room": ROOM,
      "messages": 0,
      "fromPeople": 0,
      "speakers": 0,
      "firstAt": null,
      "lastAt": null,
      "lastAnswerAt": null,
      "top": [],
      "byHour": hours(&[]),
      "agents": [],
    }),
  );
}

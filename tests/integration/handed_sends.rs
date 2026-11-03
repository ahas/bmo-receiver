//! Words handed to the room's supervisor (docs/messages.md, 「Words handed to the room's supervisor」): a send asked
//! in its agent's own round goes out, and every other — a command at a shell outside any turn, an agent outside
//! its round, a job, a spoken talk, a consult line — is queued as an order for the room's supervisor, and
//! nothing reaches the room on that call.

use std::sync::Arc;

use axum::{
  Router,
  body::{Body, to_bytes},
  extract::Request,
  http::StatusCode,
};
use sea_orm::ConnectionTrait;
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

use crate::integration::common::Scratch;
use crate::integration::common::units::{
  ADDRESS, AGENT, OWNER, SENDER, StandInChat, Standing, registration, standing,
};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::routes::routes;
use bmo_receiver::verbs::asked_sends::shell_asker;
use room_calls::sends::{Act, Asked, SayInput};

const TOKEN: &str = "the-token";
const ROOM: i64 = -100;

// A receiver over a scratch database, its rounds standing as working everywhere, reaching one road that holds
// the room, with the default answerer registered.
async fn stood() -> (Scratch, Router, Arc<StandInChat>, Standing, TempDir) {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let road = Arc::new(StandInChat::new("road-a").holding(&[ROOM]).owners(OWNER));
  let standing = standing(scratch.db(), DataRoot::new(data.path()), [road.clone()].to_vec());
  let mut agent = registration(AGENT, SENDER, [ADDRESS].to_vec(), None);

  agent.row.insert("role".to_owned(), json!("chat"));
  standing.agency.registering([agent].to_vec());

  let port = routes(standing.receiver.clone(), TOKEN);

  (scratch, port, road, standing, data)
}

async fn asked(port: &Router, path: &str, body: Value) -> (StatusCode, Value) {
  let request = Request::post(path)
    .header("authorization", format!("Bearer {TOKEN}"))
    .body(Body::from(body.to_string()))
    .unwrap();
  let answer = port.clone().oneshot(request).await.unwrap();
  let status = answer.status();
  let written = to_bytes(answer.into_body(), usize::MAX).await.unwrap();

  (status, serde_json::from_slice(&written).unwrap_or(Value::Null))
}

fn say(asker: Value, text: &str) -> Value {
  json!({ "asker": asker, "tool": "say", "input": { "text": text } })
}

// The orders queued, each its agent, room, sort and words.
async fn orders(scratch: &Scratch) -> Vec<(String, i64, String, String)> {
  let rows = scratch
    .db()
    .query_all(sea_orm::Statement::from_string(
      sea_orm::DbBackend::Postgres,
      r#"SELECT "agent", "chat_id", "sort", "text" FROM "instructions" ORDER BY "id""#,
    ))
    .await
    .unwrap();

  rows
    .iter()
    .map(|row| {
      (
        row.try_get::<String>("", "agent").unwrap(),
        row.try_get::<i64>("", "chat_id").unwrap(),
        row.try_get::<Option<String>>("", "sort").unwrap().unwrap_or_default(),
        row.try_get::<String>("", "text").unwrap(),
      )
    })
    .collect()
}

#[tokio::test]
async fn a_say_a_command_asks_outside_any_turn_becomes_an_order_for_the_rooms_supervisor() {
  let (scratch, port, road, _standing, _data) = stood().await;
  let asker = shell_asker(ROOM, None, None, "room say");

  assert_eq!(asker.agent_name, None, "the test runs outside any agent's turn");

  let body = serde_json::to_value(Asked {
    asker,
    act: Act::Say(SayInput {
      chat: None,
      via: None,
      text: "The build is green.".to_owned(),
      reply: false,
      reply_to: None,
    }),
  })
  .unwrap();
  let (status, answer) = asked(&port, "/sends", body).await;

  assert_eq!(status, StatusCode::OK, "{answer:?}");
  assert_eq!(answer["refusal"], Value::Null, "{answer:?}");
  assert!(answer["handed"].as_i64().is_some(), "{answer:?}");
  assert!(
    answer["lines"][0]
      .as_str()
      .is_some_and(|line| line.starts_with(&format!("Handed to {AGENT}"))),
    "{answer:?}"
  );
  assert!(road.sent().is_empty(), "nothing reached the room");

  let orders = orders(&scratch).await;

  assert_eq!(orders.len(), 1, "{orders:?}");

  let (agent, room, sort, text) = &orders[0];

  assert_eq!((agent.as_str(), *room, sort.as_str()), (AGENT, ROOM, "handed"));
  assert!(text.contains("a line: The build is green."), "{text}");
  assert!(text.contains("a process outside any agent's turn, through bmo room say"), "{text}");
  assert!(text.contains("your rule for handed lines"), "{text}");
}

#[tokio::test]
async fn a_send_in_the_agents_own_round_goes_out_and_the_same_send_outside_it_is_handed() {
  let (scratch, port, road, standing, _data) = stood().await;
  let turn = json!({ "room": ROOM, "agentName": AGENT, "by": format!("chat:{ROOM}") });

  let (status, answer) = asked(&port, "/sends", say(turn.clone(), "Done.")).await;

  assert_eq!(status, StatusCode::OK, "{answer:?}");
  assert_eq!(answer["handed"], Value::Null, "the round's own send is carried: {answer:?}");
  assert!(orders(&scratch).await.is_empty(), "nothing was handed");

  standing.taken.standing_idle();

  let (status, answer) = asked(&port, "/sends", say(turn, "Done again.")).await;

  assert_eq!(status, StatusCode::OK, "{answer:?}");
  assert!(answer["handed"].as_i64().is_some(), "{answer:?}");

  let orders = orders(&scratch).await;

  assert_eq!(orders.len(), 1, "{orders:?}");
  assert!(orders[0].3.contains(&format!("{AGENT}, from outside a round of its own")), "{orders:?}");
  assert!(road.sent().iter().all(|sent| sent.room_id == ROOM), "a carried send stays in its room");
}

#[tokio::test]
async fn a_jobs_send_is_handed_even_while_the_round_works() {
  let (scratch, port, road, _standing, _data) = stood().await;
  let job = json!({ "room": ROOM, "agentName": AGENT, "by": format!("chat:{ROOM}"), "job": true, "from": "bmo pin" });
  let body = json!({ "asker": job, "tool": "pin", "input": { "message": 7 } });

  let (status, answer) = asked(&port, "/sends", body).await;

  assert_eq!(status, StatusCode::OK, "{answer:?}");
  assert!(answer["handed"].as_i64().is_some(), "{answer:?}");
  assert!(road.sent().is_empty(), "nothing reached the room");

  let text = &orders(&scratch).await[0].3;

  assert!(text.contains("a job's run, through bmo pin"), "{text}");
  assert!(text.contains("the send tool's pin entry, asked with:"), "{text}");
  assert!(text.contains("\"message\":7"), "{text}");
}

#[tokio::test]
async fn a_spoken_talk_is_handed_as_its_words_and_in_its_own_round_is_left_to_the_command() {
  let (scratch, port, road, standing, _data) = stood().await;
  let speak = |asker: Value| {
    json!({
      "asker": asker,
      "tool": "speak",
      "input": { "voice": "voice-a", "text": "Three items closed.", "spoken": "Three items closed, spoken." },
    })
  };
  let turn = json!({ "room": ROOM, "agentName": AGENT, "by": format!("chat:{ROOM}") });

  let (status, answer) = asked(&port, "/sends", speak(turn)).await;

  assert_eq!(status, StatusCode::OK, "{answer:?}");
  assert_eq!(
    answer,
    json!({ "lines": [], "own": true }),
    "the command makes the note in its own round"
  );

  standing.taken.standing_idle();

  let (status, answer) = asked(&port, "/sends", speak(json!({ "room": ROOM, "from": "bmo talk" }))).await;

  assert_eq!(status, StatusCode::OK, "{answer:?}");
  assert!(answer["handed"].as_i64().is_some(), "{answer:?}");
  assert!(road.sent().is_empty(), "nothing was synthesised or sent");

  let text = &orders(&scratch).await[0].3;

  assert!(text.contains("a voice note in the voice voice-a"), "{text}");
  assert!(text.contains("Its written words, what the room reads: Three items closed."), "{text}");
  assert!(
    text.contains("Its spoken words, what the voice says: Three items closed, spoken."),
    "{text}"
  );
}

#[tokio::test]
async fn a_consult_line_outside_the_agents_round_is_handed_to_it_in_the_consult_room() {
  let (scratch, port, road, standing, _data) = stood().await;

  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value") VALUES ('consult_room', $1)"#,
      vec![ROOM.to_string().into()],
    )
    .await;
  standing.taken.standing_idle();

  let (status, answer) = asked(
    &port,
    "/consult-lines",
    json!({ "agent": AGENT, "text": "Shall we split it?", "from": "bmo consult say" }),
  )
  .await;

  assert_eq!(status, StatusCode::OK, "{answer:?}");
  assert_eq!(answer["room"], json!(ROOM), "{answer:?}");
  assert_eq!(answer["id"], Value::Null, "{answer:?}");
  assert!(answer["handed"].as_i64().is_some(), "{answer:?}");
  assert!(road.sent().is_empty(), "nothing reached the consult room");

  let orders = orders(&scratch).await;

  assert_eq!((orders[0].0.as_str(), orders[0].1), (AGENT, ROOM));
  assert!(orders[0].3.contains("a line in the consult room under your own name"), "{orders:?}");
  assert!(orders[0].3.contains("Shall we split it?"), "{orders:?}");
}

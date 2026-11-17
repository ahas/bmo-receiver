//! Words handed to the room's supervisor (docs/messages.md, 「Words handed to the room's supervisor」): words go
//! into a room only as a round's message, so a send holding words asked in its agent's own round into that round's
//! room is refused, one asked into another room is handed to that room's supervisor, and only a send holding none
//! goes out in the round; every other — a command at a shell outside any turn, an agent outside its round, a job,
//! a spoken talk — is queued as an order for the room's supervisor, and nothing reaches the room on that call.

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
use crate::integration::common::alone::alone;
use crate::integration::common::units::{
  ADDRESS, AGENT, OWNER, SENDER, StandInChat, Standing, registration, standing,
};
use bmo_receiver::chat::relays;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::routes::routes;
use bmo_receiver::verbs::asked_sends::shell_asker;
use receiver_policy::chat_round::CHAT_ROUND;
use room_calls::sends::{Act, Asked, SayInput};

const TOKEN: &str = "the-token";
const ROOM: i64 = -100;
const OTHER: i64 = -200;

// A receiver over a scratch database, its rounds standing as working everywhere, reaching one road that holds
// the room, with the default answerer registered.
async fn stood() -> (Scratch, Router, Arc<StandInChat>, Standing, TempDir) {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let road = Arc::new(StandInChat::new("road-a").holding(&[ROOM, OTHER]).owners(OWNER));
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
  let Some(_run) = alone(
    module_path!(),
    "a_say_a_command_asks_outside_any_turn_becomes_an_order_for_the_rooms_supervisor",
  ) else {
    return;
  };

  // SAFETY: this process runs this one test alone, so no other thread reads the environment.
  unsafe {
    std::env::remove_var("BMO_AGENT_PLACE");
    std::env::remove_var("BMO_TURN_AGENT");
    std::env::remove_var("BMO_TASK_LOCKED");
  }

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
  assert!(text.starts_with(&CHAT_ROUND.handed.marker), "{text}");
  assert!(text.contains("a line: The build is green."), "{text}");
  assert!(text.contains("a process outside any agent's turn, through bmo room say"), "{text}");
  assert!(
    !text.contains(&CHAT_ROUND.handed.rule),
    "the turn carries only the marker and its data; the rule stands in the room's block: {text}"
  );
}

#[tokio::test]
async fn words_asked_in_the_agents_own_round_into_its_room_are_refused_and_nothing_goes_out() {
  let (scratch, port, road, _standing, _data) = stood().await;
  let turn = json!({ "room": ROOM, "agentName": AGENT, "by": format!("chat:{ROOM}") });

  let (status, answer) = asked(&port, "/sends", say(turn, "Checking the card now.")).await;

  assert_eq!(status, StatusCode::OK, "{answer:?}");
  assert_eq!(answer["handed"], Value::Null, "{answer:?}");
  assert!(
    answer["refusal"].as_str().is_some_and(|why| why.contains("go into the round's answer")),
    "a line in the round's own room is the round's answer: {answer:?}"
  );
  assert!(road.sent().is_empty(), "no line went out without the draft process");
  assert!(orders(&scratch).await.is_empty(), "nothing was handed");
}

#[tokio::test]
async fn words_asked_in_a_round_into_another_room_are_handed_to_that_rooms_supervisor_and_nothing_goes_out()
{
  let (scratch, port, road, _standing, _data) = stood().await;
  let turn = json!({ "room": OTHER, "agentName": AGENT, "by": format!("chat:{ROOM}") });

  let (status, answer) = asked(&port, "/sends", say(turn, "The plan for Saturday stands.")).await;

  assert_eq!(status, StatusCode::OK, "{answer:?}");
  assert_eq!(answer["refusal"], Value::Null, "{answer:?}");
  assert!(answer["handed"].as_i64().is_some(), "{answer:?}");
  assert!(road.sent().is_empty(), "the other room's round writes and reviews the words");

  let orders = orders(&scratch).await;

  assert_eq!(orders.len(), 1, "{orders:?}");
  assert_eq!((orders[0].1, orders[0].2.as_str()), (OTHER, "handed"), "{orders:?}");
  assert!(orders[0].3.contains("a line: The plan for Saturday stands."), "{orders:?}");
}

#[tokio::test]
async fn a_caption_in_the_agents_own_round_is_refused_and_what_holds_no_words_goes_out() {
  let (scratch, port, road, _standing, _data) = stood().await;
  let turn = json!({ "room": ROOM, "agentName": AGENT, "by": format!("chat:{ROOM}") });
  let photo = json!({
    "asker": turn,
    "tool": "photo",
    "input": { "pictures": [{ "url": "https://example.com/a.png" }], "caption": "Here is the chart." },
  });

  let (status, answer) = asked(&port, "/sends", photo).await;

  assert_eq!(status, StatusCode::OK, "{answer:?}");
  assert!(
    answer["refusal"].as_str().is_some_and(|why| why.contains("go into the round's answer")),
    "{answer:?}"
  );
  assert!(road.sent().is_empty(), "the captioned picture did not go");

  let sticker = json!({ "asker": turn, "tool": "sticker", "input": { "emoji": "👍" } });
  let (status, answer) = asked(&port, "/sends", sticker).await;

  assert_eq!(status, StatusCode::OK, "{answer:?}");
  assert_eq!(answer["handed"], Value::Null, "{answer:?}");
  assert!(orders(&scratch).await.is_empty(), "nothing was handed");
  assert!(
    road.sent().iter().all(|sent| sent.room_id == ROOM),
    "a send of no words is carried in its round"
  );
}

#[tokio::test]
async fn a_line_handed_with_an_answer_wanted_relays_nothing_until_its_round_sends() {
  let (scratch, port, _road, _standing, _data) = stood().await;
  let turn = json!({ "room": OTHER, "agentName": AGENT, "by": format!("chat:{ROOM}") });
  let body =
    json!({ "asker": turn, "tool": "say", "input": { "text": "Is Saturday still on?", "reply": true } });

  let (status, answer) = asked(&port, "/sends", body).await;

  assert_eq!(status, StatusCode::OK, "{answer:?}");

  let order = answer["handed"].as_i64().unwrap();
  let waiting = |scratch: &Scratch| {
    let db = scratch.db().clone();

    async move {
      db.query_all(sea_orm::Statement::from_string(
        sea_orm::DbBackend::Postgres,
        format!(r#"SELECT "reply_to", "order_id" FROM "say_relays" WHERE "chat_id" = {OTHER}"#),
      ))
      .await
      .unwrap()
      .iter()
      .map(|row| {
        (
          row.try_get::<i64>("", "reply_to").unwrap(),
          row.try_get::<Option<i64>>("", "order_id").unwrap(),
        )
      })
      .collect::<Vec<_>>()
    }
  };

  assert_eq!(waiting(&scratch).await, [(ROOM, Some(order))].to_vec(), "the relay waits on the order");

  relays::arm(scratch.db(), OTHER, &[order]).await.unwrap();

  assert_eq!(
    waiting(&scratch).await,
    [(ROOM, None)].to_vec(),
    "the round that sent it armed the relay"
  );
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
async fn a_spoken_talk_is_handed_as_its_words_and_in_its_own_round_is_refused() {
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
  assert_eq!(answer["own"], Value::Null, "{answer:?}");
  assert!(
    answer["refusal"].as_str().is_some_and(|why| why.contains("go into the round's answer")),
    "a voice note in the round's own room is words for its answer: {answer:?}"
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

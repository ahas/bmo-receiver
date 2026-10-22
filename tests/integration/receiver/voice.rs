//! A briefing event the writer outside the receiver brings (docs/api.md, 「Events」): the listeners hear it whole,
//! and a malformed one reaches nobody.

use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use serde_json::{Value, json};
use tempfile::tempdir;

use bmo_receiver::data_root::DataRoot;
use bmo_receiver::events::Events;
use bmo_receiver::routes::routes;

use receiver_ports::agency::Agent;

use crate::common::Scratch;
use crate::common::queue::{Front, Kicked, TOKEN, post};
use crate::common::units::{AGENT, standing};

/// The front standing as the receiver for an owner, hearing its news in a stream of its own the test reads, and
/// registering the chat agent with the voice given.
async fn bench(voice: Value) -> (Scratch, Front, Events) {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let news = Events::new();
  let mut standing = standing(scratch.db(), DataRoot::new(data.path()), vec![]);

  standing.agency.registering(
    [Agent {
      name: AGENT.into(),
      label: Some("Agent A".into()),
      awake: true,
      brain_folder: None,
      row: json!({ "voice": voice, "defaultAnswerer": true }).as_object().unwrap().clone(),
    }]
    .to_vec(),
  );
  standing.receiver = standing.receiver.with_news(Arc::new(news.clone()));

  let kicked = Arc::new(Kicked(Mutex::default()));

  standing.receiver.hold_pass(kicked.clone());

  (
    scratch,
    Front {
      routes: routes(standing.receiver.clone(), TOKEN),
      data,
      standing,
      kicked,
    },
    news,
  )
}

#[tokio::test]
async fn a_briefing_event_reaches_the_listeners_whole_and_a_malformed_one_is_refused() {
  let (scratch, front, news) = bench(json!("bmo-voice")).await;

  let room = -402_i64;
  let parity: Value = serde_json::from_str(
    &std::fs::read_to_string("tests/fixtures/event-parity.json").expect("the parity fixture"),
  )
  .expect("its JSON");
  let emotion = parity["briefingEmotions"][1].clone();
  let data = json!({
    "chatId": room,
    "messageId": 1,
    "text": "The morning summary.",
    "seconds": 4.5,
    "sentAt": 1_760_000_000_000_i64,
    "emotion": emotion,
    "title": "Morning summary",
    "expression": "smiling",
  });
  let briefing = json!({ "kind": "briefing.created", "who": "voice runner", "data": data });

  scratch.message(room, 1, 7).await;

  let with = |key: &str, value: Value| {
    let mut changed = briefing.clone();

    changed["data"].as_object_mut().unwrap().insert(key.to_owned(), value);
    changed
  };
  let without = |key: &str| {
    let mut changed = briefing.clone();

    changed["data"].as_object_mut().unwrap().remove(key);
    changed
  };
  let refusals = [
    (without("chatId"), "a briefing event names its chatId and messageId as integers"),
    (
      with("messageId", json!(1.5)),
      "a briefing event names its chatId and messageId as integers",
    ),
    (
      without("seconds"),
      "a briefing event carries its text, its seconds (a number or null) and when it was sent",
    ),
    (
      with("seconds", json!(-1)),
      "a briefing event carries its text, its seconds (a number or null) and when it was sent",
    ),
    (
      with("emotion", json!("cheerful")),
      "a briefing event's emotion is one of the briefing emotions, or left out",
    ),
    (with("title", json!("  ")), "a briefing event's title is a line of text, or left out"),
    (with("expression", json!(3)), "a briefing event's expression is a word, or left out"),
    (with("expression", json!("")), "a briefing event's expression is a word, or left out"),
  ];

  for (asked, error) in refusals {
    assert_eq!(
      post(&front, "/events", asked.clone()).await,
      (StatusCode::BAD_REQUEST, json!({ "code": "bad-request", "error": error })),
      "{asked}",
    );
  }

  assert_eq!(
    post(&front, "/events", with("messageId", json!(2))).await,
    (
      StatusCode::NOT_FOUND,
      json!({ "code": "not-found", "error": format!("No message 2 in chat {room}.") })
    ),
  );
  assert_eq!(
    post(&front, "/events", json!({ "kind": "work-order", "data": { "chatId": room } })).await,
    (
      StatusCode::BAD_REQUEST,
      json!({ "code": "bad-request", "error": "Unknown event work-order; this route takes message.created, briefing.created, avatars.changed." }),
    ),
  );
  assert!(news.held().is_empty(), "a refused briefing reached the listeners: {:?}", news.held());

  assert_eq!(post(&front, "/events", briefing).await.0, StatusCode::NO_CONTENT);

  let reached: Vec<Value> = news
    .held()
    .iter()
    .filter(|event| event["type"] == "briefing" && event["chatId"] == room)
    .cloned()
    .collect();

  assert_eq!(reached.len(), 1, "{reached:?}");
  assert_eq!(reached[0]["messageId"], 1);
  assert_eq!(reached[0]["detail"], "voice runner");

  let heard = &reached[0]["briefing"];

  assert_eq!(heard["text"], "The morning summary.");
  assert_eq!(heard["seconds"], 4.5);
  assert_eq!(heard["sentAt"], 1_760_000_000_000_i64);
  assert_eq!(heard["emotion"], emotion);
  assert_eq!(heard["title"], "Morning summary");
  assert_eq!(heard["expression"], "smiling");
  assert!(heard["autoplay"].is_boolean(), "{heard}");

  // Left out, the three are not passed on at all.
  assert_eq!(
    post(
      &front,
      "/events",
      json!({
        "kind": "briefing.created",
        "data": { "chatId": room, "messageId": 1, "text": "Plain.", "seconds": null, "sentAt": 1 },
      }),
    )
    .await
    .0,
    StatusCode::NO_CONTENT,
  );

  let told: Vec<Value> = news
    .held()
    .iter()
    .filter(|event| event["type"] == "briefing" && event["chatId"] == room)
    .cloned()
    .collect();
  let plain = &told[1]["briefing"];

  assert!(["emotion", "title", "expression"].iter().all(|key| plain.get(key).is_none()), "{plain}");
}

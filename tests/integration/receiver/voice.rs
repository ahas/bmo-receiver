//! The voice briefing's switches and a line's full reading (docs/voiced-notices.md): one switch set at a time,
//! the owner told when the whole briefing moved, and a line's reading answered as it stands or refused before
//! anything is launched.

use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use serde_json::{Value, json};
use tempfile::tempdir;

use bmo_receiver::data_root::DataRoot;
use bmo_receiver::events::Events;
use bmo_receiver::routes::routes;

use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::roads::ROADS;

use receiver_ports::agency::Agent;
use receiver_ports::voice::{Briefed, Briefing, BriefingKind};

use crate::common::Scratch;
use crate::common::queue::{Front, Kicked, TOKEN, post};
use crate::common::units::{AGENT, OWNER, standing};

// The room a line is read in, and the number the chat agent's words there are filed under.
const ROOM: i64 = -401;
const AGENT_NUMBER: i64 = 7;

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
      row: json!({ "voice": voice, "default_answerer": 1 }).as_object().unwrap().clone(),
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

// The agent's own lines, somebody else's words, and a voice message, which is no line to read.
async fn lines(scratch: &Scratch) {
  let mark = ROADS.marks.voice.as_str();

  scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text") VALUES
         ($1, 1, $2, 'Agent A', 1760000000, 'A line of the agent''s.'),
         ($1, 2, 55, 'tester', 1760000001, 'Somebody else''s words.'),
         ($1, 3, $2, 'Agent A', 1760000002, $3),
         ($1, 4, $2, 'Agent A', 1760000003, 'Another line of the agent''s.')"#,
      vec![ROOM.into(), AGENT_NUMBER.into(), format!("{mark} heard").into()],
    )
    .await;
}

// How many notices the owner was told of the whole briefing moving.
async fn told(scratch: &Scratch) -> i64 {
  scratch
    .count(
      r#"SELECT COUNT(*) FROM "instructions" WHERE "chat_id" = $1 AND "kind" = $2"#,
      vec![OWNER.into(), CHAT_ROUND.voice.switch_kind.clone().into()],
    )
    .await
}

#[tokio::test]
async fn one_switch_is_set_at_a_time_and_moving_the_whole_briefing_queues_the_owners_notice_once() {
  let (scratch, front, _news) = bench(json!(null)).await;

  let before = told(&scratch).await;

  assert_eq!(
    post(&front, "/voice-briefing", json!({ "autoplay": false })).await,
    (StatusCode::OK, json!({ "ok": true, "on": true, "autoplay": false })),
  );
  assert_eq!(told(&scratch).await, before, "the playing alone says nothing to the owner");

  assert_eq!(
    post(&front, "/voice-briefing", json!({ "on": false })).await,
    (StatusCode::OK, json!({ "ok": true, "on": false, "autoplay": false })),
  );
  assert_eq!(told(&scratch).await, before + 1, "the whole briefing moved once");

  // Setting it where it stands says nothing, and one kind stands for the switch, so flipping it twice before the
  // session speaks leaves one line, about where it ended.
  assert_eq!(post(&front, "/voice-briefing", json!({ "on": false })).await.0, StatusCode::OK);
  assert_eq!(post(&front, "/voice-briefing", json!({ "on": true })).await.0, StatusCode::OK);
  assert_eq!(told(&scratch).await, before + 1);

  for asked in [json!({}), json!({ "on": true, "autoplay": true })] {
    assert_eq!(
      post(&front, "/voice-briefing", asked.clone()).await,
      (
        StatusCode::BAD_REQUEST,
        json!({ "code": "one-switch", "error": "say which switch is moved, on or autoplay, and whether it is true or false" }),
      ),
      "{asked}",
    );
  }

  // A switch said as a word is not the shape the route takes.
  assert_eq!(post(&front, "/voice-briefing", json!({ "on": "yes" })).await.0, StatusCode::BAD_REQUEST);
}

// A briefing a runner put in a room reaches every listener with its title, its emotion and the face's
// expression as they were told; a malformed one, or one naming a message the room's record does not hold, is
// refused and reaches nobody (docs/voiced-notices.md, 「What a listener hears」).
#[tokio::test]
async fn a_briefing_event_reaches_the_listeners_whole_and_a_malformed_one_is_refused() {
  let (scratch, front, news) = bench(json!("bmo-voice")).await;

  let room = -402_i64;
  let parity: Value = serde_json::from_str(
    &std::fs::read_to_string("tests/fixtures/event-parity.json").expect("the parity fixture"),
  )
  .expect("its JSON");
  let emotion = parity["briefingEmotions"][1].clone();
  let briefing = json!({
    "type": "briefing",
    "who": "voice runner",
    "chatId": room,
    "messageId": 1,
    "text": "The morning summary.",
    "seconds": 4.5,
    "sentAt": 1_760_000_000,
    "emotion": emotion,
    "title": "Morning summary",
    "expression": "smiling",
  });

  scratch.message(room, 1, 7).await;

  let with = |key: &str, value: Value| {
    let mut changed = briefing.clone();

    changed.as_object_mut().unwrap().insert(key.to_owned(), value);
    changed
  };
  let without = |key: &str| {
    let mut changed = briefing.clone();

    changed.as_object_mut().unwrap().remove(key);
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
      post(&front, "/event", asked.clone()).await,
      (StatusCode::BAD_REQUEST, json!({ "code": "bad-request", "error": error })),
      "{asked}",
    );
  }

  assert_eq!(
    post(&front, "/event", with("messageId", json!(2))).await,
    (
      StatusCode::NOT_FOUND,
      json!({ "code": "not-found", "error": format!("No message 2 in chat {room}.") })
    ),
  );
  assert_eq!(
    post(&front, "/event", json!({ "type": "work-order", "chatId": room })).await,
    (
      StatusCode::BAD_REQUEST,
      json!({ "code": "bad-request", "error": "Unknown event work-order; this route takes message, briefing, avatars-updated." }),
    ),
  );
  assert!(news.held().is_empty(), "a refused briefing reached the listeners: {:?}", news.held());

  assert_eq!(post(&front, "/event", briefing).await, (StatusCode::OK, json!({ "ok": true })));

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
  assert_eq!(heard["sentAt"], 1_760_000_000);
  assert_eq!(heard["emotion"], emotion);
  assert_eq!(heard["title"], "Morning summary");
  assert_eq!(heard["expression"], "smiling");
  assert!(heard["autoplay"].is_boolean(), "{heard}");

  // Left out, the three are not passed on at all.
  assert_eq!(
    post(
      &front,
      "/event",
      json!({ "type": "briefing", "chatId": room, "messageId": 1, "text": "Plain.", "seconds": null, "sentAt": 1 }),
    )
    .await
    .0,
    StatusCode::OK,
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

#[tokio::test]
async fn a_full_reading_is_answered_as_it_stands_and_a_line_that_is_no_agents_or_has_no_voice_is_refused() {
  let (scratch, front, news) = bench(json!("bmo-voice")).await;

  lines(&scratch).await;

  assert_eq!(
    post(&front, "/voice-briefing/line", json!({ "chatId": ROOM, "messageId": 1.5 }))
      .await
      .0,
    StatusCode::BAD_REQUEST,
  );

  for message_id in [2, 3, 9] {
    assert_eq!(
      post(&front, "/voice-briefing/line", json!({ "chatId": ROOM, "messageId": message_id }))
        .await
        .0,
      StatusCode::NOT_FOUND,
      "message {message_id}",
    );
  }

  // With nothing standing, the line is read: the reading is launched and the room's screens are told.
  let (status, made) =
    post(&front, "/voice-briefing/line", json!({ "chatId": ROOM, "messageId": 1 })).await;

  assert_eq!(status, StatusCode::OK, "{made}");
  assert_eq!(made, json!({ "ok": true, "state": "making" }));
  assert_eq!(
    front.standing.voice.launched(),
    vec![Briefing {
      chat: ROOM,
      message: Some(1),
      kind: BriefingKind::Summary,
      title: CHAT_ROUND.voice.untitled.clone(),
      awaits_line: true,
      text: "A line of the agent's.".into(),
    },],
  );
  assert!(
    news
      .held()
      .iter()
      .any(|event| event["type"] == "message" && event["chatId"] == ROOM && event["messageId"] == 1)
  );

  // A reading already kept for the line is answered as it stands, made or ready, and launches no other.
  front.standing.voice.keeping(vec![Briefed {
    run: "voice-run-1".into(),
    chat: ROOM,
    message: Some(4),
    voice_message: None,
    kind: BriefingKind::Summary,
    title: String::new(),
    finished: true,
  }]);

  assert_eq!(
    post(&front, "/voice-briefing/line", json!({ "chatId": ROOM, "messageId": 4 })).await,
    (StatusCode::OK, json!({ "ok": true, "state": "ready" })),
  );
  assert_eq!(front.standing.voice.launched().len(), 1);
}

#[tokio::test]
async fn a_line_of_an_agent_with_no_voice_is_refused_before_anything_is_launched() {
  let (scratch, front, _news) = bench(json!(null)).await;

  lines(&scratch).await;

  assert_eq!(
    post(&front, "/voice-briefing/line", json!({ "chatId": ROOM, "messageId": 1 })).await,
    (
      StatusCode::CONFLICT,
      json!({ "code": "voiceless", "error": "agent-a has no voice, so nothing is spoken. Say this in words only, and do not try to send it as a voice again" }),
    ),
  );
  assert!(front.standing.voice.launched().is_empty());
}

#[tokio::test]
async fn a_line_is_refused_in_the_switch_s_own_sentence_while_the_briefing_is_off() {
  let (scratch, front, _news) = bench(json!("bmo-voice")).await;

  lines(&scratch).await;

  assert_eq!(post(&front, "/voice-briefing", json!({ "on": false })).await.0, StatusCode::OK);
  assert_eq!(
    post(&front, "/voice-briefing/line", json!({ "chatId": ROOM, "messageId": 1 })).await,
    (
      StatusCode::CONFLICT,
      json!({ "code": "switched-off", "error": "the voice briefing is switched off; nothing is spoken until the owner switches it on again from the console. Say this in words only, and do not try to send it as a voice again" }),
    ),
  );
  assert!(front.standing.voice.launched().is_empty());
}

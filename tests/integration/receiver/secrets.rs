//! Secrets taken out (docs/messages.md, 「Secrets taken out」): a secret pasted into a room is replaced by the
//! marker before the record keeps the message, the payload beside it is kept or the room's other providers are
//! handed it, the owner's words from the console the same, and the rows the store already holds are cleaned in
//! place by `redact-kept`, which answers counts and numbers alone. Each secret-shaped value is assembled at run
//! time (docs/PERSONAL-DATA.md).

use std::collections::HashSet;
use std::sync::Arc;

use axum::{Router, body::Body, extract::Request, http::StatusCode};
use bmo_chat_client::{ChatFileKind, ChatSend, MessageFile};
use bmo_receiver::chat::shape::Reading;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::{Context, Room, rows};
use bmo_receiver::secrets::kept::redact_kept;
use bmo_receiver::secrets::marker;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::fill_marks;
use receiver_policy::roads::ROADS;
use serde_json::{Value, json};
use tower::ServiceExt;

use super::inbound;
use crate::common::Scratch;
use crate::common::queue::{Front, TOKEN, get, post, put, read, stood_on};
use crate::common::units::{OWNER, StandInChat, standing};

const ROOM: i64 = -120;

// A key of the `sk-` shape, put together here.
fn key() -> String {
  format!("{}{}", "sk", "-abcdefghijklmnopqrstuvwx")
}

// A platform token of sixty-four hexadecimal digits.
fn token() -> String {
  "0123456789abcdef".repeat(4)
}

// A front reaching two chat services that both hold the room, the room on both of them.
async fn front(scratch: &Scratch) -> (Front, Arc<StandInChat>, Arc<StandInChat>) {
  let a = Arc::new(StandInChat::new("road-a").holding(&[ROOM]).owners(OWNER));
  let b = Arc::new(StandInChat::new("road-b").holding(&[ROOM]));
  let front = stood_on(scratch, OWNER, vec![a.clone(), b.clone()]).await;

  scratch
    .run(r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES (-120, 'Alpha Room')"#, vec![])
    .await;
  put(&front, "/rooms/-120/providers", json!({ "providers": ["road-a", "road-b"] })).await;

  (front, a, b)
}

// A message handed over on the chat services' routes; the status answered.
async fn hand(routes: &Router, message: i64, body: Value) -> StatusCode {
  let asked = Request::put(format!("/chat/rooms/{ROOM}/messages/{message}"))
    .header("authorization", format!("Bearer {TOKEN}"))
    .header("content-type", "application/json")
    .body(Body::from(body.to_string()))
    .unwrap();

  read(routes.clone().oneshot(asked).await.unwrap()).await.0
}

// The words each send to a road carried.
fn sent_words(road: &StandInChat) -> Vec<String> {
  road
    .sent()
    .into_iter()
    .filter_map(|one| match one.send {
      ChatSend::Text { text, .. } => Some(text),
      _ => None,
    })
    .collect()
}

#[tokio::test]
async fn a_secret_a_chat_service_hands_over_is_kept_and_carried_as_the_marker() {
  let scratch = Scratch::new().await;
  let (front, _a, b) = front(&scratch).await;
  let routes = bmo_receiver::receiver::handover::routes(front.receiver(), TOKEN);
  let words = format!("the key is {} and the token {}", key(), token());
  let handed = json!({
    "origin": "live",
    "senderId": 55,
    "senderName": "tester",
    "sentAt": 1_760_000_000_000_i64,
    "text": words,
    "replyToMessageId": null,
    "via": "road-a",
    "album": null,
    "mode": "bot-account",
    "context": { "metadata": { "text": words, "id": 7 } },
    "room": { "kind": "group" },
  });
  let kept = format!("the key is {0} and the token {0}", marker());

  assert_eq!(hand(&routes, 5, handed).await, StatusCode::CREATED);
  assert_eq!(
    scratch
      .texts(r#"SELECT "text" FROM "messages" WHERE "chat_id" = -120 AND "message_id" = 5"#)
      .await,
    [kept.as_str()]
  );
  assert_eq!(
    sent_words(&b),
    [kept.as_str()],
    "the other provider is handed the marker, never the secret"
  );

  let (status, metadata) = get(&front, "/rooms/-120/messages/5/metadata").await;

  assert_eq!(status, StatusCode::OK, "{metadata}");
  assert_eq!(metadata["metadata"], json!({ "text": kept, "id": 7 }));

  // An edit's new words are read the same way.
  let edit = json!({
    "origin": "edit",
    "senderId": 55,
    "senderName": "tester",
    "sentAt": 1_760_000_000_000_i64,
    "text": format!("now it is {}", key()),
    "replyToMessageId": null,
    "via": "road-a",
    "album": null,
    "mode": "bot-account",
    "room": { "kind": "group" },
  });

  hand(&routes, 5, edit).await;
  assert_eq!(
    scratch
      .texts(r#"SELECT "text" FROM "messages" WHERE "chat_id" = -120 AND "message_id" = 5"#)
      .await,
    [format!("now it is {}", marker())]
  );
}

#[tokio::test]
async fn the_owners_words_from_the_console_are_kept_and_carried_as_the_marker() {
  let scratch = Scratch::new().await;
  let (front, a, b) = front(&scratch).await;
  let (status, said) = post(
    &front,
    "/owner/messages",
    json!({ "chatId": ROOM, "text": format!("{}={}", "OPENAI_API_KEY", key()) }),
  )
  .await;

  assert_eq!(status.as_u16(), 200, "{said}");

  let kept = format!("OPENAI_API_KEY={}", marker());

  assert_eq!(
    scratch.texts(r#"SELECT "text" FROM "messages" WHERE "chat_id" = -120"#).await,
    [kept.as_str()]
  );
  assert_eq!(sent_words(&a), [kept.as_str()]);
  assert_eq!(sent_words(&b), [kept]);
}

#[tokio::test]
async fn the_kept_rows_are_cleaned_in_place_and_counted_without_their_words() {
  let scratch = Scratch::new().await;
  let words = format!("the key is {}", key());

  scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "transcript")
         VALUES (-120, 1, 55, 'tester', 1000, $1, NULL), (-120, 2, 55, 'tester', 2000, 'plain words', $2),
                (-120, 3, 55, 'tester', 3000, 'nothing to hide', NULL)"#,
      vec![words.clone().into(), format!("heard {}", token()).into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "message_metadata" ("chat_id", "message_id", "metadata") VALUES (-120, 1, $1)"#,
      vec![json!({ "text": words, "id": 7 }).to_string().into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "room_situations" ("chat_id", "text", "updated_at") VALUES (-121, $1, 1000)"#,
      vec![format!("the owner shared {}", token()).into()],
    )
    .await;

  // A dry run counts what it would change and writes nothing.
  let counted = redact_kept(scratch.db(), false).await.unwrap();
  let counts = |report: &bmo_receiver::secrets::kept::Cleaned| {
    report
      .columns
      .iter()
      .map(|column| (format!("{}.{}", column.table, column.column), column.rows, column.runs))
      .collect::<Vec<_>>()
  };
  let wanted = vec![
    ("messages.text".to_owned(), 1, 1),
    ("messages.transcript".to_owned(), 1, 1),
    ("messages.situation".to_owned(), 0, 0),
    ("message_metadata.metadata".to_owned(), 1, 1),
    ("room_situations.text".to_owned(), 1, 1),
  ];

  assert_eq!(counts(&counted), wanted);
  assert_eq!(counted.messages.iter().copied().collect::<Vec<_>>(), [(-120, 1), (-120, 2)]);
  assert_eq!(counted.rooms.iter().copied().collect::<Vec<_>>(), [-121]);
  assert_eq!(
    scratch
      .texts(r#"SELECT "text" FROM "messages" WHERE "chat_id" = -120 AND "message_id" = 1"#)
      .await,
    [words.as_str()]
  );

  // The run itself writes the marker in place, and a second run finds nothing.
  let cleaned = redact_kept(scratch.db(), true).await.unwrap();

  assert_eq!(counts(&cleaned), wanted);
  assert_eq!(
    scratch
      .texts(r#"SELECT "text" FROM "messages" WHERE "chat_id" = -120 ORDER BY "message_id""#)
      .await,
    [
      format!("the key is {}", marker()),
      "plain words".to_owned(),
      "nothing to hide".to_owned()
    ]
  );
  assert_eq!(
    scratch
      .texts(r#"SELECT "transcript" FROM "messages" WHERE "chat_id" = -120 AND "message_id" = 2"#)
      .await,
    [format!("heard {}", marker())]
  );
  assert_eq!(
    scratch
      .texts(r#"SELECT "metadata" FROM "message_metadata" WHERE "chat_id" = -120"#)
      .await,
    [json!({ "text": format!("the key is {}", marker()), "id": 7 }).to_string()]
  );
  assert_eq!(
    scratch.texts(r#"SELECT "text" FROM "room_situations" WHERE "chat_id" = -121"#).await,
    [format!("the owner shared {}", marker())]
  );

  let again = redact_kept(scratch.db(), true).await.unwrap();

  assert!(again.columns.iter().all(|column| column.rows == 0 && column.runs == 0), "{again:?}");
  assert!(again.messages.is_empty() && again.rooms.is_empty());
}

#[tokio::test]
async fn an_owners_message_a_secret_was_taken_from_reaches_the_turn_with_its_note_and_a_heard_one_too() {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let chat = Arc::new(StandInChat::new("road-a").holding(&[ROOM]));
  let standing = standing(scratch.db(), DataRoot::new(data.path()), vec![chat]);
  let owner = 55;
  let marked = format!("here it is {}", marker());

  standing
    .receiver
    .history(&inbound(ROOM, 1, owner, &marked), &Context::default(), Room::default())
    .await
    .unwrap();
  standing
    .receiver
    .history(&inbound(ROOM, 2, 66, &marked), &Context::default(), Room::default())
    .await
    .unwrap();

  // What bmo-ear heard is kept as the shapes leave it.
  standing.hearing.saying(&format!("the key is {}", key()), Some(2.0));
  standing
    .receiver
    .attachment(
      &inbound(ROOM, 3, owner, &ROADS.marks.voice),
      &Context {
        carries_file: true,
        ..Context::default()
      },
      Room::default(),
      &[MessageFile {
        id: "1".into(),
        name: "voice.oga".into(),
        mime: "audio/ogg".into(),
        bytes: vec![0x4f, 0x67, 0x67, 0x53, 0x00],
        kind: Some(ChatFileKind::VoiceNote),
      }],
    )
    .await
    .unwrap();

  assert_eq!(
    scratch
      .texts(r#"SELECT "transcript" FROM "messages" WHERE "chat_id" = -120 AND "message_id" = 3"#)
      .await,
    [format!("the key is {}", marker())]
  );

  let note = fill_marks(&CHAT_ROUND.shape.secret_taken, &[("marker", marker())]);
  let shown = HashSet::new();
  let reading = Reading {
    receiver: &standing.receiver,
    owner,
    fetch_on_demand: false,
    shown: &shown,
  };
  let mut lines = vec![];

  for message in 1..=3 {
    let kept = rows::message(standing.receiver.db(), ROOM, message, "road-a").await.unwrap().unwrap();

    lines.push(reading.lines(&[kept]).await.unwrap().remove(0));
  }

  assert!(!note.contains("{marker}") && note.contains("bmo-secret put"), "{note}");
  assert!(lines[0].contains(&note), "the owner's message carries the note: {}", lines[0]);
  assert!(!lines[1].contains(&note), "another sender's message carries none: {}", lines[1]);
  assert!(lines[2].contains(&note), "the owner's heard voice note carries it: {}", lines[2]);
  assert!(lines.iter().all(|line| !line.contains(&key())));
}

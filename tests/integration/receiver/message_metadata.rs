//! What a chat service hands over beside a message as its context's `metadata` (`docs/boundary-api.md`, 「Reading a
//! room」): kept as it arrived by each hand-over that carries a message — a message, one carrying a file, an edit —
//! replaced by an edit that carries its own and left by one that carries none, kept for no message the record does
//! not hold, and read back unread beside the message's page.

use std::sync::Arc;

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::common::Scratch;
use crate::common::queue::{Front, get, post, stood_on};
use crate::common::units::{OWNER, StandInChat};

const ROOM: i64 = -811;

// A payload as a platform writes one: the words, their marks, and a file's shape.
fn payload(words: &str) -> Value {
  json!({
    "text": words,
    "entities": [{ "type": "custom_emoji", "offset": 0, "length": 2, "custom_emoji_id": "5000" }],
    "video": { "width": 640, "height": 360, "duration": 4 },
  })
}

fn handover(message: i64, words: &str, metadata: Option<Value>) -> Value {
  let mut handed = json!({
    "message": {
      "chatId": ROOM,
      "messageId": message,
      "senderId": OWNER,
      "senderName": "someone",
      "sentAt": 1,
      "text": words,
      "replyToMessageId": null,
      "via": "road-a",
      "album": null,
      "mode": "bot-account",
    },
  });

  if let Some(metadata) = metadata {
    handed["context"] = json!({ "metadata": metadata });
  }

  handed
}

async fn bench() -> (Scratch, Front, Front) {
  let scratch = Scratch::new().await;
  let chat = Arc::new(StandInChat::new("road-a").holding(&[ROOM]));
  let handovers = stood_on(&scratch, OWNER, vec![chat.clone()]).await.handovers();
  let console = stood_on(&scratch, OWNER, vec![chat]).await;

  (scratch, handovers, console)
}

async fn read_back(console: &Front, room: i64, message: i64) -> Value {
  let (status, answer) = get(console, &format!("/console/rooms/{room}/messages/{message}/metadata")).await;

  assert_eq!(status, StatusCode::OK, "{answer}");
  answer["metadata"].clone()
}

#[tokio::test]
async fn a_message_and_one_carrying_a_file_keep_what_arrived_beside_them() {
  let (scratch, handovers, console) = bench().await;

  assert_eq!(
    post(&handovers, "/message", handover(1, "said", Some(payload("said")))).await.0,
    StatusCode::OK
  );
  assert_eq!(
    post(&handovers, "/attachment", handover(2, "a file", Some(payload("a file")))).await.0,
    StatusCode::OK,
  );
  assert_eq!(post(&handovers, "/message", handover(3, "plain", None)).await.0, StatusCode::OK);

  assert_eq!(
    scratch
      .count(r#"SELECT count(*) FROM "messages" WHERE "chat_id" = $1"#, vec![ROOM.into()])
      .await,
    3,
    "the hand-overs were kept as messages"
  );
  assert_eq!(read_back(&console, ROOM, 1).await, payload("said"));
  assert_eq!(read_back(&console, ROOM, 2).await, payload("a file"));
  assert_eq!(read_back(&console, ROOM, 3).await, Value::Null, "a hand-over carrying none keeps none");
  assert_eq!(read_back(&console, ROOM, 99).await, Value::Null);
}

#[tokio::test]
async fn an_edit_replaces_what_was_kept_and_one_carrying_none_leaves_it() {
  let (_scratch, handovers, console) = bench().await;

  assert_eq!(
    post(&handovers, "/message", handover(1, "said", Some(payload("said")))).await.0,
    StatusCode::OK
  );
  assert_eq!(
    post(&handovers, "/edited", handover(1, "said again", Some(payload("said again"))))
      .await
      .0,
    StatusCode::OK,
  );
  assert_eq!(read_back(&console, ROOM, 1).await, payload("said again"));

  assert_eq!(post(&handovers, "/edited", handover(1, "said once more", None)).await.0, StatusCode::OK);
  assert_eq!(read_back(&console, ROOM, 1).await, payload("said again"));
}

#[tokio::test]
async fn a_message_the_record_does_not_hold_keeps_nothing() {
  let (scratch, handovers, console) = bench().await;

  assert_eq!(
    post(&handovers, "/edited", handover(5, "never said", Some(payload("never said"))))
      .await
      .0,
    StatusCode::OK,
  );
  assert_eq!(read_back(&console, ROOM, 5).await, Value::Null);
  assert_eq!(scratch.count(r#"SELECT count(*) FROM "message_metadata""#, vec![]).await, 0);
}

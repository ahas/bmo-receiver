//! A message a chat service hands over as a line the platform said (bmo-chat's `docs/contract.md`, 「Lines」;
//! `docs/api.md`, 「The chat services」): kept as kind `fixed-line` with the line's code and fields as they came,
//! beside the plain sentence the service wrote for it, whether said live or read back and whatever its code, and
//! served back so, a secret-shaped run in its fields as the marker; a message handed over with no line stays anyone's
//! words. The secret-shaped value is assembled at run time (docs/PERSONAL-DATA.md).

use std::sync::Arc;

use axum::http::StatusCode;
use bmo_receiver::secrets::marker;
use serde_json::{Value, json};

use crate::integration::common::Scratch;
use crate::integration::common::queue::{Front, get, put, stood_on};
use crate::integration::common::units::{OWNER, StandInChat};

const ROOM: i64 = -812;

fn person(sender_id: i64, name: &str) -> Value {
  json!({ "senderId": sender_id, "name": name })
}

fn handover(origin: &str, sender_id: i64, words: &str, line: Option<Value>) -> Value {
  let mut handed = json!({
    "origin": origin,
    "senderId": sender_id,
    "senderName": "someone",
    "sentAt": 1_000,
    "text": words,
    "replyToMessageId": null,
    "via": "road-a",
    "album": null,
    "mode": "bot-account",
  });

  if let Some(line) = line {
    handed["line"] = line;
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

// A hand-over taken: a message said live is created, one read back from the past is held.
async fn handed(handovers: &Front, message: i64, body: Value) {
  let created = body["origin"] == "live";
  let (status, answer) = put(handovers, &format!("/chat/rooms/{ROOM}/messages/{message}"), body).await;

  assert_eq!(status, if created { StatusCode::CREATED } else { StatusCode::OK }, "{answer}");
}

// The message as the console reads it back: its words, its kind and its line.
async fn served(console: &Front, message: i64) -> (Value, Value, Value) {
  let (status, answer) = get(console, &format!("/rooms/{ROOM}/messages/{message}")).await;

  assert_eq!(status, StatusCode::OK, "{answer}");
  (answer["text"].clone(), answer["kind"].clone(), answer["line"].clone())
}

#[tokio::test]
async fn people_joining_and_a_person_leaving_are_served_back_as_fixed_lines_and_words_as_words() {
  let (_scratch, handovers, console) = bench().await;
  let joined = json!({
    "code": "people-joined",
    "fields": { "people": [person(43, "A newcomer"), person(44, "Another newcomer")], "by": person(42, "A host") },
  });
  let left = json!({ "code": "person-left", "fields": { "person": person(43, "A newcomer") } });

  handed(
    &handovers,
    1,
    handover("live", 42, "A newcomer and Another newcomer joined", Some(joined.clone())),
  )
  .await;
  handed(&handovers, 2, handover("live", 43, "A newcomer left", Some(left.clone()))).await;
  handed(&handovers, 3, handover("live", 42, "welcome", None)).await;

  assert_eq!(
    served(&console, 1).await,
    (json!("A newcomer and Another newcomer joined"), json!("fixed-line"), joined)
  );
  assert_eq!(served(&console, 2).await, (json!("A newcomer left"), json!("fixed-line"), left));
  assert_eq!(served(&console, 3).await, (json!("welcome"), Value::Null, Value::Null));
}

#[tokio::test]
async fn a_line_read_back_from_the_past_is_kept_as_one_whatever_its_code() {
  let (_scratch, handovers, console) = bench().await;
  let unknown = json!({ "code": "no-such-line", "fields": { "count": 3, "note": "as it came" } });

  handed(&handovers, 4, handover("history", 42, "something happened", Some(unknown.clone()))).await;

  assert_eq!(served(&console, 4).await, (json!("something happened"), json!("fixed-line"), unknown));
}

#[tokio::test]
async fn a_secret_in_a_lines_fields_is_kept_as_the_marker() {
  let (_scratch, handovers, console) = bench().await;
  let key = format!("{}{}", "sk", "-abcdefghijklmnopqrstuvwx");
  let left = |name: &str| json!({ "code": "person-left", "fields": { "person": person(43, name) } });

  handed(&handovers, 5, handover("live", 43, "someone left", Some(left(&format!("named {key}"))))).await;

  assert_eq!(served(&console, 5).await.2, left(&format!("named {}", marker())));
}

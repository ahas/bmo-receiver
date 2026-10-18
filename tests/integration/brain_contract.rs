//! The brain's reads of a room, checked against the brain's own document (`docs/boundary-api.md`, 「For the
//! brain」, word for word against the brain's `docs/boundary-api.md`, 「The receiver」): every one a `POST` in
//! camelCase, answered with the fields the brain's document names.

use axum::http::StatusCode;
use serde_json::json;

use crate::common::Scratch;
use crate::common::queue::{get, post, stood};
use crate::common::units::{AGENT, OWNER};

#[tokio::test]
async fn the_brains_routes_answer_the_fields_its_document_names() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  scratch.message(7, 1, OWNER).await;

  // The unread messages, the counts, the read ones and the marks.
  assert_eq!(
    post(
      &front,
      "/messages/unread",
      json!({ "agent": AGENT, "chatId": 7, "limit": 10, "since": null })
    )
    .await,
    (
      StatusCode::OK,
      json!({ "messages": [json!({ "chatId": 7, "messageId": 1, "senderId": OWNER, "senderName": "someone", "sentAt": 1, "text": "words", "replyToMessageId": null })] })
    ),
  );
  assert_eq!(
    post(&front, "/messages/unread-counts", json!({ "agent": AGENT, "since": [] })).await.1["rooms"]
      .as_array()
      .unwrap()
      .iter()
      .map(|room| room["chatId"].as_i64().unwrap())
      .collect::<Vec<i64>>(),
    [7],
  );
  assert_eq!(
    post(&front, "/messages/read", json!({ "agent": AGENT, "chatId": 7, "from": 0, "limit": 10 }))
      .await
      .1["messages"]
      .as_array()
      .unwrap()
      .len(),
    0,
  );
  assert_eq!(
    post(&front, "/read-marks/count", json!({ "agent": AGENT, "chatId": 7 })).await,
    (StatusCode::OK, json!({ "count": 0 })),
  );
  assert_eq!(
    post(&front, "/read-marks/keep", json!({ "agent": AGENT, "chatId": 7, "messageIds": [1] })).await,
    (StatusCode::OK, json!({ "ok": true })),
  );
  assert_eq!(
    post(&front, "/read-marks/count", json!({ "agent": AGENT, "chatId": 7 })).await.1["count"]
      .as_i64()
      .unwrap(),
    1,
  );
  assert_eq!(
    post(&front, "/read-marks/forget", json!({ "agent": AGENT, "chatId": null })).await.1["forgotten"]
      .as_i64()
      .unwrap(),
    1,
  );

  // The rooms, their titles, their answerers and their roads.
  assert_eq!(
    post(&front, "/rooms/titles", json!([])).await.1["rooms"]
      .as_array()
      .unwrap()
      .iter()
      .map(|room| room["chatId"].as_i64().unwrap())
      .collect::<Vec<i64>>(),
    [7],
  );
  assert_eq!(
    post(&front, "/rooms/answerers", json!({ "chatId": null })).await.1["rooms"]
      .as_array()
      .unwrap()
      .iter()
      .map(|room| room["chatId"].as_i64().unwrap())
      .collect::<Vec<i64>>(),
    [7],
  );
  assert_eq!(
    post(&front, "/rooms/roads", json!({ "chatId": 7 })).await,
    (StatusCode::OK, json!({ "kind": "group", "oneToOne": false })),
  );

  // The people.
  assert_eq!(
    post(
      &front,
      "/rooms/spoke-between",
      json!({ "chatId": 7, "person": OWNER, "from": 0, "to": 5 })
    )
    .await,
    (StatusCode::OK, json!({ "spoke": true })),
  );
  assert_eq!(
    post(&front, "/rooms/spoken-in", json!({ "chatId": 7, "people": [OWNER, 8] })).await.1["people"]
      .as_array()
      .unwrap()
      .len(),
    1,
  );
  assert_eq!(
    post(&front, "/people/names", json!({ "people": [OWNER] })).await.1["names"]
      .as_array()
      .unwrap()
      .iter()
      .map(|one| one["name"].as_str().unwrap().to_owned())
      .collect::<Vec<String>>(),
    ["someone"],
  );
  assert_eq!(
    post(
      &front,
      "/rooms/owner-spoke",
      json!({ "chatId": 7, "person": OWNER, "before": 5, "seconds": 10 })
    )
    .await,
    (StatusCode::OK, json!({ "owner": false, "spoke": false })),
  );
  assert_eq!(
    post(&front, "/messages/sender-is-owner", json!({ "chatId": 7, "messageId": 1 })).await,
    (StatusCode::OK, json!({ "owner": false })),
  );

  // The agent's own number, and the two ways a row is queued.
  assert_eq!(
    post(&front, "/rooms/own-number", json!({ "agent": AGENT, "chatId": 7 })).await,
    (StatusCode::OK, json!({ "number": null })),
  );
  // Whether a text waits for something to happen first, by the receiver's reading of promises: the words it
  // stands for are the policy's, kept as data, so the sentence asked in is read the way they are.
  let waiting = std::fs::read_to_string("tests/fixtures/promises-waiting.txt")
    .unwrap()
    .trim()
    .to_owned();

  assert!(
    post(&front, "/promises/waits", json!({ "text": waiting })).await.1["waits"]
      .as_bool()
      .unwrap(),
  );
  assert!(
    !post(&front, "/promises/waits", json!({ "text": "I will tell you when it is done." }))
      .await
      .1["waits"]
      .as_bool()
      .unwrap(),
  );
  assert_eq!(
    post(&front, "/owner/notice", json!({ "code": "digest-paused" })).await.0,
    StatusCode::OK
  );
  assert_eq!(
    post(
      &front,
      "/instructions/quiet",
      json!({ "agent": AGENT, "chatId": 7, "text": "a memo.", "kind": "watch" })
    )
    .await
    .0,
    StatusCode::OK,
  );
}

#[tokio::test]
async fn a_route_the_brain_reaches_wrongly_is_refused() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  // A body that is not the shape the route takes is refused with the words naming it.
  let (status, answer) = post(&front, "/messages/unread", json!({})).await;

  assert_eq!(status, StatusCode::BAD_REQUEST);
  assert_eq!(answer["code"].as_str().unwrap(), "bad-request");

  // Reached by a method the route does not answer, the path names no route: the receiver answers where it
  // holds none, whatever the method was.
  let (method, where_none) = get(&front, "/messages/unread").await;

  assert_eq!(method, StatusCode::NOT_FOUND);
  assert_eq!(where_none["code"].as_str().unwrap(), "no-route");
}

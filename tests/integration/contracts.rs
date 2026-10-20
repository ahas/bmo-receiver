//! One contract test per sibling document (`docs/boundary-api.md`): what each sibling's own document states of
//! the routes the receiver serves, checked as that document words it — bmo-job's `docs/api.md` 「bmo-receiver」,
//! bmo-chat's `docs/protocol.md` 「Receiver routes」, the cache warmer's `docs/api.md` 「The ping hook」, and the
//! agency's `docs/api.md` 「Hooks」. The brain's 18 routes are `brain_contract.rs`.

use axum::http::StatusCode;
use serde_json::json;

use std::sync::Arc;

use crate::common::Scratch;
use crate::common::queue::{TOKEN, get, patch, pinged, post, put, remove, stood, stood_on};
use crate::common::units::{AGENT, OWNER, StandInChat};

#[tokio::test]
async fn bmo_jobs_calls_answer_as_its_document_states_them() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  // An instruction queued answers 201 with its id; its state reads in snake_case.
  let (made, queued) =
    post(&front, "/instructions", json!({ "said": { "order": "an order." }, "sort": "job" })).await;

  assert_eq!(made, StatusCode::CREATED);
  assert_eq!(
    queued
      .as_object()
      .unwrap()
      .iter()
      .map(|(key, _)| key.to_owned())
      .collect::<Vec<String>>(),
    ["id"]
  );

  let id = queued["id"].as_i64().unwrap();

  assert_eq!(
    get(&front, &format!("/instructions/{id}")).await.1["state"].as_str().unwrap(),
    "pending",
  );
  assert_eq!(
    remove(&front, &format!("/instructions/{id}")).await,
    (StatusCode::NO_CONTENT, json!({})),
  );

  // A room reads as a room whatever it is; the owner is read once.
  assert_eq!(
    get(&front, "/rooms/7")
      .await
      .1
      .as_object()
      .unwrap()
      .iter()
      .map(|(key, _)| key.to_owned())
      .collect::<Vec<String>>(),
    [
      "companions",
      "id",
      "lastMessageAt",
      "messageCount",
      "ownerRoom",
      "pinned",
      "registered",
      "senders",
      "title",
      "workRoom",
    ],
  );
  assert_eq!(
    get(&front, "/rooms/7/answerers")
      .await
      .1
      .as_object()
      .unwrap()
      .iter()
      .map(|(key, _)| key.to_owned())
      .collect::<Vec<String>>(),
    ["default", "items", "more", "voice"],
  );
  assert_eq!(
    get(&front, "/owner")
      .await
      .1
      .as_object()
      .unwrap()
      .iter()
      .map(|(key, _)| key.to_owned())
      .collect::<Vec<String>>(),
    ["room", "senderId"]
  );

  // A holder declared answers 204 with no body.
  assert_eq!(
    put(
      &front,
      "/room-holders/unit-a",
      json!({ "address": "http://127.0.0.1:9", "holdings": [] })
    )
    .await,
    (StatusCode::NO_CONTENT, json!({})),
  );
}

#[tokio::test]
async fn the_chat_protocols_handovers_are_served_behind_the_receivers_token() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await.handovers();
  let handed = json!({
    "origin": "live",
    "senderId": OWNER,
    "senderName": "someone",
    "sentAt": 1_000,
    "text": "said",
    "replyToMessageId": null,
    "via": "road-a",
    "album": null,
    "mode": "bot-account",
  });
  let path = "/chat/rooms/7/messages/21";

  assert_eq!(put(&front, path, handed.clone()).await.0, StatusCode::CREATED);
  assert_eq!(
    put(
      &front,
      path,
      json!({
        "origin": "live",
        "senderId": OWNER,
        "senderName": "someone",
        "sentAt": 1_000,
        "text": "said",
        "replyToMessageId": null,
        "via": "road-a",
        "album": null,
        "mode": "bot-account",
        "files": [{ "id": "1", "name": "a.txt", "mime": "text/plain", "bytes": "aGk=" }],
      }),
    )
    .await
    .0,
    StatusCode::OK,
  );
  assert_eq!(
    put(&front, path, json!({ "origin": "edit", "senderId": OWNER, "senderName": "someone", "sentAt": 1_000, "text": "said again", "replyToMessageId": null, "via": "road-a", "album": null, "mode": "bot-account" }))
      .await
      .0,
    StatusCode::OK,
  );
  let patched = patch(&front, path, json!({ "files": [], "gatherFailure": "gone" })).await;

  assert_eq!(patched.0, StatusCode::OK, "{patched:?}");
  assert_eq!(
    patch(&front, "/chat/rooms/7/messages/99", json!({ "files": [] })).await.0,
    StatusCode::NOT_FOUND,
  );
}

#[tokio::test]
async fn the_warmer_ping_is_answered_with_an_outcome_and_no_header_is_taken() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;
  let (status, answer) = pinged(
    &front,
    &format!("/warmer/ping?token={TOKEN}"),
    json!({ "consumer": "receiver", "target": "chat-touch", "kind": "chat_touch", "subject": AGENT, "bound_ms": 1800000 }),
  )
    .await;

  assert_eq!(status, StatusCode::OK);
  assert_eq!(answer["outcome"].as_str().unwrap(), "skipped");
  assert_eq!(answer["reason"].as_str().unwrap(), "no conversations stand to be touched",);

  // A ping of another target is passed over, and one behind a wrong token is refused.
  assert_eq!(
    pinged(
      &front,
      &format!("/warmer/ping?token={TOKEN}"),
      json!({ "consumer": "receiver", "target": "other", "kind": "chat_touch", "subject": AGENT, "bound_ms": 1 }),
    ).await.1["reason"].as_str().unwrap(),
    "the ping is not of the chat touch",
  );
  assert_eq!(
    pinged(&front, "/warmer/ping?token=wrong", json!({ "consumer": "receiver", "target": "chat-touch", "kind": "chat_touch", "subject": AGENT, "bound_ms": 1 })).await.0,
    StatusCode::UNAUTHORIZED,
  );
}

#[tokio::test]
async fn the_agency_hooks_answer_their_points_in_the_hooks_envelope() {
  let scratch = Scratch::new().await;
  // The room stands on a road, so the verbs hook has a road to answer for.
  let chat = Arc::new(StandInChat::new("road-a").holding(&[7]));
  let front = stood_on(&scratch, OWNER, vec![chat]).await;

  scratch
    .run(r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES (7, 'The room')"#, vec![])
    .await;

  // The room's title.
  assert_eq!(
    post(&front, "/hooks/room-title", json!({ "point": "room-title", "ask": 7 })).await.1["answer"]
      .as_str()
      .unwrap(),
    "The room",
  );
  assert!(
    post(&front, "/hooks/room-title", json!({ "point": "room-title", "ask": 8 })).await.1["answer"]
      .is_null(),
  );

  // The verbs the road the room is answered on performs, by the protocol's names for them.
  let verbs =
    post(&front, "/hooks/road-verbs", json!({ "point": "road-verbs", "ask": { "chat": 7 } })).await;

  assert_eq!(verbs.0, StatusCode::OK);
  assert_eq!(
    verbs.1["answer"]
      .as_object()
      .unwrap()
      .iter()
      .map(|(key, _)| key.to_owned())
      .collect::<Vec<String>>(),
    ["can"],
  );
  assert_eq!(
    verbs.1["answer"]["can"]
      .as_array()
      .unwrap()
      .iter()
      .map(|verb| verb.as_str().unwrap().to_owned())
      .collect::<Vec<String>>(),
    ["text", "pin", "delete"],
  );

  // A line typed into the program: no conversations stand, so no context is composed.
  assert!(
    post(
      &front,
      "/hooks/typed-line",
      json!({ "point": "typed-line", "ask": { "text": "a line", "sessionId": "s-1" } }),
    )
    .await
    .1["answer"]
      .is_null(),
  );

  // A point the receiver answers no hook for is refused.
  assert_eq!(
    post(&front, "/hooks/pane-claims", json!({ "point": "pane-claims", "ask": {} })).await.0,
    StatusCode::BAD_REQUEST,
  );
}

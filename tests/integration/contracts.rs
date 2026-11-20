//! One contract test per sibling document (`docs/boundary-api.md`): what each sibling's own document states of
//! the routes the receiver serves, checked as that document words it — bmo-job's `docs/api.md` 「bmo-receiver」,
//! bmo-chat's `docs/protocol.md` 「Receiver routes」, the cache warmer's `docs/api.md` 「The target-ping hook」,
//! the agency's `docs/api.md` 「Hooks」, and the gate's `docs/boundary-api.md` 「bmo-receiver」. The brain's 18
//! routes are `brain_contract.rs`.

use axum::http::StatusCode;
use serde_json::json;

use std::sync::Arc;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{
  RECEIVER_TOKEN, get, patch, pinged, post, put, remove, stood, stood_on,
};
use crate::integration::common::units::{AGENT, OWNER, StandInChat};

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
      "id",
      "lastMessageAt",
      "listened",
      "messageCount",
      "mode",
      "ownerRoom",
      "pinned",
      "privateOf",
      "registered",
      "sealed",
      "senders",
      "supervisors",
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
    put(&front, "/room-holders/unit-a", json!({ "address": "http://[::1]:9", "holdings": [] })).await,
    (StatusCode::NO_CONTENT, json!({})),
  );
}

// The keys of an answer's object, sorted as the map holds them.
fn keys(value: &serde_json::Value) -> Vec<String> {
  value.as_object().unwrap().keys().cloned().collect()
}

#[tokio::test]
async fn the_gates_calls_answer_as_its_document_states_them() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  scratch.message(7, 1, OWNER).await;

  // Every room's title and senders.
  let rooms = get(&front, "/rooms").await.1;
  let room = &rooms["items"][0];

  assert_eq!((room["id"].as_i64(), room["senders"].clone()), (Some(7), json!([OWNER])));
  assert!(room.as_object().unwrap().contains_key("title"));

  // The names people went under: a page of `Person`, each its `id` and `name`.
  assert_eq!(
    get(&front, &format!("/people?ids={OWNER}")).await,
    (StatusCode::OK, json!({ "items": [{ "id": OWNER, "name": "someone" }], "more": false })),
  );

  // One stored message, its sender, name and moment in the common format.
  let (found, message) = get(&front, "/rooms/7/messages/1").await;

  assert_eq!(found, StatusCode::OK);
  assert_eq!(
    (
      message["senderId"].as_i64(),
      message["senderName"].as_str(),
      message["sentAt"].as_i64(),
      message["text"].as_str()
    ),
    (Some(OWNER), Some("someone"), Some(1_000), Some("words")),
  );
  assert!(!keys(&message).contains(&"sent_at".to_owned()), "{:?}", keys(&message));
  assert_eq!(get(&front, "/rooms/7/messages/2").await.1["code"], "not-found");

  // The facts read by room and whether a text waits.
  assert_eq!(keys(&get(&front, "/rooms/7/roads").await.1), ["kind", "oneToOne"]);
  assert_eq!(
    keys(
      &get(&front, &format!("/rooms/7/owner-spoke?person={OWNER}&before=5000&windowMs=10000"))
        .await
        .1
    ),
    ["owner", "spoke"],
  );
  assert_eq!(
    keys(&get(&front, &format!("/rooms/7/spoke-between?person={OWNER}&from=0&to=5000")).await.1),
    ["spoke"],
  );
  assert_eq!(
    keys(&get(&front, &format!("/rooms/7/own-number?agentName={AGENT}")).await.1),
    ["number"]
  );
  assert_eq!(keys(&post(&front, "/promise-readings", json!({ "text": "words" })).await.1), ["waits"]);

  // No second family of the same facts is served.
  for path in [
    "/rooms/titles",
    "/rooms/roads",
    "/rooms/spoke-between",
    "/rooms/owner-spoke",
    "/people/names",
    "/promises/waits",
    "/rooms/own-number",
    "/owner/notice",
  ] {
    let (status, refused) = post(&front, path, json!({})).await;

    assert_eq!((status, refused["code"].as_str()), (StatusCode::NOT_FOUND, Some("no-route")), "{path}");
  }
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
    &format!("/warmer/ping?token={RECEIVER_TOKEN}"),
    json!({ "consumerName": "receiver", "targetName": "chat-touch", "subjectName": AGENT, "kind": "chat-touch", "dueAt": 1_800_000, "boundMs": 1_800_000 }),
  )
    .await;

  assert_eq!(status, StatusCode::OK);
  assert_eq!(answer["outcome"].as_str().unwrap(), "skipped");
  assert_eq!(answer["code"].as_str().unwrap(), "no-conversation");
  assert_eq!(answer["reason"].as_str().unwrap(), "no conversations stand to be touched",);

  // A skipped answer carries the code the warmer reads it under (its `docs/api.md`, 「The target-ping hook」).
  let (_, answered) = pinged(
    &front,
    &format!("/warmer/ping?token={RECEIVER_TOKEN}"),
    json!({ "consumerName": "receiver", "targetName": "chat-touch", "subjectName": AGENT, "dueAt": 0, "boundMs": 1 }),
  )
  .await;
  assert_eq!(answered["outcome"].as_str().unwrap(), "skipped");
  assert_eq!(answered["code"].as_str().unwrap(), "no-conversation");

  // A ping of another target or another kind is passed over, and one behind a wrong token is refused.
  let (_, other_target) = pinged(
    &front,
    &format!("/warmer/ping?token={RECEIVER_TOKEN}"),
    json!({ "consumerName": "receiver", "targetName": "other", "subjectName": AGENT, "kind": "chat-touch", "dueAt": 0, "boundMs": 1 }),
  )
  .await;
  assert_eq!(other_target["outcome"].as_str().unwrap(), "skipped");
  assert_eq!(other_target["code"].as_str().unwrap(), "not-chat-touch");
  assert_eq!(other_target["reason"].as_str().unwrap(), "the ping is not of the chat touch");

  let (_, other_kind) = pinged(
    &front,
    &format!("/warmer/ping?token={RECEIVER_TOKEN}"),
    json!({ "consumerName": "receiver", "targetName": "chat-touch", "subjectName": AGENT, "kind": "other", "dueAt": 0, "boundMs": 1 }),
  )
  .await;
  assert_eq!(other_kind["outcome"].as_str().unwrap(), "skipped");
  assert_eq!(other_kind["code"].as_str().unwrap(), "not-chat-touch");
  assert_eq!(
    pinged(&front, "/warmer/ping?token=wrong", json!({ "consumerName": "receiver", "targetName": "chat-touch", "subjectName": AGENT, "kind": "chat-touch", "dueAt": 0, "boundMs": 1 })).await.0,
    StatusCode::UNAUTHORIZED,
  );
}

#[tokio::test]
async fn each_hook_the_agency_is_told_of_stands_at_a_route_the_receiver_answers() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  scratch
    .run(r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES (7, 'The room')"#, vec![])
    .await;

  for point in bmo_receiver::service_start::HOOKS {
    let url = bmo_receiver::service_start::hook_url("http://[::1]:9", point);
    let path = url.strip_prefix("http://[::1]:9").unwrap();

    assert!(
      bmo_receiver::routes::served().contains(&"POST /hooks/{point}".to_owned()),
      "the receiver serves its hooks"
    );
    assert_eq!(path, format!("/hooks/{point}"), "{point} is registered at the route that answers it");
  }

  let url = bmo_receiver::service_start::hook_url("", "room-title");
  let (status, body) = post(&front, &url, json!({ "point": "room-title", "ask": 7 })).await;

  assert_eq!((status, body["answer"].as_str()), (StatusCode::OK, Some("The room")), "{body}");
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

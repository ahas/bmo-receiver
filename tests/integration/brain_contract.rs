//! A room's record read through the fixed API (`docs/api.md`): the messages, the read marks, the people, the
//! answerers and the readings a turn is held to, every one a resource read in the common wire format.

use axum::http::StatusCode;
use serde_json::json;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{get, post, put, remove, stood};
use crate::integration::common::units::{AGENT, OWNER};

#[tokio::test]
async fn a_rooms_record_answers_the_fields_the_api_names() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  scratch.message(7, 1, OWNER).await;

  // The unread messages, the counts, the read ones and the marks.
  assert_eq!(
    get(&front, &format!("/rooms/7/messages?agentName={AGENT}&unread=true&limit=10")).await,
    (
      StatusCode::OK,
      json!({ "items": [json!({
        "id": 1, "room": 7, "senderId": OWNER, "senderName": "someone", "sentAt": 1_000,
        "text": "words", "imagePath": null, "replyToMessageId": null, "answerSeconds": null,
        "round": null, "album": null, "via": null, "agentName": null, "voice": null, "kind": null, "editedAt": null, "deletedAt": null,
        "pinnedAt": null, "reactions": [], "forwardedFrom": null, "situation": null, "files": [],
      })], "more": false })
    ),
  );
  assert_eq!(
    post(&front, "/read-mark-counts", json!({ "agentName": AGENT, "since": [] })).await.1["items"]
      .as_array()
      .unwrap()
      .iter()
      .map(|room| room["room"].as_i64().unwrap())
      .collect::<Vec<i64>>(),
    [7],
  );
  assert_eq!(
    get(&front, &format!("/rooms/7/messages?agentName={AGENT}&marked=true&after=0&limit=10"))
      .await
      .1["items"]
      .as_array()
      .unwrap()
      .len(),
    0,
  );
  assert_eq!(
    get(&front, &format!("/rooms/7/read-marks?agentName={AGENT}")).await,
    (StatusCode::OK, json!({ "count": 0 })),
  );
  assert_eq!(
    post(&front, "/rooms/7/read-marks", json!({ "agentName": AGENT, "messageIds": [1] })).await,
    (StatusCode::NO_CONTENT, json!({})),
  );
  assert_eq!(
    get(&front, &format!("/rooms/7/read-marks?agentName={AGENT}")).await.1["count"]
      .as_i64()
      .unwrap(),
    1,
  );
  assert_eq!(
    remove(&front, &format!("/rooms/7/read-marks?agentName={AGENT}")).await.1["forgotten"]
      .as_i64()
      .unwrap(),
    1,
  );

  // The rooms, their titles, their answerers and their roads.
  assert_eq!(
    get(&front, "/rooms").await.1["items"]
      .as_array()
      .unwrap()
      .iter()
      .map(|room| room["id"].as_i64().unwrap())
      .collect::<Vec<i64>>(),
    [7],
  );
  assert_eq!(
    get(&front, "/answerers").await.1["items"]
      .as_array()
      .unwrap()
      .iter()
      .map(|room| room["room"].as_i64().unwrap())
      .collect::<Vec<i64>>(),
    [7],
  );
  assert_eq!(
    get(&front, "/rooms/7/roads").await,
    (StatusCode::OK, json!({ "kind": "group", "oneToOne": false })),
  );

  // The people.
  assert_eq!(
    get(&front, &format!("/rooms/7/spoke-between?person={OWNER}&from=0&to=5000")).await,
    (StatusCode::OK, json!({ "spoke": true })),
  );
  assert_eq!(
    get(&front, &format!("/rooms/7/people?ids={OWNER},8")).await.1["items"]
      .as_array()
      .unwrap()
      .len(),
    1,
  );
  assert_eq!(
    get(&front, &format!("/people?ids={OWNER}")).await.1["items"]
      .as_array()
      .unwrap()
      .iter()
      .map(|one| one["name"].as_str().unwrap().to_owned())
      .collect::<Vec<String>>(),
    ["someone"],
  );
  assert_eq!(
    get(&front, &format!("/rooms/7/owner-spoke?person={OWNER}&before=5000&windowMs=10000")).await,
    (StatusCode::OK, json!({ "owner": false, "spoke": false })),
  );
  assert_eq!(
    get(&front, "/rooms/7/messages/1/sender-is-owner").await,
    (StatusCode::OK, json!({ "owner": false })),
  );

  // The agent's own number: none before it has said anything in the room, then the sender its own words there
  // are archived under, whatever road carried them.
  assert_eq!(
    get(&front, &format!("/rooms/7/own-number?agentName={AGENT}")).await,
    (StatusCode::OK, json!({ "number": null })),
  );
  scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via", "agent_name")
         VALUES (7, 2, 9000000000011, 'the agent', 2000, 'Here.', 'record', $1)"#,
      vec![AGENT.into()],
    )
    .await;
  assert_eq!(
    get(&front, &format!("/rooms/7/own-number?agentName={AGENT}")).await,
    (StatusCode::OK, json!({ "number": 9000000000011_i64 })),
  );
  // Whether a text waits for something to happen first, by the receiver's reading of promises: the words it
  // stands for are the policy's, kept as data, so the sentence asked in is read the way they are.
  let waiting = std::fs::read_to_string("tests/fixtures/promises-waiting.txt")
    .unwrap()
    .trim()
    .to_owned();

  assert!(
    post(&front, "/promise-readings", json!({ "text": waiting })).await.1["waits"]
      .as_bool()
      .unwrap(),
  );
  let not_waiting = std::fs::read_to_string("tests/fixtures/promises-not-waiting.txt")
    .unwrap()
    .trim()
    .to_owned();

  assert!(
    !post(&front, "/promise-readings", json!({ "text": not_waiting })).await.1["waits"]
      .as_bool()
      .unwrap(),
  );
  assert_eq!(
    post(&front, "/owner/notices", json!({ "code": "digest-paused" })).await.0,
    StatusCode::CREATED
  );
  assert_eq!(
    post(
      &front,
      "/instructions",
      json!({
        "agentName": AGENT,
        "room": 7,
        "said": { "order": "a memo." },
        "sort": "watch"
      }),
    )
    .await
    .0,
    StatusCode::CREATED,
  );
  assert_eq!(
    put(&front, "/room-holders/unit-a", json!({ "address": "http://unit", "holdings": [] }))
      .await
      .0,
    StatusCode::NO_CONTENT,
  );
}

#[tokio::test]
async fn a_route_reached_wrongly_is_refused() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  // A query that is not the shape the route takes is refused with the words naming it.
  let (status, answer) = get(&front, "/rooms/7/messages?marked=maybe").await;

  assert_eq!(status, StatusCode::BAD_REQUEST);
  assert_eq!(answer["code"].as_str().unwrap(), "bad-request");

  // A method the route does not answer names no route.
  let (method, where_none) = post(&front, "/rooms/7/messages", json!({})).await;

  assert_eq!(method, StatusCode::NOT_FOUND);
  assert_eq!(where_none["code"].as_str().unwrap(), "no-route");
}

//! A signed-in person's acts on a room's message from the console (docs/api.md, 「Messages」): the relay names the
//! caller, the record keeps each act under their number and carries it where the message stands, another
//! person's message is refused to edit or delete but the administrator deletes any, a deleted message takes no act,
//! and the room's pins are listed newest first.

use axum::{
  body::{Body, to_bytes},
  extract::Request,
  http::StatusCode,
};
use bmo_chat_client::{ChatSend, ChatVoice};
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::common::Scratch;
use crate::common::queue::{Front, TOKEN, stood_on};
use crate::common::units::{OWNER, StandInChat};

const ROOM: i64 = -100;
const MEMBER: i64 = 77;

// A request as the console relay sends it, naming the caller by user and role.
async fn asked(
  front: &Front,
  method: &str,
  path: &str,
  body: Option<Value>,
  caller: (&str, &str),
) -> (StatusCode, Value) {
  let mut request = Request::builder()
    .method(method)
    .uri(path)
    .header("authorization", format!("Bearer {TOKEN}"))
    .header("owner-key", caller.0)
    .header("owner-role", caller.1);

  if body.is_some() {
    request = request.header("content-type", "application/json");
  }

  let answer = front
    .routes
    .clone()
    .oneshot(request.body(Body::from(body.map(|body| body.to_string()).unwrap_or_default())).unwrap())
    .await
    .unwrap();
  let status = answer.status();
  let written = to_bytes(answer.into_body(), usize::MAX).await.unwrap();

  (status, serde_json::from_slice(&written).unwrap_or(Value::Null))
}

const OWNERS: (&str, &str) = ("user-owner", "administrator");
const MEMBERS: (&str, &str) = ("user-member", "member");

#[tokio::test]
async fn a_person_acts_on_a_rooms_messages_as_themselves_and_the_record_keeps_and_carries_it() {
  let scratch = Scratch::new().await;
  let road = std::sync::Arc::new(StandInChat::new("road-a").holding(&[ROOM]).owners(OWNER));
  let front = stood_on(&scratch, OWNER, vec![road.clone()]).await;

  front.standing.web.linking(OWNER, OWNERS.0.to_owned());
  front.standing.web.linking(MEMBER, MEMBERS.0.to_owned());

  for (message, sender) in [(1, OWNER), (2, MEMBER)] {
    scratch
      .run(
        r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via")
           VALUES (-100, $1, $2, 'someone', $1, 'words', 'road-a')"#,
        vec![message.into(), sender.into()],
      )
      .await;
  }

  // A reaction of the member's own, kept under his number and on the record alone: no road speaks as him.
  let (status, reacted) = asked(
    &front,
    "PUT",
    "/rooms/-100/messages/1/reactions/mine",
    Some(json!({ "emoji": "👍" })),
    MEMBERS,
  )
  .await;

  assert_eq!(status, StatusCode::OK, "{reacted}");
  assert_eq!(reacted["reactions"], json!([{ "emoji": "👍", "by": MEMBER.to_string() }]));
  assert!(road.sent().is_empty(), "{:?}", road.sent());

  // The owner's reaction goes on to the road where the message stands, in his own voice.
  let (status, reacted) = asked(
    &front,
    "PUT",
    "/rooms/-100/messages/2/reactions/mine",
    Some(json!({ "emoji": "🔥" })),
    OWNERS,
  )
  .await;

  assert_eq!(status, StatusCode::OK, "{reacted}");
  assert!(
    road
      .sent()
      .iter()
      .any(|held| matches!(held.send, ChatSend::Reaction { message_id: 2, .. })
        && held.mode == Some(ChatVoice::HumanReader)),
    "{:?}",
    road.sent()
  );

  // A member acts only as themselves: a reaction with no emoji names nothing to put on.
  let (status, refused) = asked(
    &front,
    "PUT",
    "/rooms/-100/messages/1/reactions/mine",
    Some(json!({ "emoji": " " })),
    MEMBERS,
  )
  .await;

  assert_eq!((status, refused["code"].as_str()), (StatusCode::BAD_REQUEST, Some("bad-request")));

  let (_, unreacted) =
    asked(&front, "DELETE", "/rooms/-100/messages/1/reactions/mine", None, MEMBERS).await;

  assert_eq!(unreacted["reactions"], json!([]));

  // An edit and a delete of another person's message are refused; the administrator deletes any.
  let (status, refused) =
    asked(&front, "PATCH", "/rooms/-100/messages/1", Some(json!({ "text": "mine now" })), MEMBERS).await;

  assert_eq!((status, refused["code"].as_str()), (StatusCode::FORBIDDEN, Some("message-forbidden")));

  let (status, edited) = asked(
    &front,
    "PATCH",
    "/rooms/-100/messages/2",
    Some(json!({ "text": "fixed words" })),
    MEMBERS,
  )
  .await;

  assert_eq!((status, edited["text"].as_str()), (StatusCode::OK, Some("fixed words")));
  assert!(edited["editedAt"].is_i64());

  let (status, refused) = asked(&front, "DELETE", "/rooms/-100/messages/1", None, MEMBERS).await;

  assert_eq!((status, refused["code"].as_str()), (StatusCode::FORBIDDEN, Some("message-forbidden")));

  let (status, deleted) = asked(&front, "DELETE", "/rooms/-100/messages/2", None, OWNERS).await;

  assert_eq!(
    (status, deleted["text"].as_str()),
    (StatusCode::OK, Some("")),
    "the administrator deletes any"
  );
  assert!(deleted["deletedAt"].is_i64());

  // A deleted message takes no act.
  let (status, refused) =
    asked(&front, "PUT", "/rooms/-100/messages/2/pin", Some(json!({ "on": true })), OWNERS).await;

  assert_eq!((status, refused["code"].as_str()), (StatusCode::CONFLICT, Some("message-deleted")));

  // Pins are listed newest pin first.
  scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via")
         VALUES (-100, 3, 77, 'someone', 3, 'more words', 'road-a')"#,
      vec![],
    )
    .await;
  asked(&front, "PUT", "/rooms/-100/messages/1/pin", Some(json!({ "on": true })), MEMBERS).await;
  tokio::time::sleep(std::time::Duration::from_millis(5)).await;
  asked(&front, "PUT", "/rooms/-100/messages/3/pin", Some(json!({ "on": true })), MEMBERS).await;

  let (_, pins) = asked(&front, "GET", "/rooms/-100/pins", None, MEMBERS).await;
  let pinned: Vec<i64> = pins["items"]
    .as_array()
    .unwrap()
    .iter()
    .filter_map(|item| item["id"].as_i64())
    .collect();

  assert_eq!(pinned, [3, 1]);

  // A sticker of the record's set, as the owner; an emoji the set does not hold is refused.
  let (_, set) = asked(&front, "GET", "/stickers", None, MEMBERS).await;
  let first = set["items"][0]["emoji"].as_str().unwrap().to_owned();
  let (status, sent) =
    asked(&front, "POST", "/rooms/-100/stickers", Some(json!({ "emoji": first })), OWNERS).await;

  assert_eq!(
    (status, sent["kind"].as_str(), sent["senderId"].as_i64()),
    (StatusCode::CREATED, Some("sticker"), Some(OWNER))
  );

  let (status, refused) =
    asked(&front, "POST", "/rooms/-100/stickers", Some(json!({ "emoji": "🦖" })), OWNERS).await;

  assert_eq!((status, refused["code"].as_str()), (StatusCode::BAD_REQUEST, Some("unknown-sticker")));

  // A caller the relay names no user for is refused, and an unknown message is 404.
  let (status, refused) =
    asked(&front, "PUT", "/rooms/-100/messages/1/pin", Some(json!({ "on": false })), ("", "")).await;

  assert_eq!((status, refused["code"].as_str()), (StatusCode::FORBIDDEN, Some("no-caller")));

  let (status, refused) =
    asked(&front, "PUT", "/rooms/-100/messages/99/pin", Some(json!({ "on": true })), MEMBERS).await;

  assert_eq!((status, refused["code"].as_str()), (StatusCode::NOT_FOUND, Some("message-unknown")));
}

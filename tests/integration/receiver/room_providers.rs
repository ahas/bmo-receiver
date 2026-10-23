//! A room's chat providers (docs/receiver.md, 「A room's chat providers」; docs/api.md, 「Rooms」): the providers a
//! room may be on, a room's list read off what it holds until it is set whole, every kept message carried to each
//! provider of the room it does not already stand on, and a provider's echo of what was carried to it taken for
//! the message already kept.

use std::sync::Arc;

use axum::{Router, body::Body, extract::Request, http::StatusCode};
use bmo_chat_client::{ChatSend, ChatVoice};
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::common::Scratch;
use crate::common::queue::{Front, TOKEN, get, post, put, read, stood_on};
use crate::common::units::{OWNER, StandInChat};

const ROOM: i64 = -100;

// A front reaching one chat service that holds the room, the room titled so the receiver holds it.
async fn front(scratch: &Scratch) -> (Front, Arc<StandInChat>) {
  let road = Arc::new(StandInChat::new("road-a").holding(&[ROOM]).owners(OWNER));
  let front = stood_on(scratch, OWNER, vec![road.clone()]).await;

  scratch
    .run(r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES (-100, 'Alpha Room')"#, vec![])
    .await;

  (front, road)
}

// The chat services' hand-over routes over the same receiver.
fn handovers(front: &Front) -> Router {
  bmo_receiver::receiver::handover::routes(front.receiver(), TOKEN)
}

// A message handed over on those routes; the status answered.
async fn hand(routes: &Router, message: i64, body: Value) -> StatusCode {
  let asked = Request::put(format!("/chat/rooms/{ROOM}/messages/{message}"))
    .header("authorization", format!("Bearer {TOKEN}"))
    .header("content-type", "application/json")
    .body(Body::from(body.to_string()))
    .unwrap();

  read(routes.clone().oneshot(asked).await.unwrap()).await.0
}

// A message the chat service hands over, said live in the room.
fn handed(text: &str, tag: Option<&str>) -> Value {
  json!({
    "origin": "live",
    "senderId": 55,
    "senderName": "tester",
    "sentAt": 1_760_000_000_000_i64,
    "text": text,
    "replyToMessageId": null,
    "via": "road-a",
    "album": null,
    "mode": "bot-account",
    "context": { "tag": tag },
    "room": { "kind": "group" },
  })
}

#[tokio::test]
async fn the_providers_are_the_record_and_each_chat_service_and_a_rooms_list_is_set_whole() {
  let scratch = Scratch::new().await;
  let (front, _road) = front(&scratch).await;

  let (status, every) = get(&front, "/providers").await;
  let keys: Vec<&str> = every["items"]
    .as_array()
    .unwrap()
    .iter()
    .filter_map(|one| one["key"].as_str())
    .collect();

  assert_eq!(status.as_u16(), 200, "{every}");
  assert_eq!(keys, ["record", "road-a"]);
  assert!(every["items"].as_array().unwrap().iter().all(|one| one["label"].is_string()));

  assert_eq!(
    get(&front, "/rooms/-100/providers").await.1,
    json!({ "providers": ["road-a"], "chosen": false }),
    "a room never set is on the chat service that holds it",
  );

  for refused in [
    json!({ "providers": [] }),
    json!({ "providers": ["road-z"] }),
    json!({ "providers": ["record", "record"] }),
  ] {
    let (status, body) = put(&front, "/rooms/-100/providers", refused.clone()).await;

    assert_eq!((status.as_u16(), body["code"].as_str()), (400, Some("bad-request")), "{refused}");
  }

  assert_eq!(get(&front, "/rooms/-7/providers").await.1["code"], "not-found");
  assert_eq!(
    put(&front, "/rooms/-100/providers", json!({ "providers": ["record"] })).await.1,
    json!({ "providers": ["record"], "chosen": true }),
  );
  assert_eq!(
    get(&front, "/rooms/-100/providers").await.1,
    json!({ "providers": ["record"], "chosen": true })
  );
}

#[tokio::test]
async fn a_kept_message_goes_to_every_provider_of_its_room_but_the_ones_it_stands_on() {
  let scratch = Scratch::new().await;
  let (front, road) = front(&scratch).await;
  let receiver = front.receiver();

  // A line the record keeps goes on to the chat service, tagged and in the service's own voice.
  receiver.speech().kept(ROOM, "kept in the record", None).await.unwrap();

  let sent = road.sent();

  assert_eq!(sent.len(), 1, "{sent:?}");
  assert_eq!(
    sent[0].send,
    ChatSend::Text {
      text: "kept in the record".to_owned(),
      silent: false,
      fold: true
    }
  );
  assert!(sent[0].tag.is_some(), "the send names its delivery");
  assert_eq!(sent[0].mode, None);

  // A message the service handed over already stands there, and the record holds it by being the archive.
  assert_eq!(hand(&handovers(&front), 5, handed("said there", None)).await, StatusCode::CREATED);
  assert_eq!(road.sent().len(), 1, "nothing goes back to the road a message came in on");

  // A copy the road already carried is not carried to it again.
  receiver
    .speech()
    .kept(ROOM, "sent on the road", Some(("road-a", Some(77))))
    .await
    .unwrap();
  assert_eq!(road.sent().len(), 1, "a line the road already carried goes to it no second time");

  // A room on the record alone reaches no chat service.
  put(&front, "/rooms/-100/providers", json!({ "providers": ["record"] })).await;
  receiver.speech().kept(ROOM, "for the console alone", None).await.unwrap();
  assert_eq!(road.sent().len(), 1, "a room on the record alone reaches no chat service");
}

#[tokio::test]
async fn a_providers_echo_of_a_carried_message_is_the_message_already_kept() {
  let scratch = Scratch::new().await;
  let (front, road) = front(&scratch).await;
  let (status, said) =
    post(&front, "/owner/messages", json!({ "chatId": ROOM, "text": "From the console." })).await;

  assert_eq!(status.as_u16(), 200, "{said}");

  let sent = road.sent();

  assert_eq!(sent.len(), 1, "{sent:?}");
  assert_eq!(
    sent[0].mode,
    Some(ChatVoice::HumanReader),
    "the owner's words go in the owner's own voice"
  );

  let tag = sent[0].tag.clone().unwrap();
  let routes = handovers(&front);

  // The service's copy under the number it gave the send, and one naming the send's tag.
  let by_number = hand(&routes, 900, handed("From the console.", None)).await;
  let by_tag = hand(&routes, 901, handed("From the console.", Some(&tag))).await;
  let other = hand(&routes, 902, handed("Another line.", None)).await;

  assert_eq!((by_number, by_tag, other), (StatusCode::OK, StatusCode::OK, StatusCode::CREATED));
  assert_eq!(
    scratch
      .texts(r#"SELECT "message_id"::text FROM "messages" WHERE "chat_id" = -100 ORDER BY "rowid""#)
      .await,
    [said["id"].to_string(), "902".to_owned()],
    "the echoes are the message already kept, and are kept no second time",
  );
  assert!(
    front
      .standing
      .waking
      .woken()
      .iter()
      .all(|woken| !format!("{woken:?}").contains("message_id: 900")
        && !format!("{woken:?}").contains("message_id: 901")),
    "an echo wakes nobody",
  );
}

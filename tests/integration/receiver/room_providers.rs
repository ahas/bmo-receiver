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
  receiver.speech().kept(ROOM, "kept in the record", None, None, None).await.unwrap();

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
    .kept(ROOM, "sent on the road", Some(("road-a", Some(77))), None, None)
    .await
    .unwrap();
  assert_eq!(road.sent().len(), 1, "a line the road already carried goes to it no second time");

  // A room on the record alone reaches no chat service.
  put(&front, "/rooms/-100/providers", json!({ "providers": ["record"] })).await;
  receiver.speech().kept(ROOM, "for the console alone", None, None, None).await.unwrap();
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

// The agent and voice carried on a message, as the row keeps them: the agent whose words these are and the
// voice the send went out in, or `∅|∅` where the row names neither.
async fn voiced(scratch: &Scratch, message: i64) -> String {
  scratch
    .texts(&format!(
      r#"SELECT COALESCE("agent_name", '∅') || '|' || COALESCE("voice", '∅')
         FROM "messages" WHERE "chat_id" = {ROOM} AND "message_id" = {message}"#
    ))
    .await
    .pop()
    .unwrap_or_default()
}

#[tokio::test]
async fn a_send_the_receiver_archives_itself_in_the_human_avatar_voice_names_its_agent_and_voice() {
  let scratch = Scratch::new().await;
  let (front, _road) = front(&scratch).await;
  let receiver = front.receiver();

  let message = receiver
    .speech()
    .kept(ROOM, "answered as the owner", None, Some("agent-a"), Some(ChatVoice::HumanAvatar))
    .await
    .unwrap();

  assert_eq!(
    voiced(&scratch, message).await,
    "agent-a|human-avatar",
    "a send the receiver archives names the agent whose words these are and the voice it went out in",
  );
}

#[tokio::test]
async fn a_message_a_provider_hands_over_names_no_agent_or_voice() {
  let scratch = Scratch::new().await;
  let (front, _road) = front(&scratch).await;

  assert_eq!(hand(&handovers(&front), 5, handed("said in the room", None)).await, StatusCode::CREATED);
  assert_eq!(
    voiced(&scratch, 5).await,
    "∅|∅",
    "a person's message that came in from a provider names neither agent nor voice",
  );
}

#[tokio::test]
async fn a_providers_echo_of_the_receivers_own_send_keeps_the_sends_agent_and_voice() {
  let scratch = Scratch::new().await;
  let (front, _road) = front(&scratch).await;
  let receiver = front.receiver();

  // The receiver archives its own human-avatar send and stands it on the road under the number it went out as.
  let message = receiver
    .speech()
    .kept(
      ROOM,
      "answered as the owner",
      Some(("road-a", Some(77))),
      Some("agent-a"),
      Some(ChatVoice::HumanAvatar),
    )
    .await
    .unwrap();

  // The road hands that send back: it is the message already kept, de-duplicated and kept no second time, so
  // the fields the send was archived with still stand.
  assert_eq!(
    hand(&handovers(&front), 77, handed("answered as the owner", None)).await,
    StatusCode::OK
  );
  assert_eq!(
    voiced(&scratch, message).await,
    "agent-a|human-avatar",
    "the echo is de-duplicated and the send keeps the agent and voice it was archived with",
  );
  assert_eq!(
    scratch
      .texts(r#"SELECT "message_id"::text FROM "messages" WHERE "chat_id" = -100 ORDER BY "rowid""#)
      .await,
    [message.to_string()],
    "the echo is kept no second time",
  );
}

#[tokio::test]
async fn a_chosen_list_rules_both_ways_and_names_the_rooms_own_road() {
  use bmo_receiver::roads::Road;

  let scratch = Scratch::new().await;
  let (front, road) = front(&scratch).await;
  let receiver = front.receiver();
  let routes = handovers(&front);
  let kept = |id: i64| {
    let scratch = &scratch;
    async move {
      scratch
        .texts(&format!(
          r#"SELECT "text" FROM "messages" WHERE "chat_id" = -100 AND "message_id" = {id}"#
        ))
        .await
    }
  };

  // The road detached: the room stands on the record alone.
  put(&front, "/rooms/-100/providers", json!({ "providers": ["record"] })).await;

  let roads = receiver.roads();

  assert_eq!(
    roads.own(ROOM).await,
    Some(Road::Record),
    "the first provider named is the room's own road"
  );
  assert_eq!(
    roads.answering(ROOM, Some("road-a")).await,
    Some(Road::Record),
    "a road off the list answers nothing"
  );
  assert_eq!(roads.answering(ROOM, Some("record")).await, Some(Road::Record));
  assert_eq!(
    roads.unasked(ROOM).await,
    Some(Road::Record),
    "a line nobody asked for stays on the record"
  );

  // What the detached road hands over is not taken, and nothing of the room goes out on it.
  assert_ne!(hand(&routes, 6, handed("said on the detached road", None)).await, StatusCode::CREATED);
  assert!(kept(6).await.is_empty(), "a message a detached road hands over is not kept");

  receiver.speech().say(ROOM, "a notice for the room").await.unwrap();
  assert!(
    road.sent().is_empty(),
    "nothing of the room goes out on a road its list does not name: {:?}",
    road.sent()
  );

  // Attached again, the road is heard and carried to, and it is the room's own road once it is first.
  put(&front, "/rooms/-100/providers", json!({ "providers": ["road-a", "record"] })).await;

  assert_eq!(hand(&routes, 7, handed("said on the road again", None)).await, StatusCode::CREATED);
  assert_eq!(kept(7).await, vec!["said on the road again".to_owned()]);
  assert_eq!(roads.own(ROOM).await, Some(Road::Service("road-a".to_owned())));
}

#[tokio::test]
async fn a_message_the_owner_forwards_is_kept_as_his_in_the_room_it_goes_into_marked_and_carried_there() {
  let scratch = Scratch::new().await;
  let (front, road) = front(&scratch).await;

  // A message said in another room the receiver holds.
  scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via")
         VALUES (-200, 5, 55, 'someone', 1, 'the words to pass on', 'record')"#,
      vec![],
    )
    .await;

  let (status, answer) = post(&front, "/rooms/-200/messages/5/forwards", json!({ "to": ROOM })).await;

  assert_eq!(status, StatusCode::CREATED, "{answer}");

  let copy = answer["id"].as_i64().unwrap();
  let (status, kept) = get(&front, &format!("/rooms/{ROOM}/messages/{copy}")).await;

  assert_eq!(status, StatusCode::OK);
  assert_eq!(kept["text"], "the words to pass on");
  assert_eq!(kept["senderId"], OWNER, "kept as the owner's");
  assert_eq!(kept["forwardedFrom"], json!({ "room": -200, "messageId": 5, "senderName": "someone" }));

  let carried = road.sent();

  assert_eq!(carried.len(), 1, "carried out on the room's own road: {carried:?}");
  assert_eq!(carried[0].mode, Some(ChatVoice::HumanReader), "in the owner's own voice");

  // Refusals: into its own room, a message not held, a room not held.
  assert_eq!(
    post(&front, "/rooms/-200/messages/5/forwards", json!({ "to": -200 })).await.0,
    StatusCode::BAD_REQUEST
  );
  assert_eq!(
    post(&front, "/rooms/-200/messages/9/forwards", json!({ "to": ROOM })).await.0,
    StatusCode::NOT_FOUND
  );
  assert_eq!(
    post(&front, "/rooms/-200/messages/5/forwards", json!({ "to": -999 })).await.0,
    StatusCode::NOT_FOUND
  );
}

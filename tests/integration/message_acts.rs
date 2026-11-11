//! A signed-in person's acts on a room's message from the console (docs/api.md, 「Messages」): the relay names the
//! caller, the record keeps each act under their number and carries it where the message stands, an act a provider
//! holds no copy of or refuses is named as not carried and stands on the record all the same, another person's
//! message is refused to edit or delete but the administrator deletes any, a deleted message takes no act, and the
//! room's pins are listed newest first.

use axum::{
  body::{Body, to_bytes},
  extract::Request,
  http::StatusCode,
};
use bmo_chat_client::{ChatSend, ChatVerb, ChatVoice};
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{Front, TOKEN, stood_on};
use crate::integration::common::units::{OWNER, StandInChat};

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

#[tokio::test]
async fn the_owners_reaction_refused_in_his_own_voice_goes_in_the_rooms_own_unless_the_room_is_his_alone() {
  const READ_ONLY: i64 = -200;

  let scratch = Scratch::new().await;
  let road = std::sync::Arc::new(
    StandInChat::new("road-a")
      .holding(&[ROOM, READ_ONLY])
      .owners(OWNER)
      .room_moding(READ_ONLY, &[ChatVoice::HumanReader])
      .refusing_in(ChatVoice::HumanReader, &[ChatVerb::Reaction]),
  );
  let front = stood_on(&scratch, OWNER, vec![road.clone()]).await;

  front.standing.web.linking(OWNER, OWNERS.0.to_owned());

  for room in [ROOM, READ_ONLY] {
    scratch
      .run(
        r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via")
           VALUES ($1, 5, 77, 'someone', 5, 'words', 'road-a')"#,
        vec![room.into()],
      )
      .await;
  }

  // The reaction and its removal each reach the road in the voice it answers the room by.
  let (status, reacted) = asked(
    &front,
    "PUT",
    "/rooms/-100/messages/5/reactions/mine",
    Some(json!({ "emoji": "🔥" })),
    OWNERS,
  )
  .await;

  assert_eq!(status, StatusCode::OK, "{reacted}");
  assert_eq!(reacted["reactions"], json!([{ "emoji": "🔥", "by": OWNER.to_string() }]));
  assert_eq!(reacted.get("notCarried"), None, "the act went, so the answer names none");

  let (status, unreacted) =
    asked(&front, "DELETE", "/rooms/-100/messages/5/reactions/mine", None, OWNERS).await;

  assert_eq!((status, &unreacted["reactions"]), (StatusCode::OK, &json!([])));

  let carried: Vec<(i64, String, Option<ChatVoice>)> = road
    .sent()
    .into_iter()
    .filter_map(|held| match held.send {
      ChatSend::Reaction { message_id, emoji } => Some((message_id, emoji, held.mode)),
      _ => None,
    })
    .collect();

  assert_eq!(carried, [(5, "🔥".to_owned(), None), (5, String::new(), None)]);

  // A room the road reads in the owner's account alone has no other voice to take the act.
  let (status, _) = asked(
    &front,
    "PUT",
    "/rooms/-200/messages/5/reactions/mine",
    Some(json!({ "emoji": "🔥" })),
    OWNERS,
  )
  .await;

  assert_eq!(status, StatusCode::OK);
  assert_eq!(road.sent().len(), 2, "{:?}", road.sent());
}

// The providers an act's answer names as not carried.
fn named_back(answer: &Value) -> Vec<Value> {
  answer["notCarried"]
    .as_array()
    .unwrap_or_else(|| panic!("the answer names no notCarried: {answer}"))
    .clone()
}

#[tokio::test]
async fn an_act_a_provider_holds_no_copy_of_or_refuses_is_named_as_not_carried_and_still_stands_on_the_record()
 {
  const FORWARDED: i64 = -300;
  const HELD: i64 = 5;
  const NOCOPY: i64 = 6;
  const REFUSED: i64 = 7;
  const SENTENCE: &str = "does not send that kind in this voice";

  let scratch = Scratch::new().await;
  let road = std::sync::Arc::new(StandInChat::new("road-a").holding(&[ROOM]).owners(OWNER));
  // A road that turns a reaction down in the owner's voice and in its own alike: the act cannot go there.
  let refused_road = std::sync::Arc::new(
    StandInChat::new("road-b")
      .holding(&[FORWARDED])
      .refusing_in(ChatVoice::HumanReader, &[ChatVerb::Reaction])
      .refusing_in(ChatVoice::BotAccount, &[ChatVerb::Reaction]),
  );
  let front = stood_on(&scratch, OWNER, vec![road.clone(), refused_road.clone()]).await;

  front.standing.web.linking(OWNER, OWNERS.0.to_owned());

  // The room's line: one message the road holds under its own number, one kept on the record alone that never
  // went out, and one the other road holds and refuses every reaction on.
  for (room, message, via) in
    [(ROOM, HELD, "road-a"), (ROOM, NOCOPY, "record"), (FORWARDED, REFUSED, "road-b")]
  {
    scratch
      .run(
        r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via")
           VALUES ($1, $2, $3, 'someone', $2, 'words', $4)"#,
        vec![room.into(), message.into(), OWNER.into(), via.into()],
      )
      .await;
  }

  // Where the road holds the message, the act goes and the answer names nothing.
  let (status, reacted) = asked(
    &front,
    "PUT",
    "/rooms/-100/messages/5/reactions/mine",
    Some(json!({ "emoji": "👍" })),
    OWNERS,
  )
  .await;

  assert_eq!(status, StatusCode::OK, "{reacted}");
  assert_eq!(reacted["reactions"], json!([{ "emoji": "👍", "by": OWNER.to_string() }]));
  assert_eq!(reacted.get("notCarried"), None, "{reacted}");

  // A message the record holds but the chat service was never given: kept all the same, named as not carried,
  // and nothing asked of the road.
  let (status, reacted) = asked(
    &front,
    "PUT",
    "/rooms/-100/messages/6/reactions/mine",
    Some(json!({ "emoji": "👍" })),
    OWNERS,
  )
  .await;

  assert_eq!(status, StatusCode::OK, "{reacted}");
  assert_eq!(reacted["reactions"], json!([{ "emoji": "👍", "by": OWNER.to_string() }]));
  assert_eq!(
    reacted["notCarried"],
    json!([{ "provider": "road-a", "reason": "no-copy" }]),
    "{reacted}"
  );
  assert!(
    road.sent().iter().all(|held| !matches!(
      held.send,
      ChatSend::Reaction {
        message_id: NOCOPY,
        ..
      }
    )),
    "{:?}",
    road.sent()
  );

  // A road that refuses the act in both voices is named with its own sentence, after both were asked, and the
  // reaction and its removal stand on the record.
  let (status, reacted) = asked(
    &front,
    "PUT",
    "/rooms/-300/messages/7/reactions/mine",
    Some(json!({ "emoji": "👍" })),
    OWNERS,
  )
  .await;

  assert_eq!(status, StatusCode::OK, "{reacted}");
  assert_eq!(
    named_back(&reacted)[0]["provider"],
    json!("road-b"),
    "the road that refused is the one named"
  );
  assert_eq!(named_back(&reacted)[0]["reason"], json!("refused"));
  assert!(
    named_back(&reacted)[0]["detail"]
      .as_str()
      .is_some_and(|detail| detail.contains(SENTENCE)),
    "{reacted}"
  );
  assert_eq!(refused_road.turned_down(), 2, "both the voice named and the room's own were asked");
  assert_eq!(reacted["reactions"], json!([{ "emoji": "👍", "by": OWNER.to_string() }]));

  let (status, unreacted) =
    asked(&front, "DELETE", "/rooms/-300/messages/7/reactions/mine", None, OWNERS).await;

  assert_eq!(status, StatusCode::OK, "{unreacted}");
  assert_eq!(unreacted["reactions"], json!([]));
  assert_eq!(named_back(&unreacted)[0]["provider"], json!("road-b"));
  assert_eq!(named_back(&unreacted)[0]["reason"], json!("refused"));
  assert!(
    named_back(&unreacted)[0]["detail"]
      .as_str()
      .is_some_and(|detail| detail.contains(SENTENCE)),
    "{unreacted}"
  );
  assert_eq!(refused_road.turned_down(), 4, "the removal was turned down in both voices too");
}

//! Typing and presence on the receiving side (docs/receiver.md, 「Typing and presence」): a chat service's mark told in
//! its room and ended by the person's own message, a mark about a room the receiver holds nothing of passed over, a
//! person linked to an account told as that account with its presence gathered from every source, a service that
//! stops being ready leaving nothing standing, and the console's typing told on the stream and handed to the room's
//! chat service in the writer's own voice, refused in another account's private room. Nothing of either is filed.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::{
  Router,
  body::{Body, to_bytes},
  extract::Request,
  http::StatusCode,
};
use bmo_chat_client::{ChatCapability, ChatModeStanding, ChatSend, ChatVoice};
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

use crate::integration::common::Scratch;
use crate::integration::common::units::{OWNER, StandInChat, Standing, standing};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::handover;
use bmo_receiver::routes::routes;

const TOKEN: &str = "the-token";
const ROOM: i64 = -100;
const ROAD: &str = "road-a";
// A person of the chat service linked to no account, one linked to Bea's account, and the owner's account.
const PERSON: i64 = 55;
const LINKED: i64 = 56;
const BEA: i64 = 7;
const OWNERS: i64 = 1;

struct Stood {
  scratch: Scratch,
  _data: TempDir,
  standing: Standing,
  road: Arc<StandInChat>,
  port: Router,
}

// A receiver holding one group room a chat service carries with the agent's own account in it, the service showing
// typing and telling marks and
// presence, its readiness looked at every few milliseconds and no person's link kept, the owner's account linked to
// his sender and Bea's to one of the service's people.
async fn stood() -> Stood {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let road = StandInChat::new(ROAD).holding(&[ROOM]).agent_in(ROOM, 900).canning(&[
    ChatCapability::Typing,
    ChatCapability::PeopleTyping,
    ChatCapability::Presence,
  ]);

  road.moding(ChatVoice::BotAccount, ChatModeStanding::Active);
  road.moding(ChatVoice::HumanReader, ChatModeStanding::Active);

  let road = Arc::new(road);
  let mut standing = standing(scratch.db(), DataRoot::new(data.path()), vec![road.clone()]);

  standing.receiver = standing
    .receiver
    .clone()
    .looking_at_providers_every(Duration::from_millis(5), Duration::ZERO);
  standing.web.linking(OWNER, OWNERS.to_string());
  standing.web.linking(LINKED, BEA.to_string());
  scratch
    .run(
      r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES ($1, 'A group')"#,
      vec![ROOM.into()],
    )
    .await;

  let port =
    routes(standing.receiver.clone(), TOKEN).merge(handover::routes(standing.receiver.clone(), TOKEN));

  Stood {
    scratch,
    _data: data,
    standing,
    road,
    port,
  }
}

async fn asked(port: &Router, method: &str, path: &str, body: Value) -> (StatusCode, Value) {
  let request = Request::builder()
    .method(method)
    .uri(path)
    .header("authorization", format!("Bearer {TOKEN}"))
    .header("content-type", "application/json")
    .body(Body::from(body.to_string()))
    .unwrap();
  let answer = port.clone().oneshot(request).await.unwrap();
  let status = answer.status();
  let written = to_bytes(answer.into_body(), usize::MAX).await.unwrap();

  (status, serde_json::from_slice(&written).unwrap_or(Value::Null))
}

// A mark the chat service tells, answered as taken whatever becomes of it.
async fn marked(stood: &Stood, room: i64, person: i64, on: bool) {
  let mark = json!({ "via": ROAD, "roomId": room, "personId": person, "on": on, "lastsMs": 60_000 });
  let (status, answer) = asked(&stood.port, "POST", "/chat/typing-marks", mark).await;

  assert_eq!(status, StatusCode::CREATED, "{answer}");
}

// A presence change the chat service tells, answered as taken.
async fn changed(stood: &Stood, people: Value, whole: bool) {
  let change = json!({ "via": ROAD, "people": people, "whole": whole });
  let (status, answer) = asked(&stood.port, "POST", "/chat/presence-changes", change).await;

  assert_eq!(status, StatusCode::CREATED, "{answer}");
}

// A message of `person` the chat service hands over, said live in the room.
async fn spoken(stood: &Stood, message: i64, person: i64) {
  let handed = json!({
    "origin": "live",
    "senderId": person,
    "senderName": "tester",
    "sentAt": 1_760_000_000_000_i64,
    "text": "hello",
    "replyToMessageId": null,
    "via": ROAD,
    "album": null,
    "mode": "bot-account",
    "context": {},
    "room": { "kind": "group" },
  });
  let (status, answer) =
    asked(&stood.port, "PUT", &format!("/chat/rooms/{ROOM}/messages/{message}"), handed).await;

  assert!(status.is_success(), "{answer}");
}

fn told(stood: &Stood, kind: &str) -> Vec<Value> {
  stood.standing.heard.events(kind)
}

async fn until(what: &str, holds: impl Fn() -> bool) {
  let waited = tokio::time::timeout(Duration::from_secs(20), async {
    while !holds() {
      tokio::time::sleep(Duration::from_millis(5)).await;
    }
  })
  .await;

  assert!(waited.is_ok(), "{what} never came");
}

async fn rows(stood: &Stood) -> Vec<String> {
  stood
    .scratch
    .texts(r#"SELECT "message_id"::text FROM "messages" ORDER BY "message_id""#)
    .await
}

#[tokio::test]
async fn a_chat_services_mark_is_told_in_its_room_and_ended_by_the_persons_own_message() {
  let stood = stood().await;

  marked(&stood, ROOM, PERSON, true).await;
  marked(&stood, -555, PERSON, true).await;

  let typing = told(&stood, "typing");

  assert_eq!(
    typing.len(),
    1,
    "a mark about a room the receiver holds nothing of is passed over: {typing:?}"
  );
  assert_eq!(
    (
      &typing[0]["chatId"],
      &typing[0]["who"],
      &typing[0]["on"],
      &typing[0]["lastsMs"],
      &typing[0]["id"]
    ),
    (
      &json!(ROOM),
      &json!({ "kind": "person", "via": ROAD, "personId": PERSON }),
      &json!(true),
      &json!(60_000),
      &Value::Null
    ),
  );
  assert!(rows(&stood).await.is_empty(), "a mark files nothing");

  spoken(&stood, 10, PERSON).await;

  let typing = told(&stood, "typing");

  assert_eq!(typing.len(), 2, "{typing:?}");
  assert_eq!((&typing[1]["on"], &typing[1]["who"]["personId"]), (&json!(false), &json!(PERSON)));

  // A mark told off that no longer stands tells nothing.
  marked(&stood, ROOM, PERSON, false).await;

  assert_eq!(told(&stood, "typing").len(), 2);
}

#[tokio::test]
async fn a_person_linked_to_an_account_is_that_account_and_its_presence_is_gathered_from_every_source() {
  let stood = stood().await;

  spoken(&stood, 10, LINKED).await;

  let (status, _) = asked(
    &stood.port,
    "PUT",
    &format!("/accounts/{BEA}/presence"),
    json!({ "state": "away", "lastSeenAt": 300 }),
  )
  .await;

  assert_eq!(status, StatusCode::NO_CONTENT);

  changed(&stood, json!([{ "personId": LINKED, "state": "online", "lastSeenAt": 100 }]), true).await;
  marked(&stood, ROOM, LINKED, true).await;

  let shown: Vec<(Value, Value, Value)> = told(&stood, "presence")
    .iter()
    .map(|event| (event["who"].clone(), event["state"].clone(), event["lastSeenAt"].clone()))
    .collect();
  let bea = json!({ "kind": "account", "account": BEA });

  assert_eq!(
    shown,
    [(bea.clone(), json!("away"), json!(300)), (bea.clone(), json!("online"), json!(300))],
    "online over away, and the latest moment any source gives"
  );
  assert_eq!(told(&stood, "typing")[0]["who"], bea, "the linked person writes as the account");

  // A whole list that leaves the person out takes the service's word back; the console's still stands.
  changed(&stood, json!([]), true).await;
  asked(&stood.port, "PUT", &format!("/accounts/{BEA}/presence"), json!({ "state": null })).await;

  let last: Vec<Value> =
    told(&stood, "presence").iter().skip(2).map(|event| event["state"].clone()).collect();

  assert_eq!(last, [json!("away"), Value::Null], "unknown once no source tells of the account");
}

#[tokio::test]
async fn a_service_that_stops_being_ready_ends_its_marks_and_leaves_its_presence_unknown() {
  let stood = stood().await;

  spoken(&stood, 10, PERSON).await;
  changed(&stood, json!([{ "personId": PERSON, "state": "online", "lastSeenAt": null }]), false).await;
  marked(&stood, ROOM, PERSON, true).await;
  stood.road.unready.store(true, Ordering::SeqCst);

  until("the mark ended", || told(&stood, "typing").iter().any(|event| event["on"] == false)).await;
  until("the presence unknown", || {
    told(&stood, "presence").iter().any(|event| event["state"].is_null())
  })
  .await;

  assert_eq!(told(&stood, "presence")[0]["state"], "online");
}

#[tokio::test]
async fn the_consoles_typing_is_told_and_handed_to_the_room_in_the_writers_own_voice() {
  let stood = stood().await;

  for account in [OWNERS, BEA] {
    let (status, answer) = asked(
      &stood.port,
      "POST",
      &format!("/rooms/{ROOM}/typing"),
      json!({ "account": account, "on": true }),
    )
    .await;

    assert_eq!(status, StatusCode::NO_CONTENT, "{answer}");
  }

  let typing = told(&stood, "typing");

  assert_eq!(typing[0]["who"], json!({ "kind": "account", "account": OWNERS }));
  assert_eq!(typing[0]["lastsMs"], 6_000, "the base's life, renewed as the console tells it again");

  let typed = || {
    stood
      .road
      .sent()
      .into_iter()
      .filter(|send| matches!(send.send, ChatSend::Typing { on: true }))
      .map(|send| (send.mode, send.account_id))
      .collect::<Vec<_>>()
  };

  until("both marks handed to the room", || typed().len() == 2).await;

  let mut handed = typed();

  handed.sort_by_key(|(_, account)| *account);

  assert_eq!(
    handed,
    [(Some(ChatVoice::HumanReader), None), (Some(ChatVoice::BotAccount), Some(BEA))],
    "the owner's in his own voice, another account's in the service's own voice naming it"
  );

  let (status, answer) =
    asked(&stood.port, "POST", "/rooms/-555/typing", json!({ "account": BEA, "on": true })).await;

  assert_eq!((status, &answer["code"]), (StatusCode::NOT_FOUND, &json!("not-found")));

  let (status, answer) = asked(
    &stood.port,
    "POST",
    &format!("/rooms/{OWNER}/typing"),
    json!({ "account": BEA, "on": true }),
  )
  .await;

  assert_eq!(
    (status, &answer["code"]),
    (StatusCode::FORBIDDEN, &json!("private-room")),
    "nothing of another account reaches the owner's private room"
  );
}

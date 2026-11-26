//! A room's registration read and its mode changed (docs/receiver.md, 「Reading and changing a room's mode」;
//! docs/api.md, 「Rooms」): the mode, the providers and the answerers as the receiver holds them, the chat services
//! that read the room in the owner's account alone, the administrator's check, a mode changed with the record and
//! its messages left as they stand, and every reason a change is refused.

use std::sync::Arc;

use bmo_chat_client::{ChatModeStanding, ChatVoice};
use serde_json::json;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{Front, get, post, put, stood_on};
use crate::integration::common::units::{AGENT, OWNER, StandInChat};

const ROOM: i64 = -100;
const ADMIN: &str = "the-admin";

// A front reaching one chat service that holds the rooms and declares the avatar mode as given, the room read in
// the modes it states, and the web unit answering for one administrator.
async fn front(scratch: &Scratch, standing: ChatModeStanding, room_modes: Option<&[ChatVoice]>) -> Front {
  let service = StandInChat::new("road-a").holding(&[ROOM, -200]);
  let service = match room_modes {
    Some(modes) => service.room_moding(ROOM, modes),
    None => service,
  };
  let service = Arc::new(service);

  service.moding(ChatVoice::HumanAvatar, standing);

  let front = stood_on(scratch, OWNER, vec![service]).await;

  front.standing.web.admin(ADMIN.to_owned());

  front
}

// A room added on the service in the mode given.
async fn added(front: &Front, mode: &str) {
  let (status, room) =
    post(front, "/rooms", json!({ "provider": "road-a", "roomId": ROOM, "mode": mode })).await;

  assert_eq!(status.as_u16(), 201, "{room}");
}

// The mode the store keeps for the room, or none.
async fn kept_mode(scratch: &Scratch, room: i64) -> Option<String> {
  scratch
    .texts(&format!(r#"SELECT "mode" FROM "room_modes" WHERE "chat_id" = {room}"#))
    .await
    .pop()
}

// A line of an agent's own, filed in a room's record.
async fn own_line(scratch: &Scratch, room: i64, message: i64, agent: &str) {
  scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "agent_name")
         VALUES ($1, $2, -1, 'agent', 1000, 'an answer', $3)"#,
      vec![room.into(), message.into(), agent.into()],
    )
    .await;
}

fn registration_path(room: i64) -> String {
  format!("/rooms/{room}/registration?by={ADMIN}")
}

#[tokio::test]
async fn a_registration_reads_the_mode_the_providers_the_answerers_and_why_the_room_is_read_only() {
  let scratch = Scratch::new().await;
  let front = front(&scratch, ChatModeStanding::Active, Some(&[ChatVoice::HumanReader])).await;

  added(&front, "record-only").await;

  let (status, registration) = get(&front, &registration_path(ROOM)).await;

  assert_eq!(status.as_u16(), 200, "{registration}");
  assert_eq!(registration["room"].as_i64(), Some(ROOM));
  assert_eq!(registration["mode"], "record-only");
  assert_eq!(registration["providers"], json!(["road-a", "record"]));
  assert_eq!(registration["chosen"], true);
  assert_eq!(registration["answerers"]["default"], AGENT);
  assert_eq!(registration["answerers"]["voice"], AGENT);
  assert_eq!(registration["readOnly"], true);
  assert_eq!(registration["readOnlyBy"], json!(["road-a"]));

  // The agents standing in the room are the ones its own record holds lines of, each once, by name; a person's
  // line names nobody.
  own_line(&scratch, ROOM, 900, "agent-b").await;
  own_line(&scratch, ROOM, 901, "agent-b").await;
  own_line(&scratch, ROOM, 902, AGENT).await;
  scratch.message(ROOM, 903, 77).await;

  let (_, read) = get(&front, &registration_path(ROOM)).await;

  assert_eq!(read["standingAgents"], json!(["agent-a", "agent-b"]));

  // Anyone but the administrator is refused, and a room the receiver does not hold is no registration.
  assert_eq!(
    get(&front, &format!("/rooms/{ROOM}/registration?by=someone-else")).await.1["code"],
    "not-administrator"
  );
  assert_eq!(get(&front, &registration_path(-999)).await.1["code"], "not-found");
}

#[tokio::test]
async fn the_mode_changes_between_record_only_and_avatar_with_the_record_and_the_messages_left_as_they_stand()
 {
  let scratch = Scratch::new().await;
  let front = front(&scratch, ChatModeStanding::Active, None).await;

  added(&front, "record-only").await;
  scratch.message(ROOM, 1, 55).await;

  let added_at = scratch
    .texts(&format!(r#"SELECT "added_at"::text FROM "room_modes" WHERE "chat_id" = {ROOM}"#))
    .await;

  let (status, registration) = put(&front, &registration_path(ROOM), json!({ "mode": "avatar" })).await;

  assert_eq!(status.as_u16(), 200, "{registration}");
  assert_eq!(registration["mode"], "avatar");
  assert_eq!(registration["providers"], json!(["road-a", "record"]));
  assert_eq!(kept_mode(&scratch, ROOM).await.as_deref(), Some("avatar"));

  // The reading agrees, and the room is not read-only: no chat service reads it in the owner's account alone.
  let (_, read) = get(&front, &registration_path(ROOM)).await;

  assert_eq!((read["mode"].clone(), read["readOnly"].clone()), (json!("avatar"), json!(false)));
  assert_eq!(read["readOnlyBy"], json!([]));

  // The registration's other settings and the record stand as they were.
  assert_eq!(
    scratch
      .texts(&format!(r#"SELECT "added_at"::text FROM "room_modes" WHERE "chat_id" = {ROOM}"#))
      .await,
    added_at,
    "the mode was changed in place, not added again"
  );
  assert_eq!(
    scratch
      .texts(&format!(r#"SELECT "provider" FROM "room_providers" WHERE "chat_id" = {ROOM}"#))
      .await,
    ["road-a", "record"],
    "the room's providers were not touched"
  );
  assert_eq!(
    scratch
      .count(&format!(r#"SELECT COUNT(*) FROM "messages" WHERE "chat_id" = {ROOM}"#), vec![])
      .await,
    1,
    "the room's messages were not touched"
  );

  // The same mode asked again writes nothing, and record-only changes it back, asked as the console asks it,
  // naming the administrator's account beside the mode.
  assert_eq!(
    put(&front, &registration_path(ROOM), json!({ "mode": "avatar" })).await.1["mode"],
    "avatar"
  );
  assert_eq!(
    put(&front, &registration_path(ROOM), json!({ "mode": "record-only", "account": 1 }))
      .await
      .1["mode"],
    "record-only"
  );
  assert_eq!(kept_mode(&scratch, ROOM).await.as_deref(), Some("record-only"));

  // A room the receiver holds but never added takes a mode too; one it holds nothing of is no room.
  scratch.message(-200, 1, 55).await;

  let (status, taken) = put(&front, &registration_path(-200), json!({ "mode": "avatar" })).await;

  assert_eq!(status.as_u16(), 200, "{taken}");
  assert_eq!(taken["mode"], "avatar");
  assert_eq!(kept_mode(&scratch, -200).await.as_deref(), Some("avatar"));
  assert_eq!(
    put(&front, &registration_path(-999), json!({ "mode": "avatar" })).await.1["code"],
    "not-found"
  );
  assert_eq!(
    put(&front, &registration_path(ROOM), json!({ "mode": "silent" })).await.1["code"],
    "bad-request"
  );
}

#[tokio::test]
async fn a_room_came_to_by_hearing_takes_a_mode_with_its_record_left_as_it_stands() {
  let scratch = Scratch::new().await;
  let front = front(&scratch, ChatModeStanding::Unsupported, None).await;

  // The receiver knows the room only by hearing its lines: a message of a person's and one of an agent's, and no
  // registration of any kind.
  scratch.message(-200, 1, 55).await;
  own_line(&scratch, -200, 2, AGENT).await;

  let (_, before) = get(&front, &registration_path(-200)).await;

  assert_eq!(before["mode"], json!(null), "no one has set a mode for it yet");
  assert_eq!(before["standingAgents"], json!([AGENT]));
  assert_eq!(before["chosen"], false);

  // What the room's road offers bounds the mode just as it does for an added room, and a refusal writes nothing.
  let (status, refused) = put(&front, &registration_path(-200), json!({ "mode": "avatar" })).await;

  assert_eq!((status.as_u16(), refused["code"].as_str()), (409, Some("avatar-unoffered")));
  assert_eq!(kept_mode(&scratch, -200).await, None, "a refusal wrote no mode row");

  // The mode the road does offer stands, and the room is held by its mode alone: no provider list was chosen for
  // it and its record is the hearing's own.
  assert_eq!(
    put(&front, &registration_path(-200), json!({ "mode": "record-only" })).await.1["mode"],
    "record-only"
  );
  assert_eq!(kept_mode(&scratch, -200).await.as_deref(), Some("record-only"));
  assert_eq!(
    scratch.texts(r#"SELECT "provider" FROM "room_providers" WHERE "chat_id" = -200"#).await,
    Vec::<String>::new(),
    "the mode wrote no provider list"
  );
  assert_eq!(
    scratch.count(r#"SELECT COUNT(*) FROM "messages" WHERE "chat_id" = -200"#, vec![]).await,
    2,
    "the room's record stands as the hearing made it"
  );

  // A mode set from the console is no addition: the room's providers were never set, so a list is chosen for it
  // and its own road is asked again rather than the room being refused as one added.
  let (status, listed) =
    put(&front, "/rooms/-200/providers", json!({ "providers": ["road-a", "record"] })).await;

  assert_eq!(
    (status.as_u16(), listed["chosen"].as_bool()),
    (200, Some(true)),
    "a list set on a room never added is its providers being chosen: {listed}"
  );

  // The room is added whole over the mode it took: its record, its standing agent and the mode it stands in are
  // left as they stand, and the addition's list of providers stands beside them.
  let (status, room) =
    post(&front, "/rooms", json!({ "provider": "road-a", "roomId": -200, "mode": "record-only" })).await;

  assert_eq!((status.as_u16(), room["mode"].as_str()), (201, Some("record-only")), "{room}");
  assert_eq!(
    scratch.count(r#"SELECT COUNT(*) FROM "messages" WHERE "chat_id" = -200"#, vec![]).await,
    2,
    "the addition wrote no line of its own"
  );
  let (_, after) = get(&front, &registration_path(-200)).await;

  assert_eq!(after["providers"], json!(["road-a", "record"]));
  assert_eq!(after["standingAgents"], json!([AGENT]), "the room still carries the agent's own lines");

  // A room whose list an addition set is the one no second addition stands over, and no list is set on it again.
  let (status, refused) =
    post(&front, "/rooms", json!({ "provider": "road-a", "roomId": -200, "mode": "avatar" })).await;

  assert_eq!((status.as_u16(), refused["code"].as_str()), (409, Some("room-added")));
  let (status, refused) = put(&front, "/rooms/-200/providers", json!({ "providers": ["record"] })).await;

  assert_eq!((status.as_u16(), refused["code"].as_str()), (409, Some("providers-fixed")));
}

#[tokio::test]
async fn a_room_that_took_a_mode_from_the_console_is_added_in_the_mode_the_addition_asks() {
  let scratch = Scratch::new().await;
  let front = front(&scratch, ChatModeStanding::Active, None).await;

  scratch.message(-200, 1, 55).await;

  assert_eq!(
    put(&front, &registration_path(-200), json!({ "mode": "record-only" })).await.1["mode"],
    "record-only"
  );

  // The addition asks for the mode it wants and the room takes it: the mode set from the console is not a
  // registration that holds the room in the mode it was set to.
  let (status, room) =
    post(&front, "/rooms", json!({ "provider": "road-a", "roomId": -200, "mode": "avatar" })).await;

  assert_eq!((status.as_u16(), room["mode"].as_str()), (201, Some("avatar")), "{room}");
  assert_eq!(kept_mode(&scratch, -200).await.as_deref(), Some("avatar"));
  assert_eq!(
    scratch
      .texts(r#"SELECT "provider" FROM "room_providers" WHERE "chat_id" = -200 ORDER BY "place""#)
      .await,
    vec!["road-a".to_owned(), "record".to_owned()],
  );
}

#[tokio::test]
async fn an_avatar_is_refused_where_the_service_does_not_offer_it() {
  let scratch = Scratch::new().await;
  let front = front(&scratch, ChatModeStanding::Unsupported, None).await;

  added(&front, "record-only").await;

  let (status, refused) = put(&front, &registration_path(ROOM), json!({ "mode": "avatar" })).await;

  assert_eq!((status.as_u16(), refused["code"].as_str()), (409, Some("avatar-unoffered")));
  assert_eq!(
    kept_mode(&scratch, ROOM).await.as_deref(),
    Some("record-only"),
    "a refusal wrote nothing"
  );
}

#[tokio::test]
async fn an_avatar_is_refused_where_the_room_is_never_answered_in() {
  let scratch = Scratch::new().await;
  let front = front(&scratch, ChatModeStanding::Active, Some(&[ChatVoice::HumanReader])).await;

  added(&front, "record-only").await;

  let (status, refused) = put(&front, &registration_path(ROOM), json!({ "mode": "avatar" })).await;

  assert_eq!((status.as_u16(), refused["code"].as_str()), (409, Some("room-takes-no-avatar")));
  assert_eq!(
    kept_mode(&scratch, ROOM).await.as_deref(),
    Some("record-only"),
    "a refusal wrote nothing"
  );
}

#[tokio::test]
async fn an_avatar_is_refused_where_no_chat_service_stands_in_the_room() {
  let scratch = Scratch::new().await;
  let front = front(&scratch, ChatModeStanding::Active, None).await;

  added(&front, "record-only").await;
  scratch.message(ROOM, 1, 55).await;
  // An added room's list is fixed on the routes, so the store sets it here.
  bmo_receiver::roads::providers::choose(scratch.db(), ROOM, &["record".to_owned()])
    .await
    .unwrap();

  let (status, refused) = put(&front, &registration_path(ROOM), json!({ "mode": "avatar" })).await;

  assert_eq!((status.as_u16(), refused["code"].as_str()), (409, Some("no-chat-service")));
  assert_eq!(
    kept_mode(&scratch, ROOM).await.as_deref(),
    Some("record-only"),
    "a refusal wrote nothing"
  );

  // The user alone is still a mode the room may take.
  assert_eq!(
    put(&front, &registration_path(ROOM), json!({ "mode": "record-only" })).await.1["mode"],
    "record-only"
  );
}

#[tokio::test]
async fn only_the_administrator_changes_a_rooms_mode() {
  let scratch = Scratch::new().await;
  let front = front(&scratch, ChatModeStanding::Active, None).await;

  added(&front, "record-only").await;

  let (status, refused) = put(
    &front,
    &format!("/rooms/{ROOM}/registration?by=someone-else"),
    json!({ "mode": "avatar" }),
  )
  .await;

  assert_eq!((status.as_u16(), refused["code"].as_str()), (403, Some("not-administrator")));
  assert_eq!(kept_mode(&scratch, ROOM).await.as_deref(), Some("record-only"));
}

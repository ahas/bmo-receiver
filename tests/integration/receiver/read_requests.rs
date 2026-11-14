//! A read-only room's modes and its read request (docs/receiver.md, 「A read-only room」 and 「Adding a room」): a
//! room's stated modes decide whether its service reads it in the owner's account alone before the marker of no
//! identity does; the receiver asks a service to read the people's words of a room only where the room was added
//! on that service and is still on its list, whatever its mode; and a read-only room takes no mode that answers.

use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::Request;
use bmo_chat_client::{ChatCapability, ChatModeStanding, ChatVoice};
use bmo_receiver::receiver::handover::routes;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{Front, post, put, stood_on};
use crate::integration::common::units::{OWNER, StandInChat};

/// A room the service says it reads through the owner's account alone, though an account of its own is there.
const READER_ROOM: i64 = -180;
/// A room the service's own account is in, which every voice it has takes.
const BOT_ROOM: i64 = -181;
/// A room that states no modes and holds no identity of the service's own: read-only by the marker.
const MARKED_ROOM: i64 = -182;
/// A room that states the service's own voice beside the owner's, though no identity of its own stands there.
const SPOKEN_ROOM: i64 = -183;
/// The number the service's own account carries.
const BOT_NUMBER: i64 = 7;
/// The bearer the protocol's receiving-side routes take here.
const CHAT_TOKEN: &str = "the-chat-token";

// A service that lists its rooms and reads through the owner's account beside an account of its own and the
// avatar, and a second that holds the reader room too.
async fn front(scratch: &Scratch) -> (Front, Arc<StandInChat>) {
  let every = [ChatVoice::BotAccount, ChatVoice::HumanReader, ChatVoice::HumanAvatar];
  let road = Arc::new(
    StandInChat::new("road-a")
      .holding(&[READER_ROOM, BOT_ROOM, MARKED_ROOM, SPOKEN_ROOM])
      .agent_in(READER_ROOM, BOT_NUMBER)
      .agent_in(BOT_ROOM, BOT_NUMBER)
      .room_moding(READER_ROOM, &[ChatVoice::HumanReader])
      .room_moding(BOT_ROOM, &every)
      .room_moding(SPOKEN_ROOM, &[ChatVoice::BotAccount, ChatVoice::HumanReader])
      .titling(READER_ROOM, "Reader Room")
      .titling(BOT_ROOM, "Bot Room")
      .canning(&[ChatCapability::RoomList]),
  );
  let other = Arc::new(StandInChat::new("road-b").holding(&[READER_ROOM]));

  for voice in every {
    road.moding(voice, ChatModeStanding::Active);
  }

  other.moding(ChatVoice::BotAccount, ChatModeStanding::Active);

  let front = stood_on(scratch, OWNER, vec![road.clone(), other]).await;

  (front, road)
}

fn adding(room: i64, mode: &str) -> Value {
  json!({ "provider": "road-a", "roomId": room, "mode": mode })
}

// Whether the receiver asks the service under `via` to read the room, as the protocol's receiving side answers
// it: its `read`, and false where it holds nothing of the room.
async fn read(front: &Front, room: i64, via: &str) -> bool {
  let chat = routes(front.standing.receiver.clone(), CHAT_TOKEN);
  let asked = Request::get(format!("/chat/rooms/{room}?via={via}"))
    .header("authorization", format!("Bearer {CHAT_TOKEN}"))
    .body(Body::empty())
    .unwrap();
  let answer = chat.oneshot(asked).await.unwrap();
  let status = answer.status();
  let body: Value =
    serde_json::from_slice(&to_bytes(answer.into_body(), usize::MAX).await.unwrap()).unwrap_or(Value::Null);

  match status.as_u16() {
    200 => body["read"].as_bool().expect("the room carries read"),
    404 => false,
    _ => panic!("the room was not answered: {status} {body}"),
  }
}

#[tokio::test]
async fn a_room_s_stated_modes_decide_whether_it_is_read_in_the_owner_s_account_alone_before_its_identity()
{
  let scratch = Scratch::new().await;
  let (front, _road) = front(&scratch).await;
  let roads = front.standing.receiver.roads();

  assert!(
    roads.reads_only("road-a", READER_ROOM).await,
    "human-reader alone, whatever the identity"
  );
  assert!(!roads.reads_only("road-a", BOT_ROOM).await);
  assert!(
    !roads.reads_only("road-a", SPOKEN_ROOM).await,
    "the service's own voice is among its modes"
  );
  assert!(
    roads.reads_only("road-a", MARKED_ROOM).await,
    "no modes stated, and no identity of its own"
  );
}

#[tokio::test]
async fn a_room_is_asked_to_be_read_only_once_added_on_the_asking_service_and_while_on_its_list() {
  let scratch = Scratch::new().await;
  let (front, _road) = front(&scratch).await;

  assert!(!read(&front, READER_ROOM, "road-a").await, "a room never added is not asked for");

  let (status, added) = post(&front, "/rooms", adding(READER_ROOM, "record-only")).await;

  assert_eq!(status.as_u16(), 201, "{added}");
  assert!(read(&front, READER_ROOM, "road-a").await);
  assert!(!read(&front, READER_ROOM, "road-b").await, "another service was not asked");

  let (status, added) = post(&front, "/rooms", adding(BOT_ROOM, "record-only")).await;

  assert_eq!(status.as_u16(), 201, "{added}");
  assert!(
    read(&front, BOT_ROOM, "road-a").await,
    "an added room is asked for though the service answers in it"
  );

  // Taken off the service's list, the room is no longer the service's to read.
  let (status, chosen) =
    put(&front, &format!("/rooms/{READER_ROOM}/providers"), json!({ "providers": ["record"] })).await;

  assert_eq!(status.as_u16(), 200, "{chosen}");
  assert!(!read(&front, READER_ROOM, "road-a").await);
}

#[tokio::test]
async fn a_read_only_room_takes_no_mode_that_answers_though_its_service_offers_one() {
  let scratch = Scratch::new().await;
  let (front, _road) = front(&scratch).await;

  let (status, refused) = post(&front, "/rooms", adding(READER_ROOM, "avatar")).await;

  assert_eq!(
    (status.as_u16(), refused["code"].as_str()),
    (409, Some("avatar-unsupported")),
    "{refused}"
  );
  assert_eq!(post(&front, "/rooms", adding(BOT_ROOM, "avatar")).await.0.as_u16(), 201);
}

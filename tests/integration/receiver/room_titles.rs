//! A room's title (docs/receiver.md, 「A room's title」): the title the room's chat service answers for it, asked
//! afresh, replaces the kept one with no message handed again; a room its service gives no title keeps its own;
//! a title equal to the kept one writes nothing; and a room on the record alone takes the title a caller gives it,
//! where a carried room, an empty title, a room nobody holds and another account's private room are refused.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::integration::common::Scratch;
use crate::integration::common::queue::{Front, get, post, put, stood_on};
use crate::integration::common::units::{OWNER, StandInChat};

const ROOM: i64 = -100;

// A front reaching one chat service that holds the room, the room kept under `kept`.
async fn front(scratch: &Scratch, road: StandInChat, kept: &str) -> (Front, Arc<StandInChat>) {
  let road = Arc::new(road.holding(&[ROOM]).owners(OWNER));
  let front = stood_on(scratch, OWNER, vec![road.clone()]).await;

  scratch
    .run(
      r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES ($1, $2)"#,
      vec![ROOM.into(), kept.into()],
    )
    .await;

  (front, road)
}

// The title the room list shows for the room.
async fn listed_title(front: &Front) -> Value {
  let (status, listed) = get(front, "/rooms").await;

  assert_eq!(status, StatusCode::OK);

  listed["items"]
    .as_array()
    .unwrap()
    .iter()
    .find(|room| room["id"] == ROOM)
    .map(|room| room["title"].clone())
    .unwrap()
}

// The row version the kept title stands at, which moves on every write of the row.
async fn title_version(scratch: &Scratch) -> Vec<String> {
  scratch
    .texts(&format!(r#"SELECT "xmin"::text FROM "chat_titles" WHERE "chat_id" = {ROOM}"#))
    .await
}

#[tokio::test]
async fn a_title_the_service_answers_anew_replaces_the_kept_one_on_the_first_read() {
  let scratch = Scratch::new().await;
  let (front, _road) =
    front(&scratch, StandInChat::new("road-a").titling(ROOM, "Chat with myself"), "Old name").await;

  assert_eq!(listed_title(&front).await, "Chat with myself");

  let (status, room) = get(&front, &format!("/rooms/{ROOM}")).await;

  assert_eq!(status, StatusCode::OK);
  assert_eq!(room["title"], "Chat with myself");
  assert_eq!(
    scratch
      .texts(&format!(r#"SELECT "title" FROM "chat_titles" WHERE "chat_id" = {ROOM}"#))
      .await,
    ["Chat with myself"]
  );
}

#[tokio::test]
async fn a_room_its_service_gives_no_title_keeps_its_own() {
  let scratch = Scratch::new().await;
  let (front, road) = front(&scratch, StandInChat::new("road-a"), "Own name").await;

  let (status, room) = get(&front, &format!("/rooms/{ROOM}")).await;

  assert_eq!(status, StatusCode::OK);
  assert!(road.rooms_asked.load(Ordering::SeqCst) > 0);
  assert_eq!(room["title"], "Own name");
}

#[tokio::test]
async fn a_title_equal_to_the_kept_one_writes_nothing() {
  let scratch = Scratch::new().await;
  let (front, road) =
    front(&scratch, StandInChat::new("road-a").titling(ROOM, "Same name"), "Same name").await;
  let before = title_version(&scratch).await;

  let (status, room) = get(&front, &format!("/rooms/{ROOM}")).await;

  assert_eq!(status, StatusCode::OK);
  assert!(road.rooms_asked.load(Ordering::SeqCst) > 0);
  assert_eq!(room["title"], "Same name");
  assert_eq!(title_version(&scratch).await, before);
}

#[tokio::test]
async fn a_room_on_the_record_alone_takes_a_new_title_and_a_carried_one_keeps_its_services() {
  let scratch = Scratch::new().await;
  let service = Arc::new(StandInChat::new("road-a").holding(&[ROOM]));
  let front = stood_on(&scratch, OWNER, vec![service]).await;

  let (status, added) = post(
    &front,
    "/supervisors",
    json!({ "name": "agent-d", "label": "Agent D", "room": "member-d", "ownerAccount": 7 }),
  )
  .await;

  assert_eq!(status.as_u16(), 201, "{added}");

  let room = added["room"]["id"].as_i64().unwrap();
  let path = format!("/rooms/{room}/title");

  // The supervisor's own room takes the title, trimmed, and answers it.
  let (status, retitled) = put(&front, &path, json!({ "title": "  Person D " })).await;

  assert_eq!(status.as_u16(), 200, "{retitled}");
  assert_eq!(retitled, json!({ "title": "Person D" }));
  assert_eq!(get(&front, &path).await.1, json!({ "title": "Person D" }));

  // Another account's write into the private room, and an empty title, leave it as it stands.
  let (status, refused) =
    put(&front, &format!("{path}?account=8"), json!({ "title": "Someone else" })).await;

  assert_eq!((status.as_u16(), refused["code"].as_str()), (403, Some("private-room")), "{refused}");

  let (status, refused) = put(&front, &path, json!({ "title": "  " })).await;

  assert_eq!((status.as_u16(), refused["code"].as_str()), (400, Some("bad-request")), "{refused}");
  assert_eq!(get(&front, &path).await.1, json!({ "title": "Person D" }));

  // A room a chat service carries keeps its service's title, and a room nobody holds is not found.
  let (status, carried) =
    post(&front, "/rooms", json!({ "provider": "road-a", "roomId": ROOM, "mode": "record-only" })).await;

  assert_eq!(status.as_u16(), 201, "{carried}");

  let (status, refused) = put(&front, &format!("/rooms/{ROOM}/title"), json!({ "title": "Renamed" })).await;

  assert_eq!(
    (status.as_u16(), refused["code"].as_str()),
    (409, Some("titled-by-service")),
    "{refused}"
  );

  let (status, refused) = put(&front, "/rooms/-999/title", json!({ "title": "Renamed" })).await;

  assert_eq!((status.as_u16(), refused["code"].as_str()), (404, Some("not-found")), "{refused}");
}

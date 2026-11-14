//! A room's title (docs/receiver.md, 「A room's title」): the title the room's chat service answers for it, asked
//! afresh, replaces the kept one with no message handed again; a room its service gives no title keeps its own;
//! and a title equal to the kept one writes nothing.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use axum::http::StatusCode;
use serde_json::Value;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{Front, get, stood_on};
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

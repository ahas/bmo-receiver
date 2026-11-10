//! A room archived and locked (docs/receiver.md, 「An archived room」; docs/api.md, 「Rooms」): the administrator
//! locks it and opens it again, anyone else is refused, and while it is locked words the console types into it
//! are refused `room-archived` with nothing kept.

use std::sync::Arc;

use serde_json::json;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{Front, get, post, put, stood_on};
use crate::integration::common::units::{OWNER, StandInChat};

const ROOM: i64 = -100;
const ADMIN: &str = "the-admin";

// A front reaching one chat service that holds the room, the web unit answering for one administrator.
async fn front(scratch: &Scratch) -> Front {
  let service = Arc::new(StandInChat::new("road-a").holding(&[ROOM]));
  let front = stood_on(scratch, OWNER, vec![service]).await;

  front.standing.web.admin(ADMIN.to_owned());

  let (status, room) =
    post(&front, "/rooms", json!({ "provider": "road-a", "roomId": ROOM, "mode": "record-only" })).await;

  assert_eq!(status.as_u16(), 201, "{room}");
  scratch
    .run(
      &format!(r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES ({ROOM}, 'A room')"#),
      vec![],
    )
    .await;

  front
}

// The messages the store keeps for the room.
async fn kept(scratch: &Scratch) -> usize {
  scratch
    .texts(&format!(r#"SELECT "text" FROM "messages" WHERE "chat_id" = {ROOM}"#))
    .await
    .len()
}

#[tokio::test]
async fn the_administrator_locks_a_room_and_opens_it_again_and_a_locked_room_takes_no_words() {
  let scratch = Scratch::new().await;
  let front = front(&scratch).await;
  let path = format!("/rooms/{ROOM}/archive");

  let (status, open) = get(&front, &path).await;

  assert_eq!(status.as_u16(), 200, "{open}");
  assert_eq!(open, json!({ "room": ROOM, "archivedAt": null, "archivedBy": null }));

  // Anyone but the administrator or the room's leader is refused, and nothing is locked.
  let (status, refused) =
    put(&front, &format!("{path}?by=someone-else"), json!({ "archived": true })).await;

  assert_eq!((status.as_u16(), refused["code"].as_str()), (403, Some("not-room-leader")), "{refused}");
  assert_eq!(get(&front, &path).await.1["archivedAt"], json!(null));

  let (status, locked) = put(&front, &format!("{path}?by={ADMIN}"), json!({ "archived": true })).await;

  assert_eq!(status.as_u16(), 200, "{locked}");
  assert!(locked["archivedAt"].is_i64(), "{locked}");
  assert_eq!(locked["archivedBy"], ADMIN);

  // Words typed into the locked room are refused before anything is kept.
  let before = kept(&scratch).await;
  let (status, refused) = post(&front, "/owner/messages", json!({ "chatId": ROOM, "text": "Hello" })).await;

  assert_eq!((status.as_u16(), refused["code"].as_str()), (409, Some("room-archived")), "{refused}");
  assert_eq!(kept(&scratch).await, before);

  // Opened again, the room stands as it did and takes words.
  let (status, opened) = put(&front, &format!("{path}?by={ADMIN}"), json!({ "archived": false })).await;

  assert_eq!(status.as_u16(), 200, "{opened}");
  assert_eq!(opened["archivedAt"], json!(null));

  let (status, said) = post(&front, "/owner/messages", json!({ "chatId": ROOM, "text": "Hello" })).await;

  assert_eq!(status.as_u16(), 200, "{said}");
}

#[tokio::test]
async fn a_room_the_receiver_does_not_hold_is_not_archived() {
  let scratch = Scratch::new().await;
  let front = front(&scratch).await;
  let (status, refused) =
    put(&front, &format!("/rooms/-999/archive?by={ADMIN}"), json!({ "archived": true })).await;

  assert_eq!((status.as_u16(), refused["code"].as_str()), (404, Some("not-found")), "{refused}");
}

//! The console's pinned order and the chat agent's window (`docs/boundary-api.md`, 「The pinned order」 and
//! 「Reading a room」): the order set whole, a room added and taken one at a time, a room the record does not hold
//! refused with nothing written, and the window read as seconds left and the moment it closes.

use axum::http::StatusCode;
use serde_json::json;

use bmo_receiver::now_ms;

use crate::common::Scratch;
use crate::common::queue::{Front, get, post, put, stood};
use crate::common::units::{AGENT, OWNER};

// Rooms the record holds: two by their messages, one by its title alone.
const SPOKEN: i64 = -501;
const ALSO_SPOKEN: i64 = -502;
const TITLED: i64 = -503;
// A room the record does not hold.
const UNKNOWN: i64 = -599;

async fn rooms() -> (Scratch, Front) {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  scratch.message(SPOKEN, 1, OWNER).await;
  scratch.message(ALSO_SPOKEN, 1, OWNER).await;
  scratch
    .run(
      r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES ($1, 'A titled room')"#,
      vec![TITLED.into()],
    )
    .await;

  (scratch, front)
}

// The pinned order as the room list stands it.
async fn listed_first(front: &Front) -> Vec<(i64, bool)> {
  get(front, "/console/rooms").await.1["rooms"]
    .as_array()
    .unwrap()
    .iter()
    .map(|room| (room["chatId"].as_i64().unwrap(), room["pinned"].as_bool().unwrap()))
    .collect()
}

#[tokio::test]
async fn the_order_is_set_whole_and_the_room_list_stands_it_first() {
  let (_scratch, front) = rooms().await;

  assert_eq!(
    put(&front, "/console/pinned-rooms", json!({ "rooms": [TITLED, SPOKEN, TITLED] })).await,
    (StatusCode::OK, json!({ "rooms": [TITLED, SPOKEN] })),
    "a room named twice stands once, at its first place",
  );
  assert_eq!(listed_first(&front).await, [(TITLED, true), (SPOKEN, true), (ALSO_SPOKEN, false)]);

  assert_eq!(
    put(&front, "/console/pinned-rooms", json!({ "rooms": [] })).await,
    (StatusCode::OK, json!({ "rooms": [] })),
  );
  assert!(listed_first(&front).await.iter().all(|(_, pinned)| !pinned));
}

#[tokio::test]
async fn a_room_is_added_last_once_and_taken_out_without_refusing_one_not_there() {
  let (_scratch, front) = rooms().await;

  assert_eq!(
    post(&front, "/console/pinned-rooms/add", json!({ "chat": ALSO_SPOKEN })).await,
    (StatusCode::OK, json!({ "rooms": [ALSO_SPOKEN] })),
  );
  assert_eq!(
    post(&front, "/console/pinned-rooms/add", json!({ "chat": SPOKEN })).await,
    (StatusCode::OK, json!({ "rooms": [ALSO_SPOKEN, SPOKEN] })),
  );
  assert_eq!(
    post(&front, "/console/pinned-rooms/add", json!({ "chat": ALSO_SPOKEN })).await,
    (StatusCode::OK, json!({ "rooms": [ALSO_SPOKEN, SPOKEN] })),
    "a room standing already keeps its place",
  );
  assert_eq!(
    post(&front, "/console/pinned-rooms/remove", json!({ "chat": ALSO_SPOKEN })).await,
    (StatusCode::OK, json!({ "rooms": [SPOKEN] })),
  );
  assert_eq!(
    post(&front, "/console/pinned-rooms/remove", json!({ "chat": TITLED })).await,
    (StatusCode::OK, json!({ "rooms": [SPOKEN] })),
  );
  assert_eq!(
    post(&front, "/console/pinned-rooms/remove", json!({ "chat": UNKNOWN })).await,
    (StatusCode::OK, json!({ "rooms": [SPOKEN] })),
    "a number the record no longer holds can still be taken out",
  );
}

#[tokio::test]
async fn a_room_the_record_does_not_hold_is_refused_and_nothing_is_written() {
  let (scratch, front) = rooms().await;

  assert_eq!(
    post(&front, "/console/pinned-rooms/add", json!({ "chat": SPOKEN })).await.0,
    StatusCode::OK
  );

  for (status, refused) in [
    put(&front, "/console/pinned-rooms", json!({ "rooms": [TITLED, UNKNOWN] })).await,
    post(&front, "/console/pinned-rooms/add", json!({ "chat": UNKNOWN })).await,
  ] {
    assert_eq!(status, StatusCode::NOT_FOUND, "{refused}");
    assert_eq!(refused["code"], "not-found");
  }

  assert_eq!(
    scratch
      .texts(r#"SELECT "value" FROM "settings" WHERE "key" = 'console_pinned_rooms'"#)
      .await,
    [format!("[{SPOKEN}]")],
  );

  for (status, refused) in [
    put(&front, "/console/pinned-rooms", json!({ "rooms": "first" })).await,
    post(&front, "/console/pinned-rooms/add", json!({})).await,
    post(&front, "/console/pinned-rooms/remove", json!({ "chat": "a room" })).await,
  ] {
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
  }
}

#[tokio::test]
async fn the_chat_agents_window_is_read_as_seconds_left_and_when_it_closes() {
  let (scratch, front) = rooms().await;
  let now = now_ms() / 1000;
  let window = |chat: i64, agent: &str, until: i64| {
    (
      r#"INSERT INTO "room_windows" ("chat_id", "agent", "open_until") VALUES ($1, $2, $3)"#,
      vec![chat.into(), agent.to_owned().into(), until.into()],
    )
  };

  for (sql, values) in [
    window(SPOKEN, AGENT, now + 600),
    window(ALSO_SPOKEN, AGENT, now - 60),
    window(TITLED, "agent-b", now + 600),
  ] {
    scratch.run(sql, values).await;
  }

  let (status, open) = get(&front, &format!("/console/rooms/{SPOKEN}/window")).await;

  assert_eq!(status, StatusCode::OK);
  assert_eq!(open["closesAt"], now + 600);
  assert!((595..=600).contains(&open["openFor"].as_i64().unwrap()), "{open}");

  assert_eq!(
    get(&front, &format!("/console/rooms/{ALSO_SPOKEN}/window")).await,
    (StatusCode::OK, json!({ "openFor": 0, "closesAt": now - 60 })),
  );
  assert_eq!(
    get(&front, &format!("/console/rooms/{TITLED}/window")).await,
    (StatusCode::OK, json!({ "openFor": 0, "closesAt": null })),
    "another agent's window is not the chat agent's",
  );
}

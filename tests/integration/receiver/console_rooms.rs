//! A room's line and the pinned order (`docs/api.md`, 「Rooms」): the order set whole, a room added and taken one
//! at a time, a room the record does not hold refused with nothing written, and the default answerer's window read in
//! milliseconds and the moment it closes.

use axum::http::StatusCode;
use serde_json::json;

use bmo_receiver::now_ms;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{Front, get, post, put, remove, stood};
use crate::integration::common::units::{AGENT, OWNER};

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
  get(front, "/rooms").await.1["items"]
    .as_array()
    .unwrap()
    .iter()
    .map(|room| (room["id"].as_i64().unwrap(), room["pinned"].as_bool().unwrap()))
    .collect()
}

#[tokio::test]
async fn the_order_is_set_whole_and_the_room_list_stands_it_first() {
  let (_scratch, front) = rooms().await;

  assert_eq!(
    put(&front, "/pinned-rooms", json!({ "roomIds": [TITLED, SPOKEN, TITLED] })).await,
    (StatusCode::OK, json!({ "roomIds": [TITLED, SPOKEN] })),
    "a room named twice stands once, at its first place",
  );
  assert_eq!(listed_first(&front).await, [(TITLED, true), (SPOKEN, true), (ALSO_SPOKEN, false)]);

  assert_eq!(
    put(&front, "/pinned-rooms", json!({ "roomIds": [] })).await,
    (StatusCode::OK, json!({ "roomIds": [] })),
  );
  assert!(listed_first(&front).await.iter().all(|(_, pinned)| !pinned));
}

#[tokio::test]
async fn a_room_is_added_last_once_and_taken_out_without_refusing_one_not_there() {
  let (_scratch, front) = rooms().await;

  assert_eq!(
    post(&front, "/pinned-rooms", json!({ "room": ALSO_SPOKEN })).await,
    (StatusCode::OK, json!({ "roomIds": [ALSO_SPOKEN] })),
  );
  assert_eq!(
    post(&front, "/pinned-rooms", json!({ "room": SPOKEN })).await,
    (StatusCode::OK, json!({ "roomIds": [ALSO_SPOKEN, SPOKEN] })),
  );
  assert_eq!(
    post(&front, "/pinned-rooms", json!({ "room": ALSO_SPOKEN })).await,
    (StatusCode::OK, json!({ "roomIds": [ALSO_SPOKEN, SPOKEN] })),
    "a room standing already keeps its place",
  );
  assert_eq!(
    remove(&front, &format!("/pinned-rooms/{ALSO_SPOKEN}")).await,
    (StatusCode::OK, json!({ "roomIds": [SPOKEN] })),
  );
  assert_eq!(
    remove(&front, &format!("/pinned-rooms/{TITLED}")).await,
    (StatusCode::OK, json!({ "roomIds": [SPOKEN] })),
  );
  assert_eq!(
    remove(&front, &format!("/pinned-rooms/{UNKNOWN}")).await,
    (StatusCode::OK, json!({ "roomIds": [SPOKEN] })),
    "a number the record no longer holds can still be taken out",
  );
}

#[tokio::test]
async fn a_room_the_record_does_not_hold_is_refused_and_nothing_is_written() {
  let (scratch, front) = rooms().await;

  assert_eq!(post(&front, "/pinned-rooms", json!({ "room": SPOKEN })).await.0, StatusCode::OK);

  for (status, refused) in [
    put(&front, "/pinned-rooms", json!({ "roomIds": [TITLED, UNKNOWN] })).await,
    post(&front, "/pinned-rooms", json!({ "room": UNKNOWN })).await,
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
    put(&front, "/pinned-rooms", json!({ "roomIds": "first" })).await,
    post(&front, "/pinned-rooms", json!({})).await,
    remove(&front, "/pinned-rooms/not-a-room").await,
  ] {
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
  }
}

#[tokio::test]
async fn the_chat_agents_window_is_read_in_milliseconds_and_when_it_closes() {
  let (scratch, front) = rooms().await;
  let now = now_ms();
  let window = |chat: i64, agent: &str, until: i64| {
    (
      r#"INSERT INTO "room_windows" ("chat_id", "agent", "open_until") VALUES ($1, $2, $3)"#,
      vec![chat.into(), agent.to_owned().into(), until.into()],
    )
  };

  for (sql, values) in [
    window(SPOKEN, AGENT, now + 600_000),
    window(ALSO_SPOKEN, AGENT, now - 60_000),
    window(TITLED, "agent-b", now + 600_000),
  ] {
    scratch.run(sql, values).await;
  }

  let (status, open) = get(&front, &format!("/rooms/{SPOKEN}/window")).await;

  assert_eq!(status, StatusCode::OK);
  assert_eq!(open["closesAt"], now + 600_000);
  assert!((595_000..=600_000).contains(&open["openForMs"].as_i64().unwrap()), "{open}");

  assert_eq!(
    get(&front, &format!("/rooms/{ALSO_SPOKEN}/window")).await,
    (StatusCode::OK, json!({ "openForMs": 0, "closesAt": now - 60_000 })),
  );
  assert_eq!(
    get(&front, &format!("/rooms/{TITLED}/window")).await,
    (StatusCode::OK, json!({ "openForMs": 0, "closesAt": null })),
    "another agent's window is not the default answerer's",
  );
}

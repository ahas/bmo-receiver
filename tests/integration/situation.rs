//! The room's situation note as the console reads it (docs/drafting.md, 「The situation note」): the note stands on
//! each message the round that wrote it judged, with whether that round changed it, and a room answers the note as it
//! stands with when it last changed.

use axum::http::StatusCode;
use serde_json::json;

use crate::common::Scratch;
use crate::common::queue::{get, stood};

const ROOM: i64 = -5_200;
const OTHER: i64 = -5_201;

#[tokio::test]
async fn a_room_answers_its_note_and_when_it_last_changed() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, 1).await;

  let (status, none) = get(&front, &format!("/rooms/{ROOM}/situation")).await;

  assert_eq!(status, StatusCode::OK, "{none}");
  assert_eq!(none, json!({ "text": null, "updatedAt": null }));

  scratch
    .run(
      r#"INSERT INTO "room_situations" ("chat_id", "text", "updated_at") VALUES ($1, $2, $3)"#,
      vec![ROOM.into(), "Mina and Joon await the file.".into(), 1_700_000_000_000i64.into()],
    )
    .await;

  let (status, held) = get(&front, &format!("/rooms/{ROOM}/situation")).await;

  assert_eq!(status, StatusCode::OK, "{held}");
  assert_eq!(
    held,
    json!({ "text": "Mina and Joon await the file.", "updatedAt": 1_700_000_000_000i64 })
  );
  // Another room has no note of its own.
  let (_, other) = get(&front, &format!("/rooms/{OTHER}/situation")).await;

  assert!(other["text"].is_null(), "{other}");
}

#[tokio::test]
async fn a_message_carries_the_note_as_the_round_that_judged_it_read_it() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, 1).await;

  scratch.message(ROOM, 5, 1).await;
  scratch.message(ROOM, 7, 1).await;
  scratch
    .run(
      r#"UPDATE "messages" SET "situation" = $1, "situation_changed" = $2 WHERE "chat_id" = $3 AND "message_id" = $4"#,
      vec!["They wait on you.".into(), true.into(), ROOM.into(), 5i64.into()],
    )
    .await;

  let (status, held) = get(&front, &format!("/rooms/{ROOM}/messages/5")).await;

  assert_eq!(status, StatusCode::OK, "{held}");
  assert_eq!(held["situation"], json!({ "text": "They wait on you.", "changed": true }));

  // A message no round has judged under a note carries none.
  let (status, plain) = get(&front, &format!("/rooms/{ROOM}/messages/7")).await;

  assert_eq!(status, StatusCode::OK, "{plain}");
  assert!(plain["situation"].is_null(), "{plain}");
}

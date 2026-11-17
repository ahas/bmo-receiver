//! The owner's words from the console to a room's session (docs/boundary-api.md, 「The owner's words」): queued
//! for the room's session as a quiet instruction, a room nobody has seen refused, and the owner's own chat
//! counting before it has said anything.

use std::sync::Arc;

use axum::{
  Router,
  body::{Body, to_bytes},
  extract::Request,
  http::StatusCode,
};
use serde_json::{Value, json};
use tempfile::tempdir;
use tower::ServiceExt;

use crate::integration::common::Scratch;
use crate::integration::common::queue::as_console;
use crate::integration::common::units::{OWNER, StandInChat, standing};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::routes::routes;

const TOKEN: &str = "the-token";
const ROOM: i64 = -100;

// A receiver over a scratch database, reaching one road that holds the room.
async fn port() -> (Scratch, Router) {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let road = Arc::new(StandInChat::new("road-a").holding(&[ROOM]).owners(OWNER));
  let standing = standing(scratch.db(), DataRoot::new(data.path()), [road.clone()].to_vec());

  (scratch, routes(standing.receiver.clone(), as_console(TOKEN)))
}

fn post(path: &str, body: Value) -> Request {
  Request::post(path)
    .header("authorization", format!("Bearer {TOKEN}"))
    .header("content-type", "application/json")
    .body(Body::from(body.to_string()))
    .unwrap()
}

async fn asked(port: &Router, path: &str, body: Value) -> (StatusCode, Value) {
  let answer = port.clone().oneshot(post(path, body)).await.unwrap();
  let status = answer.status();
  let written = to_bytes(answer.into_body(), usize::MAX).await.unwrap();

  (status, serde_json::from_slice(&written).unwrap_or(Value::Null))
}

#[tokio::test]
async fn the_owners_console_words_are_queued_for_a_rooms_session_and_a_room_nobody_has_seen_is_refused() {
  let (scratch, port) = port().await;

  scratch
    .run(r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES (-100, 'Alpha Room')"#, vec![])
    .await;

  assert_eq!(
    asked(&port, "/owner/instructions", json!({ "chatId": ROOM, "text": "  Later, please.  " })).await,
    (StatusCode::NO_CONTENT, json!({})),
  );
  // The owner's own chat counts before it has said anything.
  assert_eq!(
    asked(
      &port,
      "/owner/instructions",
      json!({ "chatId": OWNER, "text": "Words for the owner's own chat" })
    )
    .await
    .0,
    StatusCode::NO_CONTENT
  );
  assert_eq!(
    asked(&port, "/owner/instructions", json!({ "chatId": -7, "text": "Hello" })).await.0,
    StatusCode::NOT_FOUND
  );
  assert_eq!(
    asked(&port, "/owner/instructions", json!({ "chatId": ROOM, "text": " " })).await.0,
    StatusCode::BAD_REQUEST
  );
  assert_eq!(
    asked(&port, "/owner/instructions", json!({ "chatId": "-100", "text": "Hello" }))
      .await
      .0,
    StatusCode::BAD_REQUEST
  );

  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "instructions" WHERE "chat_id" = -100 AND "sort" = 'console_talk' AND "summary" = 'Later, please.'"#,
        vec![],
      )
      .await,
    1,
  );
  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "instructions" WHERE "chat_id" = $1 AND "summary" = 'Words for the owner''s own chat'"#,
        vec![OWNER.into()],
      )
      .await,
    1,
  );
}

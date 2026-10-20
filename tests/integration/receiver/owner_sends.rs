//! What the console sends in the owner's own name (docs/boundary-api.md, 「The owner's words」; docs/receiver.md,
//! 「The owner's words」): a line typed into his private chat kept under a console number and handed to that
//! chat's session, the same line typed into another room carried on that room's road, files put in with the words
//! gathered into the albums the road takes with the caption on the first of them, and a send asked of the
//! receiver carried as its asker.

use std::sync::Arc;

use axum::{
  Router,
  body::{Body, to_bytes},
  extract::Request,
  http::StatusCode,
};
use bmo_chat_client::{AlbumItem, ChatSend, FileItem};
use serde_json::{Value, json};
use tempfile::tempdir;
use tower::ServiceExt;

use crate::common::Scratch;
use crate::common::units::{ADDRESS, AGENT, OWNER, SENDER, StandInChat, registration, standing};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::routes::routes;

const TOKEN: &str = "the-token";
const ROOM: i64 = -100;

// A receiver over a scratch database, reaching one road that holds the room and the owner's chat.
async fn stood(rooms: &[i64]) -> (Scratch, Router, Arc<StandInChat>) {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let road = Arc::new(StandInChat::new("road-a").holding(rooms).owners(OWNER));
  let standing = standing(scratch.db(), DataRoot::new(data.path()), [road.clone()].to_vec());

  let mut agent = registration(AGENT, SENDER, [ADDRESS].to_vec(), None, true);

  agent.row.insert("role".to_owned(), serde_json::json!("chat"));
  standing.agency.registering([agent].to_vec());

  (scratch, routes(standing.receiver.clone(), TOKEN), road)
}

fn post(path: &str, token: &str, body: String) -> Request {
  Request::post(path)
    .header("authorization", format!("Bearer {token}"))
    .body(Body::from(body))
    .unwrap()
}

// A form posted to a route, with the boundary the body is cut by.
fn posted(path: &str, token: &str, body: String) -> Request {
  Request::post(path)
    .header("authorization", format!("Bearer {token}"))
    .header("content-type", "multipart/form-data; boundary=own")
    .body(Body::from(body))
    .unwrap()
}

async fn form_asked(port: &Router, path: &str, body: String) -> (StatusCode, Value) {
  let answer = port.clone().oneshot(posted(path, TOKEN, body)).await.unwrap();
  let status = answer.status();
  let written = to_bytes(answer.into_body(), usize::MAX).await.unwrap();

  (status, serde_json::from_slice(&written).unwrap_or(Value::Null))
}

async fn asked(port: &Router, path: &str, token: &str, body: String) -> (StatusCode, Value) {
  let answer = port.clone().oneshot(post(path, token, body)).await.unwrap();
  let status = answer.status();
  let written = to_bytes(answer.into_body(), usize::MAX).await.unwrap();

  (status, serde_json::from_slice(&written).unwrap_or(Value::Null))
}

// A JSON body posted to a route.
fn json_post(body: Value) -> String {
  body.to_string()
}

// A form body of the test's own: each field named, and each file with its type, its name and its bytes.
fn form(fields: &[(String, String)], files: &[(String, String, String)]) -> String {
  let parts = fields.iter().map(|(name, value)| format!("--own\r\ncontent-disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"))
    .chain(files.iter().map(|(mime, name, bytes)| format!("--own\r\ncontent-type: {mime}\r\ncontent-disposition: form-data; name=\"file\"; filename=\"{name}\"\r\n\r\n{bytes}\r\n")));

  parts.collect::<Vec<String>>().join("") + "--own--\r\n"
}

#[tokio::test]
async fn a_line_the_owner_typed_in_his_own_chat_is_kept_under_a_console_number_and_handed_to_his_session() {
  let (scratch, port, road) = stood(&[ROOM]).await;
  let (status, answer) = asked(
    &port,
    "/owner/messages",
    TOKEN,
    json_post(json!({ "chatId": OWNER, "text": "  Later, please.  "})),
  )
  .await;

  assert_eq!(status, StatusCode::OK);
  assert!(answer["id"].is_i64(), "{answer:?}");
  assert!(road.sent().is_empty(), "his own chat keeps the words, nothing goes to a road");
  assert_eq!(
    scratch
      .texts(r#"SELECT "text" FROM "messages" WHERE "chat_id" = 42 AND "via" = 'record'"#)
      .await,
    ["Later, please.".to_owned()],
  );
}

#[tokio::test]
async fn a_line_the_owner_typed_in_another_room_goes_on_that_rooms_road_and_an_unknown_room_is_refused() {
  let (scratch, port, road) = stood(&[ROOM]).await;

  scratch
    .run(r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES (-100, 'Alpha Room')"#, vec![])
    .await;

  assert_eq!(
    asked(
      &port,
      "/owner/messages",
      TOKEN,
      json_post(json!({ "chatId": ROOM, "text": "The game begins."}))
    )
    .await
    .0,
    StatusCode::OK
  );
  assert_eq!(
    road.sent().iter().map(|held| held.send.clone()).collect::<Vec<ChatSend>>(),
    [ChatSend::Text {
      text: "The game begins.".to_owned(),
      silent: false,
      fold: true
    }]
    .to_vec(),
  );
  assert_eq!(
    asked(&port, "/owner/messages", TOKEN, json_post(json!({ "chatId": -7, "text": "Hello"})))
      .await
      .0,
    StatusCode::NOT_FOUND
  );
  assert_eq!(
    asked(&port, "/owner/messages", TOKEN, json_post(json!({ "chatId": ROOM, "text": " "})))
      .await
      .0,
    StatusCode::BAD_REQUEST
  );
}

#[tokio::test]
async fn a_reply_to_a_message_the_room_does_not_hold_is_refused_and_one_it_holds_answers_the_first_part() {
  let (scratch, port, road) = stood(&[ROOM]).await;

  scratch
    .run(r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES (-100, 'Alpha Room')"#, vec![])
    .await;
  scratch.message(ROOM, 7, 55).await;

  assert_eq!(
    asked(
      &port,
      "/owner/messages",
      TOKEN,
      json_post(json!({ "chatId": ROOM, "text": "words", "replyTo": 999}))
    )
    .await
    .0,
    StatusCode::CONFLICT
  );
  assert_eq!(
    asked(
      &port,
      "/owner/messages",
      TOKEN,
      json_post(json!({ "chatId": ROOM, "text": "words", "replyTo": 7}))
    )
    .await
    .0,
    StatusCode::OK
  );
  assert_eq!(
    road.sent().iter().map(|held| held.send.clone()).collect::<Vec<ChatSend>>(),
    [ChatSend::Text {
      text: "words".to_owned(),
      silent: false,
      fold: true
    }]
    .to_vec(),
  );
  assert_eq!(road.sent()[0].reply_to_message_id, Some(7));
}

#[tokio::test]
async fn files_the_owner_put_in_a_room_go_as_the_albums_the_road_takes_with_the_caption_on_the_first() {
  let (scratch, port, road) = stood(&[ROOM]).await;

  scratch
    .run(r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES (-100, 'Alpha Room')"#, vec![])
    .await;

  let body = form(
    &[
      ("chatId".to_owned(), "-100".to_owned()),
      ("text".to_owned(), "Two pictures\nand a paper".to_owned()),
    ],
    &[
      ("image/jpeg".to_owned(), "one.jpg".to_owned(), "jpeg".to_owned()),
      ("image/png".to_owned(), "two.png".to_owned(), "png".to_owned()),
      ("application/pdf".to_owned(), "paper.pdf".to_owned(), "pdf".to_owned()),
    ],
  );
  let (status, answer) = form_asked(&port, "/owner/message-files", body).await;

  assert_eq!(status, StatusCode::OK, "{answer:?}");
  assert_eq!(answer["sent"].as_array().map(Vec::len), Some(3), "{answer:?}");
  assert!(answer["failed"].as_array().unwrap().is_empty(), "{answer:?}");
  assert_eq!(
    road.sent().iter().map(|held| held.send.clone()).collect::<Vec<ChatSend>>(),
    [
      ChatSend::Album {
        items: [
          AlbumItem::File(FileItem {
            name: "one.jpg".to_owned(),
            mime: "image/jpeg".to_owned(),
            bytes: b"jpeg".to_vec()
          }),
          AlbumItem::File(FileItem {
            name: "two.png".to_owned(),
            mime: "image/png".to_owned(),
            bytes: b"png".to_vec()
          })
        ]
        .to_vec(),
        caption: Some("Two pictures\nand a paper".to_owned()),
      },
      ChatSend::Document {
        bytes: b"pdf".to_vec(),
        file_name: "paper.pdf".to_owned(),
      },
    ]
    .to_vec(),
  );
}

#[tokio::test]
async fn a_send_asked_of_the_receiver_is_carried_as_its_asker_and_one_no_agent_asks_for_a_reminder_is_refused()
 {
  let (scratch, port, _road) = stood(&[ROOM]).await;

  scratch
    .run(r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES (-100, 'Alpha Room')"#, vec![])
    .await;

  let (status, answer) = asked(
    &port,
    "/sends",
    TOKEN,
    json_post(json!({
      "asker": { "room": ROOM, "agentName": AGENT, "by": format!("chat:{ROOM}") },
      "tool": "pin",
      "input": { "chat": ROOM, "message": 7, "unpin": false },
    })),
  )
  .await;

  assert_eq!(status, StatusCode::OK);
  assert_eq!(answer["lines"].as_array().unwrap().len(), 1, "{answer:?}");
  assert_eq!(answer["refusal"], Value::Null, "{answer:?}");

  let (status, answer) = asked(
    &port,
    "/sends",
    TOKEN,
    json_post(json!({
      "asker": json!({ "room": ROOM }),
      "tool": "remind",
      "input": { "reason": "the check is due" },
    })),
  )
  .await;

  assert_eq!(status, StatusCode::OK);
  assert!(answer["refusal"].as_str().unwrap().contains("outside any agent's turn"), "{answer:?}");

  let (status, answer) = asked(
    &port,
    "/sends",
    TOKEN,
    json_post(json!({
      "asker": json!({ "room": ROOM, "agentName": AGENT }),
      "tool": "remind",
      "input": { "reason": "the check is due", "every": "daily 07:00" },
    })),
  )
  .await;

  assert_eq!(status, StatusCode::OK);
  assert_eq!(answer["refusal"], Value::Null, "{answer:?}");
}

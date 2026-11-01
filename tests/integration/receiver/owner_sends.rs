//! What the console sends in the owner's own name (docs/receiver.md, 「The owner's words」): a line typed into any
//! room kept under a console number and the name the room's own service shows him by, else his registered name,
//! handed to the room's session and carried to the room's chat service in his own voice, files put in with the
//! words carried as the albums the road takes with the caption on the first of them, and a send asked of the
//! receiver carried as its asker.

use std::sync::Arc;

use axum::{
  Router,
  body::{Body, to_bytes},
  extract::Request,
  http::StatusCode,
};
use bmo_chat_client::{AlbumItem, ChatSend, ChatVoice, FileItem};
use serde_json::{Value, json};
use tempfile::tempdir;
use tower::ServiceExt;

use crate::common::Scratch;
use crate::common::units::{ADDRESS, AGENT, OWNER, SENDER, StandInChat, registration, standing};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::routes::routes;
use receiver_policy::roads::ROADS;

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
  assert_eq!(
    road
      .sent()
      .iter()
      .map(|held| (held.send.clone(), held.mode))
      .collect::<Vec<(ChatSend, Option<ChatVoice>)>>(),
    [(
      ChatSend::Text {
        text: "Later, please.".to_owned(),
        silent: false,
        fold: false
      },
      Some(ChatVoice::HumanReader)
    )],
    "his chat keeps the words, and the service that holds it is handed them in his own voice",
  );
  assert_eq!(
    scratch
      .texts(r#"SELECT "text" FROM "messages" WHERE "chat_id" = 42 AND "via" = 'record'"#)
      .await,
    ["Later, please.".to_owned()],
  );
  assert_eq!(
    scratch
      .texts(r#"SELECT "sender_name" FROM "messages" WHERE "chat_id" = 42 AND "via" = 'record'"#)
      .await,
    [OWNER.to_string()],
    "a chat whose service never showed the owner files him under his number, never the agent's name",
  );
}

#[tokio::test]
async fn the_owners_console_line_stands_under_his_registered_name_where_no_service_showed_him() {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let road = Arc::new(StandInChat::new("road-a").holding(&[ROOM]).owners(OWNER));
  let standing = standing(scratch.db(), DataRoot::new(data.path()), [road].to_vec());

  standing.web.users.lock().unwrap().insert(OWNER, "owner-user".to_owned());

  let port = routes(standing.receiver.clone(), TOKEN);
  let (status, answer) =
    asked(&port, "/owner/messages", TOKEN, json_post(json!({ "chatId": OWNER, "text": "Hello." }))).await;

  assert_eq!(status, StatusCode::OK, "{answer:?}");
  assert_eq!(
    scratch
      .texts(&format!(
        r#"SELECT "sender_name" FROM "messages" WHERE "chat_id" = 42 AND "message_id" = {}"#,
        answer["id"]
      ))
      .await,
    ["owner-user".to_owned()],
  );
}

#[tokio::test]
async fn the_owners_console_line_stands_under_the_name_his_chats_own_service_last_showed_him_by() {
  let (scratch, port, _road) = stood(&[ROOM]).await;

  for (message, name, via, at) in [
    (1, "Owner Then", "road-a", 1_000),
    (2, "Owner Now", "road-a", 2_000),
    (-50, SENDER, "record", 3_000),
  ] {
    scratch
      .run(
        r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via")
           VALUES ($1, $2, $1, $3, $4, 'words', $5)"#,
        vec![OWNER.into(), i64::from(message).into(), name.into(), i64::from(at).into(), via.into()],
      )
      .await;
  }

  let (status, answer) = asked(
    &port,
    "/owner/messages",
    TOKEN,
    json_post(json!({ "chatId": OWNER, "text": "Now, then." })),
  )
  .await;

  assert_eq!(status, StatusCode::OK, "{answer:?}");
  assert_eq!(
    scratch
      .texts(&format!(
        r#"SELECT "sender_name" FROM "messages" WHERE "chat_id" = 42 AND "message_id" = {}"#,
        answer["id"]
      ))
      .await,
    ["Owner Now".to_owned()],
    "the console's own rows, the agent's name among them, are never read for it",
  );
}

#[tokio::test]
async fn a_line_the_owner_typed_in_another_room_is_kept_as_his_and_carried_to_its_road_and_an_unknown_room_is_refused()
 {
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
  assert!(
    road.sent().iter().all(|held| held.mode == Some(ChatVoice::HumanReader)),
    "the owner's words go in the owner's own voice"
  );
  assert_eq!(
    scratch
      .texts(r#"SELECT "sender_name" FROM "messages" WHERE "chat_id" = -100 AND "via" = 'record'"#)
      .await,
    [OWNER.to_string()],
    "the room keeps the words as the owner's",
  );
  assert_eq!(
    road.sent().iter().map(|held| held.send.clone()).collect::<Vec<ChatSend>>(),
    [ChatSend::Text {
      text: "The game begins.".to_owned(),
      silent: false,
      fold: false
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
  scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via")
         VALUES (-100, 7, 55, 'someone', 1000, 'words', 'road-a')"#,
      vec![],
    )
    .await;

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
      fold: false
    }]
    .to_vec(),
  );
  assert_eq!(
    road.sent()[0].reply_to_message_id,
    Some(7),
    "the message answered stands on the road as itself"
  );
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
  assert!(road.sent().iter().all(|held| held.mode == Some(ChatVoice::HumanReader)));
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
async fn a_voice_note_the_owner_recorded_is_kept_as_a_sound_and_carried_as_a_voice_message() {
  let (scratch, port, road) = stood(&[ROOM]).await;

  scratch
    .run(r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES (-100, 'Alpha Room')"#, vec![])
    .await;

  let body = form(
    &[
      ("chatId".to_owned(), "-100".to_owned()),
      ("text".to_owned(), "The news".to_owned()),
      ("voice".to_owned(), "true".to_owned()),
    ],
    &[("audio/ogg".to_owned(), "voice.ogg".to_owned(), "OggSvoice".to_owned())],
  );
  let (status, answer) = form_asked(&port, "/owner/message-files", body).await;

  assert_eq!(status, StatusCode::OK, "{answer:?}");
  assert_eq!(answer["sent"].as_array().map(Vec::len), Some(1), "{answer:?}");
  assert!(answer["failed"].as_array().unwrap().is_empty(), "{answer:?}");
  assert_eq!(
    road.sent().iter().map(|held| held.send.clone()).collect::<Vec<ChatSend>>(),
    [ChatSend::Voice {
      audio: b"OggSvoice".to_vec(),
      said: "The news".to_owned(),
      caption: Some("The news".to_owned()),
      transcript: None,
    }]
    .to_vec(),
  );
  assert!(road.sent().iter().all(|held| held.mode == Some(ChatVoice::HumanReader)));

  let message_id = answer["sent"][0]["id"].as_i64().unwrap();

  assert_eq!(
    scratch
      .texts(&format!(
        r#"SELECT "text" FROM "messages" WHERE "chat_id" = -100 AND "message_id" = {message_id}"#
      ))
      .await,
    [format!("{} The news", ROADS.marks.voice)],
    "the room keeps the voice word and the caption beside it",
  );
  assert_eq!(
    scratch
      .texts(&format!(
        r#"SELECT "kind" FROM "collected_items" WHERE "chat_id" = -100 AND "message_id" = {message_id}"#
      ))
      .await,
    ["audio".to_owned()],
    "the recording itself stands in the room's files as a sound",
  );
}

#[tokio::test]
async fn a_voice_note_with_more_than_one_file_or_no_sound_is_refused() {
  let (_scratch, port, _road) = stood(&[ROOM]).await;

  let two = form(
    &[("chatId".to_owned(), OWNER.to_string()), ("voice".to_owned(), "true".to_owned())],
    &[
      ("audio/ogg".to_owned(), "one.ogg".to_owned(), "OggSone".to_owned()),
      ("audio/ogg".to_owned(), "two.ogg".to_owned(), "OggStwo".to_owned()),
    ],
  );

  assert_eq!(form_asked(&port, "/owner/message-files", two).await.0, StatusCode::BAD_REQUEST);

  let paper = form(
    &[("chatId".to_owned(), OWNER.to_string()), ("voice".to_owned(), "true".to_owned())],
    &[("application/pdf".to_owned(), "paper.pdf".to_owned(), "pdf".to_owned())],
  );

  assert_eq!(form_asked(&port, "/owner/message-files", paper).await.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_send_asked_of_the_receiver_is_carried_as_its_asker() {
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
}

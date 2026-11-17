//! What the console sends in the owner's own name (docs/receiver.md, 「The owner's words」): a line typed into any
//! room kept under a console number and the name the room's own service shows him by, else his registered name,
//! handed to the room's session and carried to the room's chat service in his own voice, files put in with the
//! words carried as the albums the road takes with the caption on the first of them, words and files the room kept
//! answered as kept and carried though the agency fails after, a file of any size kept and carried whole where no
//! provider of the room declares a limit and one past a declared limit refused with it, a send asked of the
//! receiver carried as its asker, and the owner's words taken under the console relay's credential alone, never
//! under the receiver's own token (docs/api.md, 「Auth」).

use std::path::Path;
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

use crate::integration::common::Scratch;
use crate::integration::common::queue::{RECEIVER_TOKEN, as_console};
use crate::integration::common::units::{
  ADDRESS, AGENT, OWNER, SENDER, StandInChat, Standing, registration, standing, woken,
};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::routes::routes;
use receiver_policy::roads::ROADS;

const TOKEN: &str = "the-token";
const ROOM: i64 = -100;

// A receiver over a scratch database, reaching one road that holds the room and the owner's chat.
async fn stood(rooms: &[i64]) -> (Scratch, Router, Arc<StandInChat>) {
  let (scratch, standing, road) = standing_over(rooms).await;

  (scratch, routes(standing.receiver.clone(), as_console(TOKEN)), road)
}

// What such a receiver stands on, for a test that sets more of it before the routes are made.
async fn standing_over(rooms: &[i64]) -> (Scratch, Standing, Arc<StandInChat>) {
  let data = tempdir().unwrap();

  standing_on(StandInChat::new("road-a").holding(rooms).owners(OWNER), data.path()).await
}

// A receiver over a scratch database and the data root `data`, reaching `road`.
async fn standing_on(road: StandInChat, data: &Path) -> (Scratch, Standing, Arc<StandInChat>) {
  let scratch = Scratch::new().await;
  let road = Arc::new(road);
  let standing = standing(scratch.db(), DataRoot::new(data), [road.clone()].to_vec());

  let mut agent = registration(AGENT, SENDER, [ADDRESS].to_vec(), None);

  agent.row.insert("role".to_owned(), serde_json::json!("chat"));
  standing.agency.registering([agent].to_vec());

  (scratch, standing, road)
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

async fn read(port: &Router, path: &str) -> (StatusCode, Value) {
  let request = Request::get(path)
    .header("authorization", format!("Bearer {TOKEN}"))
    .body(Body::empty())
    .unwrap();
  let answer = port.clone().oneshot(request).await.unwrap();
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
async fn the_owners_words_under_the_receivers_own_token_are_refused_and_the_console_relays_are_filed() {
  let (scratch, port, road) = stood(&[ROOM]).await;
  let words = json_post(json!({ "chatId": OWNER, "text": "Spoken as the owner." }));
  let kept = r#"SELECT "text" FROM "messages" WHERE "chat_id" = 42 AND "via" = 'record'"#;

  let (status, answer) = asked(&port, "/owner/messages", RECEIVER_TOKEN, words.clone()).await;

  assert_eq!(
    (status, answer["code"].clone()),
    (StatusCode::FORBIDDEN, json!("forbidden")),
    "{answer:?}"
  );
  assert!(scratch.texts(kept).await.is_empty(), "nothing is filed under the receiver's own token");
  assert!(road.sent().is_empty(), "nothing is carried under the receiver's own token");

  let (status, answer) = asked(&port, "/owner/messages", TOKEN, words).await;

  assert_eq!(status, StatusCode::OK, "{answer:?}");
  assert_eq!(scratch.texts(kept).await, ["Spoken as the owner.".to_owned()]);
}

#[tokio::test]
async fn the_owners_console_line_stands_under_his_registered_name_where_no_service_showed_him() {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let road = Arc::new(StandInChat::new("road-a").holding(&[ROOM]).owners(OWNER));
  let standing = standing(scratch.db(), DataRoot::new(data.path()), [road].to_vec());

  standing.web.users.lock().unwrap().insert(OWNER, "owner-user".to_owned());

  let port = routes(standing.receiver.clone(), as_console(TOKEN));
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
            bytes: b"jpeg".to_vec().into()
          }),
          AlbumItem::File(FileItem {
            name: "two.png".to_owned(),
            mime: "image/png".to_owned(),
            bytes: b"png".to_vec().into()
          })
        ]
        .to_vec(),
        caption: Some("Two pictures\nand a paper".to_owned()),
      },
      ChatSend::Document {
        bytes: b"pdf".to_vec().into(),
        file_name: "paper.pdf".to_owned(),
      },
    ]
    .to_vec(),
  );
}

#[tokio::test]
async fn files_the_room_kept_are_answered_sent_and_carried_though_the_agency_then_fails() {
  let (scratch, standing, road) = standing_over(&[ROOM]).await;
  // The receiver's own wake, which reads the agency's rest before it wakes anybody.
  let standing = woken(standing);
  let port = routes(standing.receiver.clone(), as_console(TOKEN));

  scratch
    .run(r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES (-100, 'Alpha Room')"#, vec![])
    .await;
  bmo_receiver::receiver::rows::allow_chat(scratch.db(), ROOM).await.unwrap();
  standing.agency.rest_unreadable();

  let body = form(
    &[("chatId".to_owned(), "-100".to_owned())],
    &[
      ("image/jpeg".to_owned(), "one.jpg".to_owned(), "jpeg".to_owned()),
      ("application/pdf".to_owned(), "paper.pdf".to_owned(), "pdf".to_owned()),
    ],
  );
  let (status, answer) = form_asked(&port, "/owner/message-files", body).await;

  assert_eq!(status, StatusCode::OK, "{answer:?}");
  assert_eq!(
    answer["sent"]
      .as_array()
      .unwrap()
      .iter()
      .map(|held| held["name"].clone())
      .collect::<Vec<Value>>(),
    [json!("one.jpg"), json!("paper.pdf")],
    "{answer:?}",
  );
  assert!(answer["failed"].as_array().unwrap().is_empty(), "{answer:?}");

  let mut kept = scratch
    .texts(
      r#"SELECT CAST("message_id" AS TEXT) FROM "messages" WHERE "chat_id" = -100 AND "via" = 'record'"#,
    )
    .await;
  let mut answered = [answer["sent"][0]["id"].to_string(), answer["sent"][1]["id"].to_string()];

  kept.sort();
  answered.sort();
  assert_eq!(kept, answered, "each file stands in the room once, under the number the answer gives");
  assert_eq!(road.sent().len(), 2, "the kept files still go on to the room's other providers");
}

#[tokio::test]
async fn words_the_room_kept_are_answered_with_their_number_and_carried_though_the_agency_then_fails() {
  let (scratch, standing, road) = standing_over(&[ROOM]).await;
  // The receiver's own wake, which reads the agency's rest before it wakes anybody.
  let standing = woken(standing);
  let port = routes(standing.receiver.clone(), as_console(TOKEN));

  scratch
    .run(r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES (-100, 'Alpha Room')"#, vec![])
    .await;
  bmo_receiver::receiver::rows::allow_chat(scratch.db(), ROOM).await.unwrap();
  standing.agency.rest_unreadable();

  let (status, answer) = asked(
    &port,
    "/owner/messages",
    TOKEN,
    json_post(json!({ "chatId": ROOM, "text": "The game begins." })),
  )
  .await;

  assert_eq!(status, StatusCode::OK, "{answer:?}");
  assert_eq!(
    scratch
      .texts(
        r#"SELECT CAST("message_id" AS TEXT) FROM "messages" WHERE "chat_id" = -100 AND "via" = 'record'"#,
      )
      .await,
    [answer["id"].to_string()],
    "the words stand in the room once, under the number the answer gives",
  );
  assert_eq!(road.sent().len(), 1, "the kept words still go on to the room's other providers");
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
      audio: b"OggSvoice".to_vec().into(),
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

const MEBIBYTE: usize = 1024 * 1024;

// The owner's files posted into the titled room, each a paper of `bytes` bytes.
async fn papers_sent(port: &Router, sizes: &[usize]) -> (StatusCode, Value) {
  let papers = sizes
    .iter()
    .enumerate()
    .map(|(at, bytes)| ("text/plain".to_owned(), format!("paper-{at}.txt"), "a".repeat(*bytes)))
    .collect::<Vec<(String, String, String)>>();
  let body = form(&[("chatId".to_owned(), ROOM.to_string())], &papers);

  form_asked(port, "/owner/message-files", body).await
}

#[tokio::test]
async fn a_file_past_fifty_mebibytes_is_kept_and_carried_whole_where_no_provider_of_the_room_declares_a_limit()
 {
  let data = tempdir().unwrap();
  let (scratch, standing, road) =
    standing_on(StandInChat::new("road-a").holding(&[ROOM]).owners(OWNER), data.path()).await;
  let port = routes(standing.receiver.clone(), as_console(TOKEN));

  scratch
    .run(r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES (-100, 'Alpha Room')"#, vec![])
    .await;

  let sizes = [50 * MEBIBYTE + 1, MEBIBYTE / 2];
  let (status, answer) = papers_sent(&port, &sizes).await;

  assert_eq!(status, StatusCode::OK, "{answer:?}");
  assert_eq!(answer["sent"].as_array().map(Vec::len), Some(sizes.len()), "{answer:?}");

  let carried = road
    .sent()
    .iter()
    .map(|held| match &held.send {
      ChatSend::Document {
        bytes: bmo_chat_client::FileBody::Inline(bytes),
        ..
      } => bytes.len(),
      other => panic!("a paper goes as a document, not {other:?}"),
    })
    .collect::<Vec<usize>>();

  assert_eq!(carried, sizes, "each file reaches the road whole");

  let kept = scratch
    .texts(r#"SELECT "stored_path" FROM "collected_items" WHERE "chat_id" = -100 ORDER BY "id""#)
    .await
    .iter()
    .map(|path| usize::try_from(std::fs::metadata(path).unwrap().len()).unwrap())
    .collect::<Vec<usize>>();

  assert_eq!(kept, sizes, "each file is kept whole");
  assert_eq!(
    std::fs::read_dir(data.path().join("incoming")).unwrap().count(),
    0,
    "the files the form carried are gone with its request"
  );
}

#[tokio::test]
async fn a_file_past_the_limit_a_provider_of_the_room_declares_is_refused_with_that_limit_which_the_console_reads()
 {
  let data = tempdir().unwrap();
  let limited = StandInChat::new("road-a")
    .holding(&[ROOM])
    .owners(OWNER)
    .taking_files_up_to(MEBIBYTE as u64);
  let (scratch, standing, road) = standing_on(limited, data.path()).await;
  let port = routes(standing.receiver.clone(), as_console(TOKEN));

  scratch
    .run(r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES (-100, 'Alpha Room')"#, vec![])
    .await;

  let (status, answer) = papers_sent(&port, &[MEBIBYTE + 1]).await;

  assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "{answer:?}");
  assert_eq!(answer["code"], "too-large", "{answer:?}");

  let why = answer["error"].as_str().unwrap_or_default();

  assert!(why.contains(&MEBIBYTE.to_string()) && why.contains("road-a"), "{why}");
  assert!(road.sent().is_empty(), "nothing is carried");
  assert!(
    scratch.texts(r#"SELECT "source" FROM "collected_items""#).await.is_empty(),
    "nothing is kept"
  );

  // Within one form, the file past the limit is turned away by its name and the other goes.
  let (status, answer) = papers_sent(&port, &[MEBIBYTE + 1, 10]).await;

  assert_eq!(status, StatusCode::OK, "{answer:?}");
  assert_eq!(answer["sent"][0]["name"], "paper-1.txt", "{answer:?}");
  assert_eq!(answer["failed"][0]["name"], "paper-0.txt", "{answer:?}");

  let (_, room) = read(&port, "/rooms/-100/providers").await;
  let (_, providers) = read(&port, "/providers").await;
  let declared = providers["items"]
    .as_array()
    .unwrap()
    .iter()
    .find(|item| item["key"] == "road-a")
    .cloned();

  assert_eq!(room["fileLimit"], json!({ "provider": "road-a", "bytes": MEBIBYTE }), "{room}");
  assert_eq!(declared.map(|item| item["fileBytes"].clone()), Some(json!(MEBIBYTE)), "{providers}");
}

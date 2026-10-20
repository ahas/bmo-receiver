//! The file, voice and fixed-line kinds of a send asked of the receiver (docs/api.md, 「Sends」;
//! docs/chat-mcp.md, 「Its entries」): a file or several as one album read off this machine, an OGG/Opus voice
//! note with its caption and reading, and a fixed line rendered by the web unit and said without the output
//! gate or any session.

use std::sync::Arc;

use axum::{
  Router,
  body::{Body, to_bytes},
  extract::Request,
  http::StatusCode,
};
use bmo_chat_client::{AlbumItem, ChatCapability, ChatSend, FileItem};
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

use crate::common::Scratch;
use crate::common::units::{
  ADDRESS, AGENT, OWNER, SENDER, StandInChat, StandInGate, StandInWeb, registration, standing,
};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::routes::routes;

const TOKEN: &str = "the-token";
const ROOM: i64 = -100;

// The head of an OGG container, which is what a voice note is read as.
const OGG: &[u8] = &[0x4f, 0x67, 0x67, 0x53, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4];

// A receiver over a scratch database and a data root of the test's own, reaching one road that holds the room
// and declares the verbs given beside the three every road carries.
async fn stood(
  cans: &[ChatCapability],
) -> (Scratch, Router, Arc<StandInChat>, Arc<StandInWeb>, Arc<StandInGate>, TempDir) {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let road = Arc::new(StandInChat::new("road-a").holding(&[ROOM]).owners(OWNER).canning(cans));
  let standing = standing(scratch.db(), DataRoot::new(data.path()), [road.clone()].to_vec());
  let mut agent = registration(AGENT, SENDER, [ADDRESS].to_vec(), None, true);

  agent.row.insert("role".to_owned(), serde_json::json!("chat"));
  standing.agency.registering([agent].to_vec());

  let port = routes(standing.receiver.clone(), TOKEN);
  let web = standing.web.clone();
  let gate = standing.gate.clone();

  (scratch, port, road, web, gate, data)
}

async fn asked(port: &Router, body: Value) -> (StatusCode, Value) {
  let request = Request::post("/sends")
    .header("authorization", format!("Bearer {TOKEN}"))
    .body(Body::from(body.to_string()))
    .unwrap();
  let answer = port.clone().oneshot(request).await.unwrap();
  let status = answer.status();
  let written = to_bytes(answer.into_body(), usize::MAX).await.unwrap();

  (status, serde_json::from_slice(&written).unwrap_or(Value::Null))
}

fn send(tool: &str, input: Value) -> Value {
  json!({
    "asker": { "room": ROOM, "agentName": AGENT, "by": format!("chat:{ROOM}") },
    "tool": tool,
    "input": input,
  })
}

#[tokio::test]
async fn a_file_goes_as_a_document_and_several_go_as_one_album() {
  let (_scratch, port, road, _web, _gate, data) =
    stood(&[ChatCapability::Document, ChatCapability::Album]).await;

  std::fs::write(data.path().join("report.txt"), b"#1337 inspection notes\n").unwrap();
  std::fs::write(data.path().join("notes.txt"), b"the notes\n").unwrap();

  let (status, answer) = asked(
    &port,
    send(
      "file",
      json!({ "files": [{ "path": data.path().join("report.txt").to_string_lossy() }] }),
    ),
  )
  .await;

  assert_eq!(status, StatusCode::OK, "{answer:?}");
  assert_eq!(answer["refusal"], Value::Null, "{answer:?}");
  assert_eq!(
    road.sent().iter().map(|held| held.send.clone()).collect::<Vec<ChatSend>>(),
    [ChatSend::Document {
      bytes: b"#1337 inspection notes\n".to_vec(),
      file_name: "report.txt".to_owned(),
    }]
    .to_vec(),
  );

  let (status, answer) = asked(
    &port,
    send(
      "file",
      json!({
        "files": [
          { "path": data.path().join("report.txt").to_string_lossy() },
          { "path": data.path().join("notes.txt").to_string_lossy() },
        ],
        "caption": "the two of them",
      }),
    ),
  )
  .await;

  assert_eq!(status, StatusCode::OK, "{answer:?}");
  assert_eq!(answer["refusal"], Value::Null, "{answer:?}");
  assert_eq!(
    road.sent().iter().map(|held| held.send.clone()).collect::<Vec<ChatSend>>(),
    [
      ChatSend::Document {
        bytes: b"#1337 inspection notes\n".to_vec(),
        file_name: "report.txt".to_owned(),
      },
      ChatSend::Album {
        items: [
          AlbumItem::File(FileItem {
            name: "report.txt".to_owned(),
            mime: "text/plain".to_owned(),
            bytes: b"#1337 inspection notes\n".to_vec(),
          }),
          AlbumItem::File(FileItem {
            name: "notes.txt".to_owned(),
            mime: "text/plain".to_owned(),
            bytes: b"the notes\n".to_vec(),
          }),
        ]
        .to_vec(),
        caption: Some("the two of them".to_owned()),
      },
    ]
    .to_vec(),
  );
}

#[tokio::test]
async fn a_voice_note_goes_as_a_voice_message_with_its_caption_and_its_reading() {
  let (_scratch, port, road, _web, _gate, data) = stood(&[ChatCapability::Voice]).await;

  std::fs::write(data.path().join("note.ogg"), OGG).unwrap();

  let (status, answer) = asked(
    &port,
    send(
      "voice",
      json!({
        "path": data.path().join("note.ogg").to_string_lossy(),
        "caption": "the news",
        "transcript": "the whole of the news",
      }),
    ),
  )
  .await;

  assert_eq!(status, StatusCode::OK, "{answer:?}");
  assert_eq!(answer["refusal"], Value::Null, "{answer:?}");
  assert_eq!(
    road.sent().iter().map(|held| held.send.clone()).collect::<Vec<ChatSend>>(),
    [ChatSend::Voice {
      audio: OGG.to_vec(),
      said: "the whole of the news".to_owned(),
      caption: Some("the news".to_owned()),
      transcript: Some("the whole of the news".to_owned()),
    }]
    .to_vec(),
  );
}

#[tokio::test]
async fn a_fixed_line_is_rendered_by_the_web_unit_and_said_without_the_output_gate() {
  let (_scratch, port, road, _web, gate, data) = stood(&[]).await;

  let (status, answer) = asked(
    &port,
    send(
      "fixed_line",
      json!({ "code": "draw", "fields": { "label": "the word chain", "result": "apple" } }),
    ),
  )
  .await;

  assert_eq!(status, StatusCode::OK, "{answer:?}");
  assert_eq!(answer["refusal"], Value::Null, "{answer:?}");
  assert_eq!(
    road.sent().iter().map(|held| held.send.clone()).collect::<Vec<ChatSend>>(),
    [ChatSend::Text {
      text: "draw in en".to_owned(),
      silent: false,
      fold: false,
    }]
    .to_vec(),
  );
  assert!(gate.read.lock().unwrap().is_empty(), "a fixed line is already read");
  assert!(data.path().exists());
}

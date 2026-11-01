//! A send asked through the generated client (docs/api.md, 「Sends」; docs/chat-mcp.md, 「Its entries」): the file,
//! voice and fixed-line kinds cross the wire as the receiver's `/sends` route takes them.

use std::sync::{Arc, Mutex};

use axum::body::Bytes;
use axum::extract::State;
use axum::http::Uri;
use axum::{Json, Router};
use receiver_client::{Asker, ReceiverClient, SendRequest};
use serde_json::{Value, json};

const ROOM: i64 = -4_200;
const TOKEN: &str = "the-token";

// A receiver standing on a loopback port: every request kept, each answered with one line.
#[derive(Clone, Default)]
struct Book(Arc<Mutex<Vec<(String, Value)>>>);

async fn heard(State(book): State<Book>, uri: Uri, body: Bytes) -> Json<Value> {
  let said: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);

  book.0.lock().unwrap().push((uri.path().to_owned(), said));

  Json(json!({ "lines": ["sent"], "refusal": null }))
}

async fn stood(book: Book) -> String {
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let base = format!("http://{}", listener.local_addr().unwrap());

  tokio::spawn(async move {
    axum::serve(listener, Router::new().fallback(heard).with_state(book)).await.unwrap();
  });

  base
}

#[tokio::test]
async fn a_file_send_crosses_as_the_receiver_takes_it() {
  let book = Book::default();
  let base = stood(book.clone()).await;
  let input = json!({
    "files": [{ "path": "/tmp/notes.txt" }, { "path": "/tmp/report.pdf", "name": "report.pdf" }],
    "caption": "the two of them",
  });
  let client = ReceiverClient::new(&base, TOKEN).unwrap();

  client
    .send_tool(&SendRequest {
      asker: Asker {
        room: Some(ROOM),
        via: None,
        agent_name: Some("agent-a".to_owned()),
        by: Some(format!("chat:{ROOM}")),
        job: false,
        from: None,
      },
      tool: "file".to_owned(),
      input: input.clone(),
    })
    .await
    .unwrap();

  let kept = book.0.lock().unwrap().clone();

  assert_eq!(
    kept,
    [(
      "/sends".to_owned(),
      json!({
        "asker": {
          "room": ROOM,
          "via": null,
          "agentName": "agent-a",
          "by": format!("chat:{ROOM}"),
          "job": false,
        },
        "tool": "file",
        "input": input,
      }),
    )]
    .to_vec(),
  );
}

#[tokio::test]
async fn a_voice_send_crosses_as_the_receiver_takes_it() {
  let book = Book::default();
  let base = stood(book.clone()).await;
  let input = json!({
    "path": "/tmp/note.ogg",
    "caption": "the news",
    "transcript": "the whole of the news",
  });
  let client = ReceiverClient::new(&base, TOKEN).unwrap();

  client
    .send_tool(&SendRequest {
      asker: Asker {
        room: Some(ROOM),
        via: None,
        agent_name: Some("agent-a".to_owned()),
        by: Some(format!("chat:{ROOM}")),
        job: false,
        from: None,
      },
      tool: "voice".to_owned(),
      input: input.clone(),
    })
    .await
    .unwrap();

  assert_eq!(
    book.0.lock().unwrap().clone(),
    [(
      "/sends".to_owned(),
      json!({
        "asker": {
          "room": ROOM,
          "via": null,
          "agentName": "agent-a",
          "by": format!("chat:{ROOM}"),
          "job": false,
        },
        "tool": "voice",
        "input": input,
      })
    )]
    .to_vec()
  );
}

#[tokio::test]
async fn a_fixed_line_crosses_as_the_receiver_takes_it() {
  let book = Book::default();
  let base = stood(book.clone()).await;
  let input = json!({ "code": "draw", "fields": { "label": "the word chain", "result": "apple" } });
  let client = ReceiverClient::new(&base, TOKEN).unwrap();

  client
    .send_tool(&SendRequest {
      asker: Asker {
        room: Some(ROOM),
        via: None,
        agent_name: Some("agent-a".to_owned()),
        by: Some(format!("chat:{ROOM}")),
        job: false,
        from: None,
      },
      tool: "fixed_line".to_owned(),
      input: input.clone(),
    })
    .await
    .unwrap();

  assert_eq!(
    book.0.lock().unwrap().clone(),
    [(
      "/sends".to_owned(),
      json!({
        "asker": {
          "room": ROOM,
          "via": null,
          "agentName": "agent-a",
          "by": format!("chat:{ROOM}"),
          "job": false,
        },
        "tool": "fixed_line",
        "input": input,
      })
    )]
    .to_vec()
  );
}

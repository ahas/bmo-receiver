//! The console talks the receiver answers (docs/boundary-api.md, 「For bmo-job」, `POST /talks`): the topic, the
//! turns said so far and what the owner says now, taken as a round of the owner's private chat and answered with
//! what the agent said, one turn at a time per talk, and refused with why a turn came back with nothing. A taker
//! of the test's own stands in for the conversations.

use std::sync::{Arc, Mutex};

use axum::{
  Router,
  body::{Body, to_bytes},
  extract::Request,
  http::StatusCode,
  response::Response,
};
use serde_json::{Value, json};
use tempfile::tempdir;
use tower::ServiceExt;

use crate::integration::common::Scratch;
use crate::integration::common::units::{ADDRESS, AGENT, SENDER, registration, standing};
use bmo_receiver::chat::conversation::{BoxFuture, Talking};
use bmo_receiver::chat::talk::{TalkFailure, TalkRequest};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::routes::routes;

const TOKEN: &str = "the-token";

/// The turns asked, and the answers handed back one at a time.
struct Taker {
  asked: Mutex<Vec<TalkRequest>>,
  answers: Mutex<Vec<Result<Option<String>, TalkFailure>>>,
}

impl Talking for Taker {
  fn talk<'a>(&'a self, request: &'a TalkRequest) -> BoxFuture<'a, Result<Option<String>, TalkFailure>> {
    self.asked.lock().unwrap().push(request.clone());
    let answer = self.answers.lock().unwrap().remove(0);

    Box::pin(async { answer })
  }
}

// A taker that stands still until the test hands it an answer, so a second word can be asked while the first is
// being answered.
struct Waiting {
  asked: Mutex<Vec<TalkRequest>>,
  answers: tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<Result<Option<String>, TalkFailure>>>,
}

impl Talking for Waiting {
  fn talk<'a>(&'a self, request: &'a TalkRequest) -> BoxFuture<'a, Result<Option<String>, TalkFailure>> {
    self.asked.lock().unwrap().push(request.clone());

    Box::pin(async {
      self
        .answers
        .lock()
        .await
        .recv()
        .await
        .unwrap_or(Err(TalkFailure::Other("no answer".into())))
    })
  }
}

async fn stood(answers: Vec<Result<Option<String>, TalkFailure>>) -> (Router, Arc<Taker>) {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let standing = standing(scratch.db(), DataRoot::new(data.path()), vec![]);

  standing
    .agency
    .registering([registration(AGENT, SENDER, [ADDRESS].to_vec(), None)].to_vec());

  let taker = Arc::new(Taker {
    asked: Mutex::default(),
    answers: Mutex::new(answers.clone()),
  });
  let port = routes(standing.receiver.clone(), TOKEN);

  standing.receiver.hold_talks(taker.clone());

  (port, taker)
}

fn post(path: &str, token: &str, body: &Value) -> Request {
  Request::post(path)
    .header("authorization", format!("Bearer {token}"))
    .header("content-type", "application/json")
    .body(Body::from(body.to_string()))
    .unwrap()
}

async fn asked(port: Router, path: &str, token: &str, body: &Value) -> (StatusCode, Value) {
  let answer = port.clone().oneshot(post(path, token, body)).await.unwrap();
  let status = answer.status();
  let written = to_bytes(answer.into_body(), usize::MAX).await.unwrap();

  (status, serde_json::from_slice(&written).unwrap_or(Value::Null))
}

fn head(request: &TalkRequest) -> Vec<String> {
  request.head.clone()
}

#[tokio::test]
async fn an_item_talk_is_answered_and_its_head_names_the_item() {
  let (port, taker) = stood([Ok(Some("Nobody has touched it yet.".into()))].to_vec()).await;
  let body = json!({
    "topic": {
      "kind": "item", "id": 900, "title": "Talk about items", "done": false, "merged_into": null,
      "held": false, "hold": null, "stopping": false, "approved": true, "status": "waiting",
    },
    "turns": [],
    "said": "  Where is it?  ",
  });

  assert_eq!(
    asked(port.clone(), "/talks", TOKEN, &body).await,
    (StatusCode::OK, json!({ "answer": "Nobody has touched it yet." }))
  );

  let asked_turn = taker.asked.lock().unwrap();

  assert!(head(&asked_turn[0])[0].contains("item #900"));
  assert_eq!(asked_turn[0].said, "Where is it?");
  assert!(asked_turn[0].turns.is_empty());
}

#[tokio::test]
async fn the_next_turn_is_handed_the_turns_before_it() {
  let (port, taker) = stood([Ok(Some("one".into())), Ok(Some("two".into()))].to_vec()).await;
  let body = json!({
    "topic": { "kind": "item", "id": 900, "title": "x", "done": false, "merged_into": null, "held": false,
      "hold": null, "stopping": false, "approved": true, "status": "waiting" },
    "turns": [{ "owner": true, "text": "first" }, { "owner": false, "text": "second" }],
    "said": "third",
  });

  asked(port.clone(), "/talks", TOKEN, &body).await;

  assert_eq!(taker.asked.lock().unwrap()[0].turns.len(), 2);
}

#[tokio::test]
async fn a_document_talk_keeps_what_he_was_looking_at_and_hands_the_passage_on() {
  let (port, taker) = stood([Ok(None)].to_vec()).await;
  let body = json!({
    "topic": {
      "kind": "document", "document": "docs/wake.md", "label": "docs", "writer": "the jobs",
      "file": "/repo/docs/wake.md", "version": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "heading": "The window", "passage": "A room that woke BMO keeps a window.", "translated": true,
    },
    "turns": [],
    "said": "Why three minutes?",
  });

  let (status, answer) = asked(port.clone(), "/talks", TOKEN, &body).await;

  assert_eq!(status, StatusCode::OK);
  assert_eq!(answer, json!({ "answer": null }));

  let written = head(&taker.asked.lock().unwrap()[0]).join("\n");

  assert!(written.contains("docs/wake.md") && written.contains("A room that woke BMO keeps a window."));
  assert!(written.contains("The window"));
}

#[tokio::test]
async fn a_talk_that_came_back_with_nothing_is_refused_with_its_code() {
  let (port, _taker) = stood([Err(TalkFailure::Resting), Ok(None)].to_vec()).await;
  let body = json!({
    "topic": { "kind": "item", "id": 900, "title": "x", "done": false, "merged_into": null, "held": false,
      "hold": null, "stopping": false, "approved": true, "status": "waiting" },
    "turns": [],
    "said": "Where is it?",
  });

  assert_eq!(
    asked(port.clone(), "/talks", TOKEN, &body).await,
    (
      StatusCode::CONFLICT,
      json!({ "code": "resting", "error": "the agent is resting, so these words were not answered" })
    )
  );
}

#[tokio::test]
async fn a_second_word_while_the_first_is_unanswered_is_refused() {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let standing = standing(scratch.db(), DataRoot::new(data.path()), vec![]);

  standing
    .agency
    .registering([registration(AGENT, SENDER, [ADDRESS].to_vec(), None)].to_vec());

  let (handed, answers) = tokio::sync::mpsc::unbounded_channel();
  let taker = Arc::new(Waiting {
    asked: Mutex::default(),
    answers: tokio::sync::Mutex::new(answers),
  });

  let port = routes(standing.receiver.clone(), TOKEN);

  standing.receiver.hold_talks(taker.clone());

  let body = json!({
    "topic": { "kind": "item", "id": 900, "title": "x", "done": false, "merged_into": null, "held": false,
      "hold": null, "stopping": false, "approved": true, "status": "waiting" },
    "turns": [],
    "said": "Where is it?",
  });
  let first = tokio::spawn(holding_on(port.clone(), body.clone()));

  while taker.asked.lock().unwrap().is_empty() {
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
  }

  let (status, _) = asked(port.clone(), "/talks", TOKEN, &body).await;

  assert_eq!(status, StatusCode::CONFLICT);

  handed.send(Ok(Some("there.".into()))).unwrap();
  first.await.unwrap();
}

// One talk asked of the receiver and held until its answer is handed over.
async fn holding_on(port: Router, body: Value) -> Response {
  port.oneshot(post("/talks", TOKEN, &body)).await.unwrap()
}

#[tokio::test]
async fn a_talk_with_nothing_said_or_no_conversations_standing_is_refused() {
  let (port, _taker) = stood(vec![]).await;

  assert_eq!(
    asked(port.clone(), "/talks", TOKEN, &json!({ "said": "   " })).await.0,
    StatusCode::BAD_REQUEST
  );
  assert_eq!(
    asked(port.clone(), "/talks", "wrong", &json!({ "said": "x" })).await.0,
    StatusCode::UNAUTHORIZED
  );
  assert_eq!(
    asked(port.clone(), "/talks", TOKEN, &json!({ "topic": { "kind": "other" }, "said": "x" }))
      .await
      .0,
    StatusCode::BAD_REQUEST
  );
}

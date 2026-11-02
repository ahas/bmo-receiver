//! The receiver's routes standing as one service (docs/boundary-api.md, 「Routes served」 and 「The receiver's
//! own error shape」): every route of the table answers something of its own rather than the no-route refusal,
//! a request that carries no token is turned away before any route reads it, a path no route answers is refused
//! `no-route` naming the method and the path, and a route whose needs are not met is refused naming what is
//! missing.

use axum::{
  body::{Body, to_bytes},
  extract::Request,
  http::StatusCode,
  response::Response,
};
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{Front, get, patch, post, put, remove, stood};
use crate::integration::common::units::OWNER;

use bmo_receiver::routes::served;

// The routes the base library's own router serves, mounted under `/chat` beside the receiver's table.
const HANDOVERS: [&str; 5] = [
  "GET /chat/rooms",
  "GET /chat/rooms/{room_id}",
  "PUT /chat/rooms/{room_id}/messages/{message_id}",
  "PATCH /chat/rooms/{room_id}/messages/{message_id}",
  "POST /chat/avatar-changes",
];

/// A response's status and JSON body.
async fn read(answer: Response) -> (StatusCode, Value) {
  let status = answer.status();
  let written = to_bytes(answer.into_body(), usize::MAX).await.unwrap();

  (status, serde_json::from_slice(&written).unwrap_or(Value::Null))
}

/// A request asked with no token at all, which is what a stranger brings.
async fn bare(front: &Front, path: &str) -> (StatusCode, Value) {
  let answer = front
    .routes
    .clone()
    .oneshot(Request::get(path).body(Body::empty()).unwrap())
    .await
    .unwrap();

  read(answer).await
}

/// The answer a route gave, as its code where it refused one.
fn refused(body: &Value) -> String {
  body.get("code").and_then(Value::as_str).unwrap_or_default().to_owned()
}

#[tokio::test]
async fn every_route_of_the_table_stands_once() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  for name in served() {
    let Some((method, path)) = name.split_once(" ") else {
      panic!("{name} names no method and no path");
    };

    if HANDOVERS.into_iter().any(|one| name == one) {
      continue;
    }

    let (status, body) = match method {
      "GET" => get(&front, path).await,
      "POST" => post(&front, path, json!({})).await,
      "PUT" => put(&front, path, json!({})).await,
      "PATCH" => patch(&front, path, json!({})).await,
      "DELETE" => remove(&front, path).await,
      other => panic!("{name} names a method no route takes: {other}"),
    };

    assert_ne!(refused(&body), "no-route", "{name} answers the no-route refusal: {status} {body}");
  }

  // The hand-overs the base library serves beside the receiver's own table.
  let handovers = front.handovers();

  for name in HANDOVERS {
    let Some((method, path)) = name.split_once(" ") else {
      panic!("{name} names no path");
    };
    let path = path.replace("{room_id}", "7").replace("{message_id}", "21");
    let (status, body) = match method {
      "GET" => get(&handovers, &path).await,
      "PUT" => put(&handovers, &path, json!({})).await,
      "PATCH" => patch(&handovers, &path, json!({})).await,
      _ => post(&handovers, &path, json!({})).await,
    };

    assert_ne!(refused(&body), "no-route", "{name} answers the no-route refusal: {status} {body}");
  }
}

#[tokio::test]
async fn a_request_without_the_token_is_turned_away_before_any_route_reads_it() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  for path in ["/health", "/no-such-route"] {
    assert_eq!(
      bare(&front, path).await,
      (
        StatusCode::UNAUTHORIZED,
        json!({ "code": "unauthorized", "error": "The bearer token is missing or wrong." }),
      ),
      "{path}",
    );
  }
}

#[tokio::test]
async fn a_path_no_route_answers_is_refused_naming_the_method_and_the_path() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  assert_eq!(
    get(&front, "/no-such-route").await,
    (
      StatusCode::NOT_FOUND,
      json!({ "code": "no-route", "error": "There is no route at GET /no-such-route." }),
    ),
  );
}

#[tokio::test]
async fn a_route_whose_needs_are_not_met_names_what_is_missing() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  // A talk is a round of the chat agent's conversations, which this front stands none of.
  let body = json!({
    "topic": {
      "kind": "item", "id": 900, "title": "Talk about items", "done": false, "merged_into": null,
      "held": false, "hold": null, "stopping": false, "approved": true, "status": "waiting",
    },
    "turns": [],
    "said": "where is it?",
  });

  assert_eq!(
    post(&front, "/talks", body).await,
    (
      StatusCode::SERVICE_UNAVAILABLE,
      json!({ "code": "not-ready", "error": "the chat agent's conversations are not standing." }),
    ),
  );
}

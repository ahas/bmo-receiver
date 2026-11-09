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

  // A talk is a round of the default answerer's conversations, which this front stands none of.
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
      json!({ "code": "not-ready", "error": "the default answerer's conversations are not standing." }),
    ),
  );
}

#[tokio::test]
async fn a_draft_question_is_switched_off_and_a_console_question_taken_away() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;
  let row = "/draft-processes/*/*/*";
  let set = |rounds: Value, off: Value| json!({ "rounds": rounds, "light": ["final"], "off": off, "lightChars": 12 });

  // A question switched off keeps its place; a name no list holds is dropped.
  let (status, body) =
    put(&front, row, set(json!(["information", "facts", "final"]), json!(["facts", "intent"]))).await;

  assert_eq!(status, StatusCode::OK, "{body}");
  assert_eq!(body["rounds"], json!(["information", "facts", "final"]));
  assert_eq!(body["off"], json!(["facts"]));

  let (_, page) = get(&front, "/draft-processes").await;

  assert_eq!(page["items"][0]["off"], json!(["facts"]), "{page}");

  // A set that names no `off` switches nothing off.
  let (status, body) = put(
    &front,
    row,
    json!({ "rounds": ["information", "final"], "light": ["final"], "lightChars": 12 }),
  )
  .await;

  assert_eq!((status, &body["off"]), (StatusCode::OK, &json!([])), "{body}");

  let (status, body) = put(&front, row, set(json!(["information", "final"]), json!(["final"]))).await;

  assert_eq!((status, refused(&body).as_str()), (StatusCode::BAD_REQUEST, "bad-request"), "{body}");

  // A question the console added is taken away once no list names it.
  let (status, _) = put(&front, "/draft-questions/tone", json!({ "text": "Is the tone right?" })).await;

  assert_eq!(status, StatusCode::OK);

  let (status, _) = put(&front, row, set(json!(["tone", "final"]), json!([]))).await;

  assert_eq!(status, StatusCode::OK);

  let (status, body) = remove(&front, "/draft-questions/tone").await;

  assert_eq!((status, refused(&body).as_str()), (StatusCode::CONFLICT, "conflict"), "{body}");

  let (status, _) = put(&front, row, set(json!(["final"]), json!([]))).await;

  assert_eq!(status, StatusCode::OK);
  assert_eq!(remove(&front, "/draft-questions/tone").await.0, StatusCode::NO_CONTENT);

  let (status, body) = remove(&front, "/draft-questions/tone").await;

  assert_eq!((status, refused(&body).as_str()), (StatusCode::NOT_FOUND, "not-found"), "{body}");

  let names = |page: &Value| -> Vec<String> {
    page["items"]
      .as_array()
      .unwrap()
      .iter()
      .map(|one| format!("{} {}", one["name"].as_str().unwrap(), one["source"].as_str().unwrap()))
      .collect()
  };
  let (_, page) = get(&front, "/draft-questions").await;

  assert!(!names(&page).iter().any(|one| one.starts_with("tone ")), "{page}");

  // A question the policy holds stands again in the policy's text, a list naming it or not.
  let (status, _) = put(&front, "/draft-questions/final", json!({ "text": "Write it now." })).await;

  assert_eq!(status, StatusCode::OK);
  assert_eq!(remove(&front, "/draft-questions/final").await.0, StatusCode::NO_CONTENT);

  let (_, page) = get(&front, "/draft-questions").await;

  assert!(names(&page).contains(&"final default".to_owned()), "{page}");
}

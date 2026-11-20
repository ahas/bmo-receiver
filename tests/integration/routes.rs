//! The receiver's routes standing as one service (docs/boundary-api.md, 「Routes served」 and 「The receiver's
//! own error shape」): every route of the table answers something of its own rather than the no-route refusal,
//! a request that carries no token is turned away before any route reads it, a path no route answers is refused
//! `no-route` naming the method and the path, a route whose needs are not met is refused naming what is
//! missing, and the routes that file a person's words are taken under the console relay's credential alone
//! (docs/api.md, 「Auth」).

use axum::{
  Router,
  body::{Body, to_bytes},
  extract::Request,
  http::StatusCode,
  response::Response,
};
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{
  Front, RECEIVER_TOKEN, TOKEN, get, patch, post, put, remove, stood,
};
use crate::integration::common::units::OWNER;

use bmo_receiver::routes::{Bearers, routes, served};

// The routes the base library's own router serves, mounted under `/chat` beside the receiver's table.
const HANDOVERS: [&str; 9] = [
  "GET /chat/rooms",
  "GET /chat/rooms/{room_id}",
  "PUT /chat/rooms/{room_id}/messages/{message_id}",
  "PATCH /chat/rooms/{room_id}/messages/{message_id}",
  "POST /chat/avatar-changes",
  "POST /chat/reaction-changes",
  "POST /chat/deletions",
  "POST /chat/typing-marks",
  "POST /chat/presence-changes",
];

// The receiving side of the chat protocol as its published contract states it, which the base library's router
// serves.
const PROTOCOL: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../base/chat/docs/openapi/receiver.json");

// A route with each path parameter's name left out, so two spellings of one route read alike.
fn shape(route: &str) -> String {
  let mut shaped = String::new();
  let mut naming = false;

  for one in route.chars() {
    match one {
      '{' => {
        naming = true;
        shaped.push('{');
      }
      '}' => {
        naming = false;
        shaped.push('}');
      }
      _ if naming => {}
      _ => shaped.push(one),
    }
  }

  shaped
}

#[test]
fn every_route_the_protocol_serves_on_the_receiving_side_stands_in_the_list() {
  let contract: Value = serde_json::from_str(&std::fs::read_to_string(PROTOCOL).unwrap()).unwrap();
  let listed: Vec<String> = served().iter().map(|route| shape(route)).collect();
  let mut seen = 0;

  for (path, methods) in contract["paths"].as_object().unwrap() {
    for method in methods.as_object().unwrap().keys() {
      if !["get", "put", "post", "patch", "delete"].contains(&method.as_str()) {
        continue;
      }

      let route = format!("{} /chat{path}", method.to_uppercase());

      assert!(listed.contains(&shape(&route)), "{route} is served and missing from the routes list");
      assert!(
        HANDOVERS.iter().any(|one| shape(one) == shape(&route)),
        "{route} is missing from the hand-overs"
      );
      seen += 1;
    }
  }

  assert_eq!(seen, HANDOVERS.len(), "the hand-overs list names no route the protocol does not serve");
}

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

// The routes that file a person's words, each its method and a path naming a room and a message.
const PERSONS_WORDS: [(&str, &str); 7] = [
  ("POST", "/owner/messages"),
  ("POST", "/owner/message-files"),
  ("POST", "/owner/instructions"),
  ("POST", "/rooms/42/words"),
  ("POST", "/rooms/42/word-files"),
  ("PATCH", "/rooms/42/messages/21"),
  ("POST", "/rooms/42/messages/21/forwards"),
];

/// A request asked of routes under a bearer, with an empty JSON body where the method carries one.
async fn under(routes: &Router, token: &str, method: &str, path: &str) -> (StatusCode, Value) {
  let body = if method == "GET" { Body::empty() } else { Body::from("{}") };
  let request = Request::builder()
    .method(method)
    .uri(path)
    .header("authorization", format!("Bearer {token}"))
    .header("content-type", "application/json")
    .body(body)
    .unwrap();

  read(routes.clone().oneshot(request).await.unwrap()).await
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
async fn a_route_that_files_a_persons_words_refuses_the_receivers_own_token_before_reading_anything() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  for (method, path) in PERSONS_WORDS {
    let (status, body) = under(&front.routes, RECEIVER_TOKEN, method, path).await;

    assert_eq!(
      (status, refused(&body)),
      (StatusCode::FORBIDDEN, "forbidden".to_owned()),
      "{method} {path}"
    );

    let (status, body) = under(&front.routes, TOKEN, method, path).await;

    assert_ne!(
      refused(&body),
      "forbidden",
      "{method} {path} under the console's token: {status} {body}"
    );
  }

  let (status, _) = under(&front.routes, RECEIVER_TOKEN, "GET", "/health").await;

  assert_eq!(status, StatusCode::OK, "every other route still takes the receiver's own token");
}

#[tokio::test]
async fn without_a_console_credential_of_its_own_no_token_files_a_persons_words() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;
  let unnamed = routes(front.receiver(), RECEIVER_TOKEN);
  let named_as_the_receivers =
    routes(front.receiver(), Bearers::new(RECEIVER_TOKEN).console(Some(RECEIVER_TOKEN)));

  for routes in [unnamed, named_as_the_receivers] {
    for (method, path) in PERSONS_WORDS {
      let (status, body) = under(&routes, RECEIVER_TOKEN, method, path).await;

      assert_eq!(
        (status, refused(&body)),
        (StatusCode::FORBIDDEN, "forbidden".to_owned()),
        "{method} {path}"
      );
    }
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
async fn a_supervisor_s_stage_chain_and_texts_are_its_own_and_an_added_stage_leaves_once_no_chain_names_it()
{
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;
  let chain = "/instruction-stages/agent-a";
  let names = |page: &Value| -> Vec<String> {
    page["items"]
      .as_array()
      .unwrap()
      .iter()
      .map(|one| format!("{} {}", one["name"].as_str().unwrap(), one["source"].as_str().unwrap()))
      .collect()
  };

  // A supervisor with no chain of its own follows the policy's.
  let (_, page) = get(&front, "/instruction-stages?agentName=agent-a").await;

  assert_eq!(page["items"].as_array().map(Vec::len), Some(1), "{page}");
  assert_eq!(
    (&page["items"][0]["agentName"], &page["items"][0]["source"]),
    (&json!("*"), &json!("default"))
  );

  // A stage the console adds is that supervisor's: its chain may name it, another's may not.
  let (status, body) =
    put(&front, "/instruction-texts/agent-a/tone", json!({ "text": "Is the tone right?" })).await;

  assert_eq!((status, &body["policyText"]), (StatusCode::OK, &Value::Null), "{body}");

  let (status, body) = put(&front, chain, json!({ "stages": ["tone", "review", "tone"] })).await;

  assert_eq!(
    (status, &body["stages"]),
    (StatusCode::OK, &json!(["tone", "review"])),
    "named once each: {body}"
  );

  let (status, body) = put(&front, "/instruction-stages/agent-b", json!({ "stages": ["tone"] })).await;

  assert_eq!((status, refused(&body).as_str()), (StatusCode::BAD_REQUEST, "bad-request"), "{body}");

  let (_, page) = get(&front, "/instruction-stages").await;

  assert_eq!(
    page["items"][0]["source"],
    json!("default"),
    "the policy's chain opens the table: {page}"
  );
  assert_eq!(page["items"][1]["agentName"], json!("agent-a"), "{page}");

  // No chain and no text stands for every supervisor, and a stage's name and text are held to their shape.
  for (key, body) in [
    ("/instruction-stages/*", json!({ "stages": ["review"] })),
    ("/instruction-texts/*/tone", json!({ "text": "Is the tone right?" })),
    ("/instruction-texts/agent-a/Tone", json!({ "text": "Is the tone right?" })),
    ("/instruction-texts/agent-a/tone", json!({ "text": " " })),
  ] {
    let (status, answer) = put(&front, key, body).await;

    assert_eq!(
      (status, refused(&answer).as_str()),
      (StatusCode::BAD_REQUEST, "bad-request"),
      "{key}: {answer}"
    );
  }

  // An added stage a chain still names stays; once out of the chain it leaves the catalogue.
  let (status, body) = remove(&front, "/instruction-texts/agent-a/tone").await;

  assert_eq!((status, refused(&body).as_str()), (StatusCode::CONFLICT, "conflict"), "{body}");
  assert_eq!(put(&front, chain, json!({ "stages": ["review"] })).await.0, StatusCode::OK);
  assert_eq!(remove(&front, "/instruction-texts/agent-a/tone").await.0, StatusCode::NO_CONTENT);

  let (status, body) = remove(&front, "/instruction-texts/agent-a/tone").await;

  assert_eq!((status, refused(&body).as_str()), (StatusCode::NOT_FOUND, "not-found"), "{body}");

  // A policy stage's text set for one supervisor is read by it alone, and carries the policy's text to go back to.
  let (status, body) =
    put(&front, "/instruction-texts/agent-a/review", json!({ "text": "Check the names." })).await;

  assert_eq!(status, StatusCode::OK, "{body}");
  assert!(body["policyText"].as_str().is_some_and(|text| text != "Check the names."), "{body}");

  let (_, own) = get(&front, "/instruction-texts?agentName=agent-a").await;
  let (_, other) = get(&front, "/instruction-texts?agentName=agent-b").await;

  assert_eq!(names(&own), ["review set"], "{own}");
  assert_eq!(names(&other), ["review default"], "another supervisor reads the policy's: {other}");
  assert_eq!(remove(&front, "/instruction-texts/agent-a/review").await.0, StatusCode::NO_CONTENT);

  let (_, own) = get(&front, "/instruction-texts?agentName=agent-a").await;

  assert_eq!(names(&own), ["review default"], "{own}");

  // The chain taken away, the supervisor follows the policy's again.
  assert_eq!(remove(&front, chain).await.0, StatusCode::NO_CONTENT);
  assert_eq!(remove(&front, chain).await.0, StatusCode::NOT_FOUND);
}

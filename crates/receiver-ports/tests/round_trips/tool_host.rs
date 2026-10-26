//! The tool host's port tried against a stand-in on a Unix socket (`docs/boundary-api.md`, 「The tool host」): the
//! catalogue's changes read off its event stream with the issuer token, a cursor the host holds no event after
//! answered `410 gone`, and the tool sessions read page by page.

use std::time::Duration;

use axum::extract::Query;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use futures_util::StreamExt;
use receiver_ports::tool_host::{HttpToolHost, ToolHost};
use serde_json::json;

const ISSUER: &str = "issuer-token-a";

fn issued(headers: &HeaderMap) -> bool {
  headers.get("authorization").and_then(|value| value.to_str().ok()) == Some(&format!("Bearer {ISSUER}"))
}

async fn events(
  headers: HeaderMap,
  Query(query): Query<Vec<(String, String)>>,
) -> axum::response::Response {
  if !issued(&headers) {
    return (
      StatusCode::FORBIDDEN,
      axum::Json(json!({ "code": "forbidden", "error": "Not the issuer." })),
    )
      .into_response();
  }

  if query.iter().any(|(name, value)| name == "after" && value == "5") {
    return (
      StatusCode::GONE,
      axum::Json(json!({ "code": "gone", "error": "No event after 5 is held." })),
    )
      .into_response();
  }

  let event = json!({
    "id": 5, "kind": "tool.placed", "at": 1,
    "data": { "tool": "sample", "namespaces": ["bmo"], "replaced": true,
              "entries": [{ "tool": "bmo/sample", "reachedBy": "call", "summary": "Samples a thing." }] },
  });

  (
    [("content-type", "text/event-stream")],
    format!(": ping\n\nid: 5\nevent: tool.placed\ndata: {event}\n\n"),
  )
    .into_response()
}

async fn sessions(
  headers: HeaderMap,
  Query(query): Query<Vec<(String, String)>>,
) -> axum::response::Response {
  if !issued(&headers) {
    return StatusCode::FORBIDDEN.into_response();
  }

  let page = if query.iter().any(|(name, value)| name == "after" && value == "1") {
    json!({ "items": [{ "holderName": "agent-b", "namespaces": ["web"], "createdAt": 2, "labels": {} }], "more": false })
  } else {
    json!({ "items": [{ "holderName": "agent-a", "namespaces": ["bmo", "web"], "createdAt": 1, "labels": {} }], "more": true })
  };

  axum::Json(page).into_response()
}

#[tokio::test]
async fn the_catalogues_changes_and_the_sessions_are_read_on_the_socket_with_the_issuer_token() {
  let dir = tempfile::tempdir().unwrap();
  let socket = dir.path().join("host.sock");
  let token = dir.path().join("issuer-token");

  std::fs::write(&token, format!("{ISSUER}\n")).unwrap();

  let listener = tokio::net::UnixListener::bind(&socket).unwrap();
  let app = axum::Router::new().route("/events", get(events)).route("/tool-sessions", get(sessions));

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  let host = HttpToolHost::new(socket, token, Duration::from_secs(5));
  let read: Vec<_> = host.events(None).await.unwrap().collect().await;

  assert_eq!(read.len(), 1, "{read:?}");

  let event = read[0].as_ref().unwrap();

  assert_eq!(
    (event.id, event.kind.as_str(), event.data.tool.as_str(), event.data.replaced),
    (5, "tool.placed", "sample", true)
  );
  assert_eq!(event.data.entries[0].reached_by, "call");

  let gone = host.events(Some(5)).await.err().unwrap();

  assert_eq!(gone.status(), Some(410));
  assert_eq!(gone.code(), Some("gone"));

  let standing = host.sessions().await.unwrap();

  assert_eq!(
    standing.iter().map(|session| session.holder_name.as_str()).collect::<Vec<_>>(),
    ["agent-a", "agent-b"]
  );
}

#[tokio::test]
async fn a_stream_that_says_nothing_past_its_bound_ends_as_dropped() {
  let dir = tempfile::tempdir().unwrap();
  let socket = dir.path().join("host.sock");
  let token = dir.path().join("issuer-token");

  std::fs::write(&token, ISSUER).unwrap();

  let listener = tokio::net::UnixListener::bind(&socket).unwrap();
  let app = axum::Router::new().route(
    "/events",
    get(|| async {
      let silent = futures_util::stream::pending::<Result<String, std::io::Error>>();

      ([("content-type", "text/event-stream")], axum::body::Body::from_stream(silent)).into_response()
    }),
  );

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  let host = HttpToolHost::new(socket, token, Duration::from_millis(200));
  let read: Vec<_> =
    tokio::time::timeout(Duration::from_secs(5), host.events(None).await.unwrap().collect())
      .await
      .expect("the silent stream ends");

  assert_eq!(read.len(), 1);
  assert!(read[0].is_err(), "{read:?}");
}

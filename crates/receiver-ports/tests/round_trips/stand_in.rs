//! An in-process stand-in for another unit. It takes every call on a loopback port, records it, and answers it
//! from the fixture set for its method and path. A call with no fixture is answered 500 with the code
//! `no-fixture`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use receiver_ports::Peer;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

/// One call the stand-in took.
#[derive(Clone, Debug, PartialEq)]
pub struct Call {
  /// Its method and path, such as `POST /session/turn`.
  pub route: String,
  /// Its query as it was sent; empty where it had none.
  pub query: String,
  /// Its body read as JSON; none where it had no body.
  pub body: Option<Value>,
  /// Its `Authorization` header as it was sent; none where it had none.
  pub authorization: Option<String>,
}

/// What a route answers: a status, and a body with its type where it has one.
#[derive(Clone)]
struct Fixture {
  status: StatusCode,
  body: Option<(&'static str, String)>,
}

#[derive(Default)]
struct Book {
  calls: Vec<Call>,
  fixtures: HashMap<String, Fixture>,
}

type Shared = Arc<Mutex<Book>>;

/// The stand-in, serving until it is dropped.
pub struct StandIn {
  book: Shared,
  base: String,
  server: JoinHandle<()>,
}

async fn take(
  State(book): State<Shared>,
  method: Method,
  uri: Uri,
  headers: HeaderMap,
  body: Bytes,
) -> Response {
  let route = format!("{method} {}", uri.path());
  let body = (!body.is_empty()).then(|| serde_json::from_slice(&body).expect("a call's body is JSON"));
  let authorization = headers
    .get(header::AUTHORIZATION)
    .map(|value| value.to_str().expect("an authorization header is text").to_owned());
  let fixture = {
    let mut book = book.lock().expect("the book is not poisoned");

    book.calls.push(Call {
      route: route.clone(),
      query: uri.query().unwrap_or_default().to_owned(),
      body,
      authorization,
    });
    book.fixtures.get(&route).cloned()
  };

  match fixture {
    Some(Fixture {
      status,
      body: Some((kind, body)),
    }) => (status, [(header::CONTENT_TYPE, kind)], Body::from(body)).into_response(),
    Some(Fixture { status, body: None }) => status.into_response(),
    None => {
      let said = json!({ "error": format!("No fixture is set for {route}."), "code": "no-fixture" });

      (StatusCode::INTERNAL_SERVER_ERROR, axum::Json(said)).into_response()
    }
  }
}

impl StandIn {
  /// Starts the stand-in on a free loopback port.
  pub async fn start() -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a loopback port is free");
    let bound = listener.local_addr().expect("the port is bound");
    let book = Shared::default();
    let app = Router::new().fallback(take).with_state(book.clone());
    let server = tokio::spawn(async move {
      axum::serve(listener, app).await.expect("the stand-in serves");
    });

    Self {
      book,
      base: format!("http://{bound}"),
      server,
    }
  }

  /// The unit's peer, as the configuration would name it: the stand-in's address and the token given.
  pub fn peer(&self, token: &str) -> Peer {
    Peer {
      base: self.base.clone(),
      token: token.to_owned(),
    }
  }

  fn book(&self) -> MutexGuard<'_, Book> {
    self.book.lock().expect("the book is not poisoned")
  }

  /// Sets `route` to answer `status` with a JSON `body`.
  pub fn answer(&self, route: &str, status: u16, body: Value) {
    self.set(route, status, Some(("application/json", body.to_string())));
  }

  /// Sets `route` to answer `status` with no body.
  pub fn answer_empty(&self, route: &str, status: u16) {
    self.set(route, status, None);
  }

  /// Sets `route` to answer a stream of server-sent events, written out whole.
  pub fn answer_events(&self, route: &str, events: &str) {
    self.set(route, 200, Some(("text/event-stream", events.to_owned())));
  }

  fn set(&self, route: &str, status: u16, body: Option<(&'static str, String)>) {
    let status = StatusCode::from_u16(status).expect("a fixture's status is a status");

    self.book().fixtures.insert(route.to_owned(), Fixture { status, body });
  }

  /// Every call taken so far, in order.
  pub fn calls(&self) -> Vec<Call> {
    self.book().calls.clone()
  }

  /// Each call taken so far as its route, query and body, in order.
  pub fn took(&self) -> Vec<(String, String, Option<Value>)> {
    self.calls().into_iter().map(|call| (call.route, call.query, call.body)).collect()
  }
}

impl Drop for StandIn {
  fn drop(&mut self) {
    self.server.abort();
  }
}

/// A route, query and body as `StandIn::took` gives them.
pub fn call(route: &str, query: &str, body: Option<Value>) -> (String, String, Option<Value>) {
  (route.to_owned(), query.to_owned(), body)
}

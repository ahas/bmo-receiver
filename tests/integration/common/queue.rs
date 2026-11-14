//! What the receiver's own route tests stand on (`docs/boundary-api.md`, 「Routes served」): the receiver's
//! routes standing as one router over a scratch schema and a data root of the test's own, asked one request at
//! a time with no listener, and a recording instruction pass so a test can see that queuing a row woke a turn.

use std::sync::{Arc, Mutex};

use axum::{
  Router,
  body::{Body, to_bytes},
  extract::Request,
  http::StatusCode,
  response::Response,
};
use futures_util::future::BoxFuture;
use serde_json::Value;
use tempfile::tempdir;
use tower::ServiceExt;

use crate::integration::common::Scratch;
use crate::integration::common::units::{StandInChat, Standing, standing};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::{Pass, Receiver};
use bmo_receiver::routes::routes;

pub const TOKEN: &str = "fixture";

/// The instruction pass, keeping how many times it was asked to hand the rows over at once.
pub struct Kicked(pub Mutex<usize>);

impl Pass for Kicked {
  fn kick(&self) {
    let mut held = self.0.lock().unwrap();

    *held += 1;
  }

  fn riders<'a>(&'a self, _agent: &'a str, _chat_id: i64) -> BoxFuture<'a, ()> {
    Box::pin(async {})
  }
}

impl Kicked {
  pub fn times(&self) -> usize {
    *self.0.lock().unwrap()
  }
}

/// The receiver's routes standing for a test: the router, the data root it writes under, the units it reaches
/// and the pass it kicks.
pub struct Front {
  pub routes: Router,
  pub data: tempfile::TempDir,
  pub standing: Standing,
  pub kicked: Arc<Kicked>,
}

/// The front standing as the receiver for an owner, on roots of its own and no chat service.
pub async fn stood(scratch: &Scratch, owner: i64) -> Front {
  stood_on(scratch, owner, vec![]).await
}

/// The same, on the chat services a test needs the rooms to stand on.
pub async fn stood_on(scratch: &Scratch, _owner: i64, chats: Vec<Arc<StandInChat>>) -> Front {
  let data = tempdir().unwrap();
  let standing = standing(scratch.db(), DataRoot::new(data.path()), chats);
  let receiver = standing.receiver.clone();
  let kicked = Arc::new(Kicked(Mutex::default()));

  receiver.hold_pass(kicked.clone());

  Front {
    routes: routes(receiver.clone(), TOKEN),
    data,
    standing,
    kicked,
  }
}

impl Front {
  /// The receiver the routes answer behind.
  pub fn receiver(&self) -> Receiver {
    self.standing.receiver.clone()
  }

  /// The chat services' hand-over routes of the same receiver, served by the base library's own router behind the
  /// receiver's token, beside the front a test keeps asking.
  pub fn handover_routes(&self) -> Router {
    bmo_receiver::receiver::handover::routes(self.receiver(), TOKEN)
  }

  /// The same front asked on the chat services' hand-over routes, served by the base library's own router behind
  /// the receiver's token.
  pub fn handovers(self) -> Front {
    Front {
      routes: self.handover_routes(),
      data: self.data,
      standing: self.standing,
      kicked: self.kicked,
    }
  }
}

/// A JSON body posted to the front with the token; the status and the JSON answered.
pub async fn post(front: &Front, path: &str, body: Value) -> (StatusCode, Value) {
  posted_to(front.routes.clone(), path, body).await
}

/// A JSON body posted to routes with the token; the status and the JSON answered.
pub async fn posted_to(routes: Router, path: &str, body: Value) -> (StatusCode, Value) {
  let answer = routes
    .oneshot(
      Request::post(path)
        .header("authorization", format!("Bearer {TOKEN}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap(),
    )
    .await
    .unwrap();

  read(answer).await
}

/// A GET asked of the front with the token.
pub async fn get(front: &Front, path: &str) -> (StatusCode, Value) {
  let answer = front
    .routes
    .clone()
    .oneshot(
      Request::get(path)
        .header("authorization", format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  read(answer).await
}

/// A GET asked of the front with the token and the headers given, its response as it came, for a route answering
/// bytes.
pub async fn fetched(front: &Front, path: &str, headers: &[(&str, &str)]) -> Response {
  let asked = headers.iter().fold(
    Request::get(path).header("authorization", format!("Bearer {TOKEN}")),
    |asked, (name, value)| asked.header(*name, *value),
  );

  front.routes.clone().oneshot(asked.body(Body::empty()).unwrap()).await.unwrap()
}

/// A DELETE asked of the front with the token.
pub async fn remove(front: &Front, path: &str) -> (StatusCode, Value) {
  let answer = front
    .routes
    .clone()
    .oneshot(
      Request::delete(path)
        .header("authorization", format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .unwrap(),
    )
    .await
    .unwrap();

  read(answer).await
}

/// A PUT asked of the front with the token.
pub async fn put(front: &Front, path: &str, body: Value) -> (StatusCode, Value) {
  let answer = front
    .routes
    .clone()
    .oneshot(
      Request::put(path)
        .header("authorization", format!("Bearer {TOKEN}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap(),
    )
    .await
    .unwrap();

  read(answer).await
}

/// A PATCH asked of the front with the token.
pub async fn patch(front: &Front, path: &str, body: Value) -> (StatusCode, Value) {
  let answer = front
    .routes
    .clone()
    .oneshot(
      Request::patch(path)
        .header("authorization", format!("Bearer {TOKEN}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap(),
    )
    .await
    .unwrap();

  read(answer).await
}

/// The warmer's hook posted with no header at all, the token standing in the URL it registers.
pub async fn pinged(front: &Front, path: &str, body: Value) -> (StatusCode, Value) {
  let answer = front
    .routes
    .clone()
    .oneshot(Request::post(path).body(Body::from(body.to_string())).unwrap())
    .await
    .unwrap();

  read(answer).await
}

/// A response's status and JSON body.
pub async fn read(answer: Response) -> (StatusCode, Value) {
  let status = answer.status();
  let written = to_bytes(answer.into_body(), usize::MAX).await.unwrap();

  (status, serde_json::from_slice(&written).unwrap_or(Value::Null))
}

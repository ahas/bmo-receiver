//! How every port reaches its unit: behind the unit's token where it takes one, a refusal read with its code, a
//! refused connection offered again only for the patience window, and an address that is no `http` one refused.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use futures_util::future::BoxFuture;
use receiver_ports::http::Done;
use receiver_ports::{Caller, Patience, Peer, PortFailure, Wait};
use serde_json::json;

use crate::stand_in::StandIn;

#[tokio::test]
async fn a_call_carries_the_units_token_and_a_unit_given_none_is_called_with_no_header() {
  let unit = StandIn::start().await;

  unit.answer("POST /ping", 200, json!({ "ok": true }));

  let _: Done = Caller::new(&unit.peer("the-token")).post(&["ping"], &json!({})).await.unwrap();
  let _: Done = Caller::new(&unit.peer("")).post(&["ping"], &json!({})).await.unwrap();

  let headers: Vec<Option<String>> = unit.calls().into_iter().map(|call| call.authorization).collect();

  assert_eq!(headers, [Some("Bearer the-token".to_owned()), None]);
}

#[tokio::test]
async fn a_refusal_is_read_with_its_status_and_code_and_a_path_segment_is_encoded() {
  let unit = StandIn::start().await;

  unit.answer("GET /rooms/a%20b", 409, json!({ "code": "conflict", "error": "It stands otherwise." }));

  let failure = Caller::new(&unit.peer("t")).get::<Done>(&["rooms", "a b"], &[]).await.unwrap_err();

  assert_eq!((failure.status(), failure.code()), (Some(409), Some("conflict")));
}

// A waiter that returns at once, counting the waits it was asked for.
#[derive(Default)]
struct Counted(AtomicU32);

impl Wait for Counted {
  fn wait(&self, _interval: Duration) -> BoxFuture<'_, ()> {
    self.0.fetch_add(1, Ordering::SeqCst);

    Box::pin(async {})
  }
}

#[tokio::test]
async fn a_refused_connection_is_offered_again_for_the_window_and_then_given_up() {
  // A port that was bound and let go, so nothing listens there.
  let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
  let waits = Arc::new(Counted::default());
  let patience = Patience {
    window: Duration::from_secs(6),
    every: Duration::from_secs(2),
    waiter: waits.clone(),
  };
  let peer = Peer {
    base: format!("http://{closed}"),
    token: "t".to_owned(),
  };

  let failure = Caller::new(&peer)
    .with_patience(patience)
    .post::<Done>(&["ping"], &json!({}))
    .await
    .unwrap_err();

  assert!(
    matches!(failure, PortFailure::Unreachable { waited, .. } if waited == Duration::from_secs(6)),
    "{failure:?}"
  );
  assert_eq!(waits.0.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn an_address_that_is_no_http_one_is_refused_before_anything_is_sent() {
  let peer = Peer {
    base: "unix:/run/unit.sock".to_owned(),
    token: String::new(),
  };

  let failure = Caller::new(&peer).post::<Done>(&["ping"], &json!({})).await.unwrap_err();

  assert!(matches!(failure, PortFailure::NotAnAddress { .. }), "{failure:?}");
}

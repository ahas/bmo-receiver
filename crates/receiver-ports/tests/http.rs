//! How every port reaches its unit: behind the unit's token where it takes one, a refusal read with its code, a
//! refused connection offered again until the unit listens or only once where no wait is set, and an address that is no `http` one refused.

use std::io::{Read, Write};
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

// A waiter that returns at once, counting the waits it was asked for, and that opens a unit at the address on the
// wait it is told to, answering every request it takes with success.
struct Opening {
  waits: AtomicU32,
  opens_at: u32,
  address: std::net::SocketAddr,
}

impl Wait for Opening {
  fn wait(&self, _interval: Duration) -> BoxFuture<'_, ()> {
    if self.waits.fetch_add(1, Ordering::SeqCst) + 1 == self.opens_at {
      let listener = std::net::TcpListener::bind(self.address).unwrap();

      std::thread::spawn(move || {
        for stream in listener.incoming() {
          let mut stream = stream.unwrap();
          let mut head = [0_u8; 4096];
          let _ = stream.read(&mut head).unwrap();
          let body = r#"{"ok":true}"#;
          let answer = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
          );

          stream.write_all(answer.as_bytes()).unwrap();
        }
      });
    }

    Box::pin(async {})
  }
}

// A port that was bound and let go, so nothing listens there.
fn closed() -> std::net::SocketAddr {
  std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap()
}

#[tokio::test]
async fn a_refused_connection_is_offered_again_until_the_unit_listens() {
  let address = closed();
  let waits = Arc::new(Opening {
    waits: AtomicU32::new(0),
    opens_at: 3,
    address,
  });
  let patience = Patience {
    every: Duration::from_secs(2),
    waiter: waits.clone(),
  };
  let peer = Peer {
    base: format!("http://{address}"),
    token: "t".to_owned(),
  };

  let done: Done = Caller::new(&peer).with_patience(patience).post(&["ping"], &json!({})).await.unwrap();

  assert!(done.ok);
  assert_eq!(waits.waits.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn a_refused_connection_with_no_wait_set_is_given_up_at_once() {
  let address = closed();
  let waits = Arc::new(Opening {
    waits: AtomicU32::new(0),
    opens_at: 0,
    address,
  });
  let patience = Patience {
    every: Duration::ZERO,
    waiter: waits.clone(),
  };
  let peer = Peer {
    base: format!("http://{address}"),
    token: "t".to_owned(),
  };

  let failure = Caller::new(&peer)
    .with_patience(patience)
    .post::<Done>(&["ping"], &json!({}))
    .await
    .unwrap_err();

  assert!(matches!(failure, PortFailure::Unreachable { .. }), "{failure:?}");
  assert_eq!(waits.waits.load(Ordering::SeqCst), 0);
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

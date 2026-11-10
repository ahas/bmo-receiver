//! The chat services' port against a stand-in of a chat service's provider routes (bmo-chat's
//! `docs/protocol.md`), each service reached by its road's name.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use bmo_chat_client::{ChatHealth, ChatSend, ChatSendRequest, ClientFailure};
use receiver_ports::Peer;
use receiver_ports::chat::ChatServices;
use serde_json::json;

use crate::stand_in::{StandIn, call};

#[tokio::test]
async fn a_road_named_in_the_configuration_is_reached_through_the_chat_client() {
  let unit = StandIn::start().await;
  let chats = ChatServices::from_peers(&BTreeMap::from([("road-a".to_owned(), unit.peer("chat-token"))]));

  unit.answer("GET /health", 200, json!({ "ready": true, "reason": null, "version": "0.1.0" }));
  unit.answer(
    "GET /rooms/-7",
    200,
    json!({
      "id": -7,
      "owns": true,
      "identity": null,
      "kind": "group",
      "privateTo": null,
      "title": "a room"
    }),
  );
  unit.answer("GET /owner", 200, json!({ "roomId": -7 }));

  assert!(chats.road("road-b").is_none(), "a road the configuration does not list is none");
  assert_eq!(chats.names().collect::<Vec<_>>(), ["road-a"]);

  let road = chats.road("road-a").unwrap();

  assert_eq!(road.health().await.unwrap(), ChatHealth::ready("0.1.0"));
  assert!(road.room_owns(-7).await.unwrap());
  assert_eq!(road.room_title(-7).await.unwrap().as_deref(), Some("a room"));
  assert_eq!(road.owner_room().await.unwrap(), Some(-7));

  assert_eq!(
    unit.took(),
    [
      call("GET /health", "", None),
      call("GET /rooms/-7", "", None),
      call("GET /rooms/-7", "", None),
      call("GET /owner", "", None),
    ]
  );
  assert!(
    unit
      .calls()
      .iter()
      .all(|call| call.authorization.as_deref() == Some("Bearer chat-token"))
  );
}

fn text(text: &str) -> ChatSendRequest {
  ChatSendRequest::new(
    -7,
    ChatSend::Text {
      text: text.to_owned(),
      silent: false,
      fold: false,
    },
  )
}

#[tokio::test]
async fn a_refused_road_is_skipped_at_once_and_a_live_slow_one_still_waits() {
  let closed = std::net::TcpListener::bind("[::1]:0").unwrap().local_addr().unwrap();
  let down = ChatServices::from_peers(&BTreeMap::from([(
    "road-down".to_owned(),
    Peer {
      base: format!("http://{closed}"),
      token: "t".to_owned(),
    },
  )]));
  let started = Instant::now();
  let failure = down.road("road-down").unwrap().send(&text("hello")).await.unwrap_err();

  assert!(
    matches!(failure, ClientFailure::Unreachable { waited, .. } if waited == Duration::ZERO),
    "{failure:?}"
  );
  assert!(started.elapsed() < Duration::from_secs(1), "a refused road is not waited on");

  let unit = StandIn::start().await;
  unit.answer_after("POST /sends", 200, json!({ "messageId": 7 }), Duration::from_millis(150));

  let slow = ChatServices::from_peers(&BTreeMap::from([("road-slow".to_owned(), unit.peer("t"))]));
  let started = Instant::now();
  let sent = slow.road("road-slow").unwrap().send(&text("hello")).await.unwrap();

  assert_eq!(sent.message_id, Some(7));
  assert!(started.elapsed() >= Duration::from_millis(150), "a live slow road still waits");
}

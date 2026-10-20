//! The chat services' port against a stand-in of a chat service's provider routes (bmo-chat's
//! `docs/protocol.md`), each service reached by its road's name.

use std::collections::BTreeMap;

use bmo_chat_client::ChatHealth;
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

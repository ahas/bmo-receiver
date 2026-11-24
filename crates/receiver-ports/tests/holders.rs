//! The declared holders' port against a stand-in holder (`docs/boundary-api.md`, 「A declared holder」): the call
//! carries the token of the configured unit at the holder's origin, and its answer is read by the holder's codes.

use receiver_ports::Peer;
use receiver_ports::holders::{Holders, Holding, HttpHolders};
use serde_json::json;

use crate::stand_in::{StandIn, call};

fn elsewhere() -> Peer {
  Peer {
    base: "http://127.0.0.1:9".to_owned(),
    token: "elsewhere".to_owned(),
  }
}

#[tokio::test]
async fn a_holder_is_asked_behind_the_token_of_the_unit_configured_at_its_origin() {
  let unit = StandIn::start().await;
  let configured = unit.peer("holder-token");
  let declared = format!("{}/", configured.base);

  unit.answer(
    "GET /rooms/-7/holdings",
    200,
    json!({ "holdings": [{ "code": "room_records", "count": 6 }] }),
  );

  let held = HttpHolders::new(vec![elsewhere(), configured]).holdings(&declared, -7).await.unwrap();

  assert_eq!(
    held,
    [Holding {
      code: "room_records".to_owned(),
      count: 6
    }]
  );
  assert_eq!(unit.took(), [call("GET /rooms/-7/holdings", "", None)]);
  assert_eq!(unit.calls()[0].authorization.as_deref(), Some("Bearer holder-token"));
}

#[tokio::test]
async fn a_holder_at_an_origin_no_configured_unit_stands_at_is_asked_with_no_token() {
  let unit = StandIn::start().await;

  unit.answer("GET /rooms/-7/holdings", 200, json!({ "holdings": [] }));

  let held = HttpHolders::new(vec![elsewhere()]).holdings(&unit.peer("").base, -7).await.unwrap();

  assert!(held.is_empty());
  assert_eq!(unit.calls()[0].authorization, None);
}

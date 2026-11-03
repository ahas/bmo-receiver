//! The brain's port against a stand-in of the brain's published routes (the brain's `docs/boundary-api.md`).

use receiver_ports::brain::{BrainUnit, HttpBrain};
use receiver_ports::{Peer, PortFailure};
use serde_json::json;

use crate::stand_in::{StandIn, call};

#[tokio::test]
async fn the_agent_s_brain_its_due_state_and_a_fold_go_out_as_the_brain_publishes_them_behind_its_token() {
  let unit = StandIn::start().await;
  let brain = HttpBrain::new(&unit.peer("t"));

  unit.answer(
    "GET /brains",
    200,
    json!({ "items": [{ "id": 9, "agentKey": "agent-a" }], "more": false }),
  );
  unit.answer(
    "POST /brains/9/recall",
    200,
    json!({
      "block": "", "chosen": [], "tokenCount": 0, "budgetTokens": 1800,
      "unconsolidatedImportance": 151, "consolidationDue": true, "released": []
    }),
  );
  unit.answer("POST /brains/9/folds", 200, json!({ "clearedCount": 4 }));

  assert_eq!(brain.brain_of("agent-a").await.unwrap(), Some(9));
  assert!(brain.due(9).await.unwrap());
  assert_eq!(brain.fold(9, "session-1").await.unwrap(), 4);

  assert_eq!(
    unit.took(),
    [
      call("GET /brains", "agentKey=agent-a&limit=2", None),
      call("POST /brains/9/recall", "", Some(json!({ "words": "", "record": false }))),
      call("POST /brains/9/folds", "", Some(json!({ "sessionKey": "session-1" }))),
    ]
  );
  assert!(unit.calls().iter().all(|call| call.authorization.as_deref() == Some("Bearer t")));
}

#[tokio::test]
async fn an_agent_with_no_brain_or_more_than_one_has_none() {
  let unit = StandIn::start().await;
  let brain = HttpBrain::new(&unit.peer("t"));

  unit.answer("GET /brains", 200, json!({ "items": [], "more": false }));
  assert_eq!(brain.brain_of("agent-a").await.unwrap(), None);

  unit.answer("GET /brains", 200, json!({ "items": [{ "id": 9 }, { "id": 10 }], "more": false }));
  assert_eq!(brain.brain_of("agent-a").await.unwrap(), None);
}

#[tokio::test]
async fn a_brain_that_does_not_listen_ends_the_call_at_once() {
  let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
  let address = listener.local_addr().unwrap();

  drop(listener);

  let brain = HttpBrain::new(&Peer {
    base: format!("http://{address}"),
    token: "t".to_owned(),
  });
  let failure = brain.due(9).await.unwrap_err();

  assert!(matches!(failure, PortFailure::Unreachable { .. }), "{failure:?}");
}

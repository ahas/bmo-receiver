//! The brain's port against a stand-in of the brain's published routes (the brain's `docs/boundary-api.md`).

use receiver_ports::brain::{BrainUnit, HttpBrain};
use receiver_ports::{Peer, PortFailure};
use serde_json::json;

use crate::stand_in::{StandIn, call};

fn one_brain(unit: &StandIn, id: i64) {
  unit.answer(
    "GET /brains",
    200,
    json!({ "items": [{ "id": id, "agentKey": "agent-a" }], "more": false }),
  );
}

fn consolidation(due: bool) -> serde_json::Value {
  json!({ "unconsolidatedImportance": 151, "consolidationDue": due, "threshold": 150, "unconsolidatedCount": 20 })
}

#[tokio::test]
async fn the_due_state_and_a_fold_go_out_as_the_brain_publishes_them_behind_its_token() {
  let unit = StandIn::start().await;
  let brain = HttpBrain::new(&unit.peer("t"));

  one_brain(&unit, 9);
  unit.answer("GET /brains/9/consolidation", 200, consolidation(true));
  unit.answer("POST /brains/9/folds", 200, json!({ "clearedCount": 4 }));

  assert_eq!(brain.due("agent-a").await.unwrap(), Some(true));
  assert_eq!(brain.fold("agent-a", "session-1").await.unwrap(), Some(4));

  assert_eq!(
    unit.took(),
    [
      call("GET /brains", "agentKey=agent-a&limit=2", None),
      call("GET /brains/9/consolidation", "", None),
      call("POST /brains/9/folds", "", Some(json!({ "sessionKey": "session-1" }))),
    ],
    "the agent's brain is looked up once and kept"
  );
  assert!(unit.calls().iter().all(|call| call.authorization.as_deref() == Some("Bearer t")));
}

#[tokio::test]
async fn a_kept_brain_the_brain_answers_404_for_is_let_go_and_looked_up_again() {
  let unit = StandIn::start().await;
  let brain = HttpBrain::new(&unit.peer("t"));

  one_brain(&unit, 9);
  unit.answer(
    "GET /brains/9/consolidation",
    404,
    json!({ "code": "not-found", "message": "No such brain." }),
  );

  assert_eq!(brain.due("agent-a").await.unwrap(), None, "a brain gone is no brain");

  one_brain(&unit, 10);
  unit.answer("GET /brains/10/consolidation", 200, consolidation(false));

  assert_eq!(brain.due("agent-a").await.unwrap(), Some(false));

  let routes: Vec<String> = unit.took().into_iter().map(|(route, _, _)| route).collect();

  assert_eq!(
    routes,
    [
      "GET /brains",
      "GET /brains/9/consolidation",
      "GET /brains",
      "GET /brains/10/consolidation"
    ]
  );
}

#[tokio::test]
async fn an_agent_with_no_brain_or_more_than_one_has_none_and_nothing_is_kept() {
  let unit = StandIn::start().await;
  let brain = HttpBrain::new(&unit.peer("t"));

  unit.answer("GET /brains", 200, json!({ "items": [], "more": false }));
  assert_eq!(brain.due("agent-a").await.unwrap(), None);

  unit.answer("GET /brains", 200, json!({ "items": [{ "id": 9 }, { "id": 10 }], "more": false }));
  assert_eq!(brain.fold("agent-a", "session-1").await.unwrap(), None);

  let routes: Vec<String> = unit.took().into_iter().map(|(route, _, _)| route).collect();

  assert_eq!(routes, ["GET /brains", "GET /brains"], "no call is made on a brain not found");
}

#[tokio::test]
async fn a_brain_that_does_not_listen_ends_the_call_at_once() {
  let listener = std::net::TcpListener::bind("[::1]:0").unwrap();
  let address = listener.local_addr().unwrap();

  drop(listener);

  let brain = HttpBrain::new(&Peer {
    base: format!("http://{address}"),
    token: "t".to_owned(),
  });
  let failure = brain.due("agent-a").await.unwrap_err();

  assert!(matches!(failure, PortFailure::Unreachable { .. }), "{failure:?}");
}

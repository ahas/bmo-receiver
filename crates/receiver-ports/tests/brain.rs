//! The brain's port against a stand-in of the brain's published routes (the brain's `docs/boundary-api.md`).

use receiver_ports::brain::{BrainUnit, HttpBrain, RoomRule};
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

#[tokio::test]
async fn a_rooms_rules_are_read_without_a_use_recorded_and_stand_however_often_they_are_read() {
  let unit = StandIn::start().await;
  let brain = HttpBrain::new(&unit.peer("t"));
  let rule = json!({
    "id": 31,
    "type": "rule",
    "text": "Every member may ask about the system.",
    "status": "active",
    "subjects": [{ "id": 5, "type": "room", "key": "-7" }, { "id": 6, "type": "person", "key": "42" }],
  });
  let other = json!({
    "id": 30,
    "type": "rule",
    "text": "Member 43 may file items.",
    "status": "active",
    "subjects": [{ "id": 5, "type": "room", "key": "-7" }],
  });

  one_brain(&unit, 9);
  unit.answer(
    "GET /brains/9/subjects",
    200,
    json!({ "items": [{ "id": 5, "type": "room", "key": "-7" }] }),
  );
  unit.answer("GET /brains/9/memories", 200, json!({ "items": [rule, other], "more": false }));

  let first = brain.room_rules("agent-a", -7).await.unwrap().unwrap();
  let second = brain.room_rules("agent-a", -7).await.unwrap().unwrap();

  assert_eq!(
    first,
    [
      RoomRule {
        id: 30,
        persons: vec![],
        text: "Member 43 may file items.".to_owned(),
      },
      RoomRule {
        id: 31,
        persons: vec!["42".to_owned()],
        text: "Every member may ask about the system.".to_owned(),
      },
    ],
    "the rules come lowest id first, each with the people it is about"
  );
  assert_eq!(second, first, "a rule read once is read again");

  let rules = call(
    "GET /brains/9/memories",
    "type=rule&status=active&subjectId=5&record=false&limit=500",
    None,
  );

  assert_eq!(
    unit.took(),
    [
      call("GET /brains", "agentKey=agent-a&limit=2", None),
      call("GET /brains/9/subjects", "type=room&key=-7&limit=1", None),
      rules.clone(),
      call("GET /brains/9/subjects", "type=room&key=-7&limit=1", None),
      rules,
    ],
    "the read records no use and moves no rule: nothing but reads is asked"
  );
}

#[tokio::test]
async fn a_room_the_brain_holds_no_subject_of_has_no_rules() {
  let unit = StandIn::start().await;
  let brain = HttpBrain::new(&unit.peer("t"));

  one_brain(&unit, 9);
  unit.answer("GET /brains/9/subjects", 200, json!({ "items": [] }));

  assert_eq!(brain.room_rules("agent-a", -7).await.unwrap(), Some(vec![]));
}

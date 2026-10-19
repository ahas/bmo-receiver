//! The gate port against a stand-in of the routes the receiver states for the gate (`docs/boundary-api.md`,
//! 「The gate」).

use receiver_ports::gate::{
  Entry, Envelope, Gate, HttpGate, Judgement, RoomKind, Sanitise, Sanitised, Situation, Source, TurnEvent,
};
use serde_json::json;

use crate::stand_in::{StandIn, call};

#[tokio::test]
async fn every_call_on_the_gate_goes_out_as_the_receiver_states_it() {
  let unit = StandIn::start().await;
  let gate = HttpGate::new(&unit.peer("t"));

  unit.answer(
    "POST /reference/selected",
    200,
    json!({ "entries": [{ "path": "docs/a.md", "text": "a", "source": "document", "fragment": 5 }] }),
  );
  unit.answer("POST /sanitised", 200, json!({ "text": "clean", "changed": true }));
  unit.answer_empty("POST /judgements", 204);
  unit.answer_empty("POST /judgements/attached", 204);
  unit.answer_empty("POST /options/copied", 204);

  let envelope = Envelope {
    chat: -7,
    message: Some(11),
    lines: vec!["someone: words".to_owned()],
    speakers: vec![42],
    situation: Some(Situation {
      agent: "agent-a".to_owned(),
      room: RoomKind::Group,
      events: vec![TurnEvent {
        kind: "job-ended".to_owned(),
        item: Some(3),
      }],
      now: 100,
    }),
    holds: None,
    round: Some("r-1".to_owned()),
    people_spoke: true,
    answered: None,
    off: vec![],
    records: true,
  };

  assert_eq!(
    gate.reference(&envelope).await.unwrap(),
    [Entry {
      path: "docs/a.md".to_owned(),
      text: "a".to_owned(),
      source: Source::Document,
      fragment: Some(5),
    }]
  );

  let asked = Sanitise {
    text: "words".to_owned(),
    label: "a message".to_owned(),
    chat: Some(-7),
    message: Some(11),
  };

  assert_eq!(
    gate.sanitise(&asked).await.unwrap(),
    Sanitised {
      text: "clean".to_owned(),
      changed: true,
    }
  );

  let judgement = Judgement {
    chat: -7,
    message: None,
    stage: "round".to_owned(),
    verdict: "skip".to_owned(),
    detail: None,
    ms: Some(12),
    round: None,
  };

  gate.judge(&judgement).await.unwrap();
  gate.attach(-7, 12, 1_700_000_000).await.unwrap();
  gate.copy_options("companion-a", "agent-a").await.unwrap();

  assert_eq!(
    unit.took(),
    [
      call(
        "POST /reference/selected",
        "",
        Some(json!({
          "chat": -7,
          "message": 11,
          "lines": ["someone: words"],
          "speakers": [42],
          "situation": { "agent": "agent-a", "room": "group", "events": [{ "kind": "job-ended", "item": 3 }], "now": 100 },
          "round": "r-1",
          "people_spoke": true,
          "off": [],
          "records": true,
        }))
      ),
      call(
        "POST /sanitised",
        "",
        Some(json!({ "text": "words", "label": "a message", "chat": -7, "message": 11 }))
      ),
      call(
        "POST /judgements",
        "",
        Some(json!({ "chat": -7, "stage": "round", "verdict": "skip", "ms": 12 }))
      ),
      call(
        "POST /judgements/attached",
        "",
        Some(json!({ "chat": -7, "message": 12, "since": 1_700_000_000 }))
      ),
      call("POST /options/copied", "", Some(json!({ "agent": "companion-a", "from": "agent-a" }))),
    ]
  );
}

#[tokio::test]
async fn a_stopped_sanitiser_run_comes_back_with_its_code() {
  let unit = StandIn::start().await;

  unit.answer("POST /sanitised", 409, json!({ "code": "stopped", "error": "The run was stopped." }));

  let asked = Sanitise {
    text: "words".to_owned(),
    label: "a message".to_owned(),
    chat: None,
    message: None,
  };
  let failure = HttpGate::new(&unit.peer("t")).sanitise(&asked).await.unwrap_err();

  assert_eq!(failure.code(), Some("stopped"));
}

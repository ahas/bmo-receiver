//! The gate port against a stand-in of the gate's published routes (`docs/api.md`, 「Reference」, 「Judgements」
//! and 「Gates」).

use receiver_ports::gate::{
  Entry, Envelope, Gate, HttpGate, Judgement, RoomKind, Situation, Source, TurnEvent,
};
use serde_json::json;

use crate::stand_in::{StandIn, call};

#[tokio::test]
async fn every_call_on_the_gate_goes_out_in_the_shape_the_gate_publishes() {
  let unit = StandIn::start().await;
  let gate = HttpGate::new(&unit.peer("t"));

  unit.answer(
    "POST /reference/selected",
    200,
    json!({ "entries": [{ "path": "docs/a.md", "text": "a", "source": "document" }] }),
  );
  unit.answer("POST /judgements", 201, json!({ "id": 1 }));
  unit.answer_empty("POST /judgements/attached", 204);
  unit.answer_empty("POST /options/copied", 204);

  let envelope = Envelope {
    room_id: -7,
    message_id: Some(11),
    lines: vec!["someone: words".to_owned()],
    speaker_ids: vec![42],
    situation: Some(Situation {
      agent_name: "agent-a".to_owned(),
      room_kind: RoomKind::Group,
      events: vec![TurnEvent {
        kind: "job-ended".to_owned(),
        item_id: Some(3),
      }],
      now_at: 1_700_000_000_000,
    }),
    holds: None,
    round: Some("r-1".to_owned()),
    people_spoke: true,
    answered: None,
    records: true,
  };

  assert_eq!(
    gate.reference(&envelope).await.unwrap(),
    [Entry {
      path: "docs/a.md".to_owned(),
      text: "a".to_owned(),
      source: Source::Document,
      fragment: None,
      holds: None,
    }]
  );

  let judgement = Judgement {
    room_id: -7,
    message_id: None,
    gate_name: "round".to_owned(),
    verdict: "skip".to_owned(),
    detail: None,
    ms: Some(12),
    round: None,
  };

  gate.judge(&judgement).await.unwrap();
  gate.attach(-7, 12, 1_700_000_000_000).await.unwrap();
  gate.copy_options("agent-d", "agent-a").await.unwrap();

  assert_eq!(
    unit.took(),
    [
      call(
        "POST /reference/selected",
        "",
        Some(json!({
          "roomId": -7,
          "messageId": 11,
          "lines": ["someone: words"],
          "speakerIds": [42],
          "situation": {
            "agentName": "agent-a",
            "roomKind": "group",
            "events": [{ "kind": "job-ended", "itemId": 3 }],
            "nowAt": 1_700_000_000_000_i64,
          },
          "round": "r-1",
          "peopleSpoke": true,
          "records": true,
        }))
      ),
      call(
        "POST /judgements",
        "",
        Some(json!({ "roomId": -7, "gateName": "round", "verdict": "skip", "ms": 12 }))
      ),
      call(
        "POST /judgements/attached",
        "",
        Some(json!({ "roomId": -7, "messageId": 12, "since": 1_700_000_000_000_i64 }))
      ),
      call(
        "POST /options/copied",
        "",
        Some(json!({ "agentName": "agent-d", "fromAgentName": "agent-a" }))
      ),
    ]
  );
  assert!(unit.calls().iter().all(|call| call.authorization.as_deref() == Some("Bearer t")));
}

//! The voice engine's port against a stand-in of the engine's published routes (the voice's `docs/boundary-api.md`,
//! 「Briefings」).

use receiver_ports::voice::{Briefing, HttpVoice, NewBriefing, VoiceEngine};
use serde_json::json;

use crate::stand_in::{StandIn, call};

fn briefing(id: i64, kind: &str, state: &str) -> serde_json::Value {
  json!({ "id": id, "chatId": -5, "messageId": 11, "kind": kind, "state": state, "plays": false })
}

#[tokio::test]
async fn a_briefing_a_lines_lookup_and_a_stop_of_one_kind_go_out_as_the_engine_publishes_them_behind_its_token()
 {
  let unit = StandIn::start().await;
  let voice = HttpVoice::new(&unit.peer("t"));

  unit.answer("POST /briefings", 201, briefing(1, "full", "waiting"));
  unit.answer("GET /briefings", 200, json!({ "items": [briefing(1, "full", "done")], "more": false }));
  unit.answer(
    "POST /briefings/stop",
    200,
    json!({ "items": [briefing(2, "summary", "stopped")], "more": false }),
  );

  let taken = voice
    .brief(&NewBriefing {
      voice_name: "voice-a".to_owned(),
      text: "The backup finished.".to_owned(),
      chat_id: -5,
      message_id: Some(11),
      kind: "full".to_owned(),
      title: None,
      emotion: None,
      plays: false,
      key: None,
      asked_by: "agent-a".to_owned(),
    })
    .await
    .unwrap();
  let standing = voice.of_line(-5, 11, "full").await.unwrap();
  let stopped = voice.stop_kind("summary").await.unwrap();
  let read = |id: i64, kind: &str, state: &str| Briefing {
    id,
    kind: kind.to_owned(),
    state: state.to_owned(),
    synthesis_id: None,
  };

  assert_eq!(
    (taken, standing, stopped),
    (
      read(1, "full", "waiting"),
      vec![read(1, "full", "done")],
      vec![read(2, "summary", "stopped")]
    )
  );
  assert_eq!(
    unit.took(),
    [
      call(
        "POST /briefings",
        "",
        Some(json!({
          "voiceName": "voice-a",
          "text": "The backup finished.",
          "chatId": -5,
          "messageId": 11,
          "kind": "full",
          "plays": false,
          "askedBy": "agent-a",
        }))
      ),
      call("GET /briefings", "chatId=-5&messageId=11&kind=full&limit=10", None),
      call("POST /briefings/stop", "", Some(json!({ "kind": "summary" }))),
    ],
    "the stop names its kind and no room, so it stops that kind in every room"
  );
  assert!(unit.calls().iter().all(|call| call.authorization.as_deref() == Some("Bearer t")));
}

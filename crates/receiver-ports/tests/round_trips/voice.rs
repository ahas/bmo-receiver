//! The voice unit's port against a stand-in of the routes the receiver states for it (`docs/boundary-api.md`,
//! 「The voice unit」).

use receiver_ports::voice::{Briefed, Briefing, BriefingKind, HttpVoice, Voice};
use serde_json::json;

use crate::stand_in::{StandIn, call};

#[tokio::test]
async fn a_briefing_is_launched_listed_and_handed_its_line() {
  let unit = StandIn::start().await;
  let voice = HttpVoice::new(&unit.peer("t"));

  unit.answer("POST /briefings", 200, json!({ "run": "voice-1" }));
  unit.answer(
    "GET /briefings",
    200,
    json!({ "briefings": [{ "run": "voice-1", "chat": -7, "voice_message": null, "kind": "notice", "title": "t", "finished": false }] }),
  );
  unit.answer_empty("POST /briefings/voice-1/line", 204);

  let briefing = Briefing {
    chat: -7,
    message: None,
    kind: BriefingKind::Notice,
    title: "t".to_owned(),
    awaits_line: true,
    text: "words".to_owned(),
  };

  assert_eq!(voice.brief(&briefing).await.unwrap(), "voice-1");
  assert_eq!(
    voice.briefings(-7, true).await.unwrap(),
    [Briefed {
      run: "voice-1".to_owned(),
      chat: -7,
      message: None,
      voice_message: None,
      kind: BriefingKind::Notice,
      title: "t".to_owned(),
      finished: false,
    }]
  );
  voice.line("voice-1", 12).await.unwrap();

  assert_eq!(
    unit.took(),
    [
      call(
        "POST /briefings",
        "",
        Some(json!({ "chat": -7, "kind": "notice", "title": "t", "awaits_line": true, "text": "words" }))
      ),
      call("GET /briefings", "chat=-7&awaiting=true", None),
      call("POST /briefings/voice-1/line", "", Some(json!({ "message": 12 }))),
    ]
  );
}

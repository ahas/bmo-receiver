//! The brain port against a stand-in of the brain's routes (the brain's `docs/boundary-api.md`).

use receiver_ports::brain::{Brain, HttpBrain, Load, Reading, Reminder, RoomRule, RuleText};
use serde_json::json;

use crate::stand_in::{StandIn, call};

fn rule() -> RoomRule {
  RoomRule {
    id: "r1".to_owned(),
    subject: 42,
    text: "no spoilers".to_owned(),
  }
}

#[tokio::test]
async fn the_rules_and_the_reminders_go_out_as_the_brain_states_them() {
  let unit = StandIn::start().await;
  let brain = HttpBrain::new(&unit.peer("t"));
  let rules = json!({ "rules": [{ "id": "r1", "subject": 42, "text": "no spoilers" }] });

  unit.answer("POST /room-rules/read", 200, rules.clone());
  unit.answer("POST /room-rules/set", 200, rules);
  unit.answer("POST /reminders/set", 200, json!({ "answered": true, "lines": ["set for 09:00"] }));
  unit.answer(
    "POST /reminders/list",
    200,
    json!({ "reminders": [{ "id": 3, "reason": "stand-up" }], "lines": ["#3 stand-up"] }),
  );

  assert_eq!(brain.room_rules(-7).await.unwrap(), [rule()]);

  let text = RuleText {
    subject: 42,
    text: "no spoilers".to_owned(),
  };

  assert_eq!(brain.set_room_rule(-7, 11, "r1", Some(&text)).await.unwrap(), Ok(vec![rule()]));

  let reminder = Reminder {
    agent: "agent-a".to_owned(),
    chat_id: -7,
    at: None,
    every: Some("daily 09:00".to_owned()),
    reason: "stand-up".to_owned(),
    memory: None,
    user: Some(42),
  };

  assert_eq!(
    brain.set_reminder(&reminder).await.unwrap(),
    Ok(Reading {
      lines: vec!["set for 09:00".to_owned()]
    })
  );

  let listed = brain.reminders(-7, None, true).await.unwrap();

  assert_eq!(
    (listed.reminders[0]["id"].clone(), listed.lines),
    (json!(3), vec!["#3 stand-up".to_owned()])
  );

  assert_eq!(
    unit.took(),
    [
      call("POST /room-rules/read", "", Some(json!({ "chatId": -7 }))),
      call(
        "POST /room-rules/set",
        "",
        Some(
          json!({ "chatId": -7, "messageId": 11, "id": "r1", "rule": { "subject": 42, "text": "no spoilers" } })
        )
      ),
      call(
        "POST /reminders/set",
        "",
        Some(
          json!({ "agent": "agent-a", "chatId": -7, "every": "daily 09:00", "reason": "stand-up", "user": 42 })
        )
      ),
      call("POST /reminders/list", "", Some(json!({ "chatId": -7, "agent": null, "all": true }))),
    ]
  );
}

#[tokio::test]
async fn a_refusal_the_brain_answers_comes_back_as_its_code_and_parameters() {
  let unit = StandIn::start().await;

  unit.answer(
    "POST /reminders/cancel",
    200,
    json!({ "answered": false, "refusal": { "code": "no-reminder", "message": "There is no reminder 9.", "id": 9 } }),
  );

  let refused = HttpBrain::new(&unit.peer("t")).cancel_reminder(9).await.unwrap().unwrap_err();

  assert_eq!((refused.code.as_str(), refused.params["id"].clone()), ("no-reminder", json!(9)));
  assert_eq!(unit.took(), [call("POST /reminders/cancel", "", Some(json!({ "id": 9 })))]);
}

#[tokio::test]
async fn the_sessions_rows_and_a_rooms_leaving_go_out_as_the_brain_states_them() {
  let unit = StandIn::start().await;
  let brain = HttpBrain::new(&unit.peer("t"));
  let load = Load {
    path: "docs/a.md".to_owned(),
    version: "v1".to_owned(),
  };
  let ok = json!({ "ok": true });

  unit.answer(
    "POST /sessions/loads",
    200,
    json!({ "loads": [{ "path": "docs/a.md", "version": "v1" }] }),
  );
  unit.answer("POST /sessions/loads/keep", 200, ok.clone());
  unit.answer("POST /sessions/loads/forget", 200, ok.clone());
  unit.answer("POST /session-summary/read", 200, json!({ "note": null }));
  unit.answer("POST /session-summary/clear", 200, ok.clone());
  unit.answer("POST /people/before-fold", 200, json!({ "outcome": "launched", "unit": "u-1" }));
  unit.answer("GET /memories", 200, json!({ "memories": [{ "slug": "s" }] }));
  unit.answer("POST /notes/list", 200, json!({ "notes": [] }));
  unit.answer("POST /watches/list", 200, json!({ "watches": [{ "id": 1 }], "lines": [] }));
  unit.answer("POST /rooms/forgotten", 200, ok);
  unit.answer("POST /rooms/removed", 200, json!({ "took": { "rows": { "notes": 2 } } }));

  let loads = [load];

  assert_eq!(brain.loads("s-1").await.unwrap(), loads);
  brain.keep_loads("s-1", &loads, 100).await.unwrap();
  brain.forget_loads("s-1").await.unwrap();
  assert_eq!(brain.session_summary("agent-a").await.unwrap(), None);
  brain.clear_session_summary("agent-a").await.unwrap();
  assert_eq!(brain.people_before_fold(-7, "agent-a").await.unwrap().detail["unit"], "u-1");
  assert_eq!(brain.memories(Some("agent-a"), Some(-7)).await.unwrap()[0]["slug"], "s");
  assert!(brain.notes("agent-a", None).await.unwrap().is_empty());
  assert_eq!(brain.watches(Some(-7), None, false).await.unwrap()[0]["id"], 1);
  brain.room_forgotten(-7).await.unwrap();
  assert_eq!(brain.room_removed(-7, true).await.unwrap()["rows"]["notes"], 2);

  assert_eq!(
    unit.took(),
    [
      call("POST /sessions/loads", "", Some(json!({ "session": "s-1" }))),
      call(
        "POST /sessions/loads/keep",
        "",
        Some(json!({ "session": "s-1", "loads": [{ "path": "docs/a.md", "version": "v1" }], "at": 100 }))
      ),
      call("POST /sessions/loads/forget", "", Some(json!({ "session": "s-1" }))),
      call("POST /session-summary/read", "", Some(json!({ "agent": "agent-a" }))),
      call("POST /session-summary/clear", "", Some(json!({ "agent": "agent-a" }))),
      call("POST /people/before-fold", "", Some(json!({ "chatId": -7, "agent": "agent-a" }))),
      call("GET /memories", "agent=agent-a&chat=-7", None),
      call("POST /notes/list", "", Some(json!({ "agent": "agent-a", "chatId": null }))),
      call("POST /watches/list", "", Some(json!({ "by": -7, "agent": null, "all": false }))),
      call("POST /rooms/forgotten", "", Some(json!({ "chatId": -7 }))),
      call("POST /rooms/removed", "", Some(json!({ "chatId": -7, "preview": true }))),
    ]
  );
}

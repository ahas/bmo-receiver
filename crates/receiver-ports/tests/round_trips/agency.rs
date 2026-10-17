//! The agency port against a stand-in of the agency's routes (the agency's `docs/api.md`).

use futures_util::StreamExt;
use receiver_ports::agency::{Agency, Delivery, Event, HttpAgency, Scope, Turn, TurnRoom};
use serde_json::json;

use crate::stand_in::{StandIn, call};

#[tokio::test]
async fn every_agency_call_goes_out_as_its_document_states_and_its_answer_reads() {
  let unit = StandIn::start().await;
  let agency = HttpAgency::new(&unit.peer("t"));
  let session = json!({ "id": "agent-a@-7", "agent": "agent-a", "scope": { "kind": "room", "room": -7 } });

  unit.answer(
    "GET /agents",
    200,
    json!({ "agents": [{ "name": "agent-a", "awake": false, "brainFolder": "", "roads": ["chat"] }], "roles": ["chat"] }),
  );
  unit.answer("POST /session/start", 200, session.clone());
  unit.answer("POST /session/turn", 200, json!({ "delivery": "blocked" }));
  unit.answer(
    "GET /session/state",
    200,
    json!({ "session": "agent-a@-7", "provider": "claude-code", "model": "m", "place": null, "generation": 2, "context": {} }),
  );
  unit.answer("POST /session/fold", 200, json!({ "ok": true }));
  unit.answer("GET /session/list", 200, json!({ "sessions": [session] }));
  unit.answer(
    "POST /rooms/forget",
    200,
    json!({ "gone": { "room_sessions": 1 }, "stayed": [{ "table": "agent_log", "rows": 3, "why": "the job's" }] }),
  );
  unit.answer_empty("POST /hooks", 204);
  unit.answer("GET /run-log", 200, json!({ "lines": [{ "id": 4, "text": "a line" }] }));

  let agents = agency.agents().await.unwrap();

  assert_eq!(agents.agents[0].name, "agent-a");
  assert!(!agents.agents[0].awake);
  assert_eq!(agents.agents[0].brain_folder.as_deref(), Some(""));
  assert_eq!(agents.agents[0].row["roads"], json!(["chat"]), "the rest of the row is kept as it came");

  let started = agency.start("agent-a", Scope::Room { room: -7 }).await.unwrap();

  assert_eq!(started.scope, Scope::Room { room: -7 });

  let turn = Turn {
    session: started.id.clone(),
    prompt: "words".to_owned(),
    room: Some(TurnRoom {
      chat_id: -7,
      text: "the room".to_owned(),
      mark: "m".to_owned(),
    }),
    via: None,
    base_persona: None,
    privileged: Some(true),
    answers: vec![11],
    began: Some(1_000),
  };

  assert_eq!(agency.turn(&turn).await.unwrap(), Delivery::Blocked);
  assert_eq!(agency.state("agent-a@-7").await.unwrap().provider.as_deref(), Some("claude-code"));
  agency.fold("agent-a@-7").await.unwrap();
  assert_eq!(agency.sessions().await.unwrap(), [started]);
  assert_eq!(agency.forget_room(-7, true).await.unwrap().stayed[0].rows, 3);
  agency
    .register_hook("room-title", "http://receiver/agency/hooks/room-title", "t")
    .await
    .unwrap();
  assert_eq!(agency.run_log("run-1", Some(3)).await.unwrap()[0]["id"], 4);

  assert_eq!(
    unit.took(),
    [
      call("GET /agents", "", None),
      call(
        "POST /session/start",
        "",
        Some(json!({ "agent": "agent-a", "scope": { "kind": "room", "room": -7 } }))
      ),
      call(
        "POST /session/turn",
        "",
        Some(json!({
          "session": "agent-a@-7",
          "prompt": "words",
          "room": { "chatId": -7, "text": "the room", "mark": "m" },
          "privileged": true,
          "answers": [11],
          "began": 1000,
        }))
      ),
      call("GET /session/state", "session=agent-a%40-7", None),
      call("POST /session/fold", "", Some(json!({ "session": "agent-a@-7" }))),
      call("GET /session/list", "", None),
      call("POST /rooms/forget", "", Some(json!({ "chatId": -7, "dryRun": true }))),
      call(
        "POST /hooks",
        "",
        Some(
          json!({ "point": "room-title", "url": "http://receiver/agency/hooks/room-title", "token": "t" })
        )
      ),
      call("GET /run-log", "run=run-1&after=3", None),
    ]
  );
}

#[tokio::test]
async fn a_session_fault_comes_back_with_its_outcome_code() {
  let unit = StandIn::start().await;

  unit.answer(
    "POST /session/end",
    409,
    json!({ "code": "session-busy", "error": "The session is in a turn." }),
  );

  let failure = HttpAgency::new(&unit.peer("t")).end("agent-a@-7").await.unwrap_err();

  assert_eq!(failure.code(), Some("session-busy"));
}

#[tokio::test]
async fn the_event_stream_reads_each_event_by_its_kind_and_ends_with_the_stream() {
  let unit = StandIn::start().await;

  unit.answer_events(
    "GET /events",
    concat!(
      "event: turn-started\ndata: {\"session\":\"s\",\"run\":\"r\",\"at\":5}\n\n",
      ": a comment\n\n",
      "event: turn-ended\ndata: {\"session\":\"s\",\"run\":\"r\",\"outcome\":\"answered\",\"answer\":\"hi\"}\n\n",
    ),
  );

  let agency = HttpAgency::new(&unit.peer("t"));
  let events: Vec<Event> = agency
    .events(&["turn-started", "turn-ended"])
    .await
    .unwrap()
    .map(|event| event.unwrap())
    .collect()
    .await;

  assert_eq!(
    events,
    [
      Event {
        kind: "turn-started".to_owned(),
        data: json!({ "session": "s", "run": "r", "at": 5 }),
      },
      Event {
        kind: "turn-ended".to_owned(),
        data: json!({ "session": "s", "run": "r", "outcome": "answered", "answer": "hi" }),
      },
    ]
  );
  assert_eq!(unit.took(), [call("GET /events", "kinds=turn-started%2Cturn-ended", None)]);
}

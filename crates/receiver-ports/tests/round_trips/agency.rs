//! The agency port against a stand-in of the agency's routes (the agency's `docs/api.md`).

use futures_util::StreamExt;
use receiver_ports::agency::{
  Agency, CompanionRemoval, CompanionRow, Delivery, Event, FieldRefusal, HttpAgency, RestKind, Said, Scope,
  Turn, TurnRoom,
};
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
  unit.answer("GET /reset-point", 200, json!({ "point": 2_000 }));

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
  assert_eq!(agency.reset_point("agent-a").await.unwrap(), Some(2_000));

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
      call("GET /reset-point", "agent=agent-a", None),
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

#[tokio::test]
async fn a_companion_is_added_and_removed_on_the_routes_the_agency_serves_and_a_refused_field_comes_back() {
  let unit = StandIn::start().await;
  let agency = HttpAgency::new(&unit.peer("t"));
  let row = CompanionRow {
    name: "companion-a".to_owned(),
    label: "Companion A".to_owned(),
    provider: "provider-a".to_owned(),
    model: "model-a".to_owned(),
    worker: "worker-a".to_owned(),
  };
  let removal = CompanionRemoval {
    agent: "companion-a".to_owned(),
    rooms: vec![-7, -8],
    by: Some("tester".to_owned()),
  };

  unit.answer("POST /companions/add", 200, json!({ "ok": true }));
  unit.answer("POST /companions/remove", 200, json!({ "ok": true }));

  assert_eq!(agency.add_companion(&row).await.unwrap(), Ok(()));
  assert_eq!(agency.remove_companion(&removal).await.unwrap(), Ok(()));
  assert_eq!(
    agency
      .remove_companion(&CompanionRemoval {
        by: None,
        ..removal.clone()
      })
      .await
      .unwrap(),
    Ok(()),
    "an asker the console does not name is left out, for the agency's own default"
  );

  unit.answer(
    "POST /companions/add",
    400,
    json!({ "field": "model", "error": "The model has no address." }),
  );
  unit.answer(
    "POST /companions/remove",
    400,
    json!({ "field": "agent", "error": "companion-a is not a companion." }),
  );

  let refused = FieldRefusal {
    field: "model".to_owned(),
    error: "The model has no address.".to_owned(),
  };

  assert_eq!(agency.add_companion(&row).await.unwrap(), Err(refused));
  assert_eq!(agency.remove_companion(&removal).await.unwrap().unwrap_err().field, "agent");

  unit.answer("POST /companions/add", 400, json!({ "code": "bad-request", "error": "Not JSON." }));

  let failure = agency.add_companion(&row).await.unwrap_err();

  assert_eq!(
    (failure.status(), failure.code()),
    (Some(400), Some("bad-request")),
    "a 400 that names no field is the call's failure"
  );

  let added = json!({
    "name": "companion-a",
    "label": "Companion A",
    "provider": "provider-a",
    "model": "model-a",
    "worker": "worker-a",
  });
  let removed = json!({ "agent": "companion-a", "rooms": [-7, -8], "by": "tester" });

  assert_eq!(
    unit.took(),
    [
      call("POST /companions/add", "", Some(added.clone())),
      call("POST /companions/remove", "", Some(removed.clone())),
      call(
        "POST /companions/remove",
        "",
        Some(json!({ "agent": "companion-a", "rooms": [-7, -8] }))
      ),
      call("POST /companions/add", "", Some(added.clone())),
      call("POST /companions/remove", "", Some(removed)),
      call("POST /companions/add", "", Some(added)),
    ]
  );
}

#[tokio::test]
async fn words_pushed_into_a_working_turn_read_each_answer_the_agency_gives() {
  let unit = StandIn::start().await;
  let agency = HttpAgency::new(&unit.peer("t"));

  for (said, read) in
    [("delivered", Said::Delivered), ("waiting", Said::Waiting), ("refused", Said::Refused)]
  {
    unit.answer("POST /session/push", 200, json!({ "said": said }));

    assert_eq!(agency.push("agent-a@-7", "more words").await.unwrap(), read);
  }

  unit.answer(
    "POST /session/push",
    404,
    json!({ "code": "unknown-session", "error": "No such session." }),
  );

  assert_eq!(
    agency.push("agent-a@-7", "more words").await.unwrap_err().code(),
    Some("unknown-session")
  );
  assert_eq!(
    unit.took()[0],
    call(
      "POST /session/push",
      "",
      Some(json!({ "session": "agent-a@-7", "words": "more words" }))
    )
  );
}

#[tokio::test]
async fn a_reset_point_and_a_passed_rest_read_their_empty_answers_and_a_missing_agent_is_refused() {
  let unit = StandIn::start().await;
  let agency = HttpAgency::new(&unit.peer("t"));

  unit.answer("GET /reset-point", 200, json!({ "point": null }));
  unit.answer("GET /rest/passed", 200, json!({ "kind": "limit" }));

  assert_eq!(agency.reset_point("agent-a").await.unwrap(), None);
  assert_eq!(agency.passed_rest().await.unwrap().kind, Some(RestKind::Limit));

  unit.answer("GET /rest/passed", 200, json!({ "kind": null }));

  assert_eq!(agency.passed_rest().await.unwrap().kind, None);

  unit.answer("GET /reset-point", 400, json!({ "error": "No agent is named." }));

  assert_eq!(agency.reset_point("").await.unwrap_err().status(), Some(400));
  assert_eq!(
    unit.took(),
    [
      call("GET /reset-point", "agent=agent-a", None),
      call("GET /rest/passed", "", None),
      call("GET /rest/passed", "", None),
      call("GET /reset-point", "agent=", None),
    ]
  );
}

//! The agency port against a stand-in of the agency's routes (the agency's `docs/api.md`).

use futures_util::StreamExt;
use receiver_ports::agency::{
  Agency, ConsoleAgentRow, Delivery, Event, FieldRefusal, Forgotten, HttpAgency, Load, RestKind, Resting,
  Run, Said, Scope, Turn, TurnRead, TurnRoom,
};
use serde_json::json;

use crate::stand_in::{StandIn, call};

#[tokio::test]
async fn every_agency_call_goes_out_as_its_document_states_and_its_answer_reads() {
  let unit = StandIn::start().await;
  let agency = HttpAgency::new(&unit.peer("t"));
  let session = json!({
    "id": "agent-a@-7",
    "agentName": "agent-a",
    "scope": "room",
    "roomId": -7,
    "provider": "claude-code",
    "model": "m",
    "place": null,
    "generation": 2,
    "context": { "carried": 90_000, "window": 1_000_000 },
    "foldRatio": 0.15,
    "supervisor": "agent-a",
    "role": null,
  });

  unit.answer(
    "GET /supervisors",
    200,
    json!({
      "items": [{ "name": "agent-a", "awake": false, "brainFolder": "", "repository": "repo-a" }],
      "more": false,
    }),
  );
  unit.answer(
    "GET /roles",
    200,
    json!({
      "roles": [{ "name": "video", "label": "Video", "description": null, "bundles": [
        { "name": "room.send", "code": "room-send", "label": "Send" },
      ] }],
      "bundles": [],
      "lines": [],
    }),
  );
  unit.answer(
    "GET /repositories",
    200,
    json!({ "items": [{ "name": "repo-a", "label": "Repo A", "checkouts": [] }], "more": false }),
  );
  unit.answer("GET /stand-in", 200, json!({ "answers": true }));
  unit.answer("POST /sessions", 201, session.clone());
  unit.answer("POST /sessions/agent-a@-7/turns", 201, json!({ "delivery": "blocked" }));
  unit.answer("GET /sessions/agent-a@-7", 200, session.clone());
  unit.answer("POST /sessions/agent-a@-7/folds", 201, json!({ "sessionId": "agent-a@-7", "at": 5 }));
  unit.answer("GET /sessions", 200, json!({ "items": [session], "more": false }));
  unit.answer_empty("DELETE /room-holdings/-7", 204);
  unit.answer_empty("PUT /hooks/room-title", 200);
  unit.answer(
    "GET /log-lines",
    200,
    json!({ "items": [{ "id": 4, "text": "a line" }], "more": false }),
  );
  unit.answer("GET /runs/7", 200, json!({ "id": 7, "startedAt": 1_700_000 }));
  unit.answer(
    "GET /runs",
    200,
    json!({ "items": [{ "id": 9, "round": "round:-7:1:1" }, { "id": 8, "round": "round:-7:1:1" }], "more": false }),
  );
  unit.answer("PATCH /runs/9", 200, json!({ "id": 9, "messageId": 12 }));

  let agents = agency.supervisors().await.unwrap();

  assert_eq!(agents.agents[0].name, "agent-a");
  assert!(!agents.agents[0].awake);
  assert_eq!(agents.agents[0].brain_folder.as_deref(), Some(""));
  assert_eq!(agents.agents[0].row["repository"], "repo-a", "the rest of the row is kept as it came");

  let roles = agency.roles().await.unwrap();

  assert_eq!((roles[0].name.as_str(), roles[0].label.as_deref()), ("video", Some("Video")));
  assert!(roles[0].gives("room.send") && !roles[0].gives("graph.read"));
  assert_eq!(agency.repositories().await.unwrap()[0].name, "repo-a");
  assert!(agency.stand_in("agent-a").await.unwrap());

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
    privileged: Some(true),
    answers: vec![11],
    began: Some(1_000),
    round: Some("round:-7:1:1".to_owned()),
  };

  assert_eq!(agency.turn(&turn).await.unwrap(), Delivery::Blocked);
  let state = agency.state("agent-a@-7").await.unwrap();

  assert_eq!(state.provider.as_deref(), Some("claude-code"));
  assert_eq!((state.supervisor.as_deref(), state.role.as_deref()), (Some("agent-a"), None));
  assert_eq!(
    (state.context["carried"].as_i64(), state.context["window"].as_i64(), state.fold_ratio),
    (Some(90_000), Some(1_000_000), Some(0.15)),
    "what the session carries comes through, so its fold is judged from it"
  );
  agency.fold("agent-a@-7").await.unwrap();
  assert_eq!(agency.sessions().await.unwrap(), [started]);
  assert_eq!(
    agency.forget_room(-7, false).await.unwrap(),
    Forgotten::default(),
    "a removal answers nothing"
  );
  unit.answer(
    "DELETE /room-holdings/-7",
    200,
    json!({ "gone": { "room_sessions": 1 }, "stayed": [{ "table": "agent_log", "rows": 3, "why": "the job's" }] }),
  );
  assert_eq!(agency.forget_room(-7, true).await.unwrap().stayed[0].rows, 3);
  agency
    .register_hook("room-title", "http://receiver/agency/hooks/room-title", "t")
    .await
    .unwrap();
  assert_eq!(agency.run_log("run-1", Some(3)).await.unwrap()[0]["id"], 4);
  assert_eq!(
    agency.run(7).await.unwrap(),
    Some(Run {
      started_at: 1_700_000
    })
  );
  assert_eq!(
    agency.round_runs("round:-7:1:1").await.unwrap(),
    [9, 8],
    "newest first, as the agency answers"
  );
  agency.attach_message(9, 12).await.unwrap();

  assert_eq!(
    unit.took(),
    [
      call("GET /supervisors", "", None),
      call("GET /roles", "", None),
      call("GET /repositories", "", None),
      call("GET /stand-in", "agent=agent-a", None),
      call(
        "POST /sessions",
        "",
        Some(json!({ "agentName": "agent-a", "scope": "room", "roomId": -7 }))
      ),
      call(
        "POST /sessions/agent-a@-7/turns",
        "",
        Some(json!({
          "prompt": "words",
          "room": { "chatId": -7, "text": "the room", "mark": "m" },
          "privileged": true,
          "answers": [11],
          "began": 1000,
          "round": "round:-7:1:1",
        }))
      ),
      call("GET /sessions/agent-a@-7", "", None),
      call("POST /sessions/agent-a@-7/folds", "", Some(json!({}))),
      call("GET /sessions", "", None),
      call("DELETE /room-holdings/-7", "", None),
      call("DELETE /room-holdings/-7", "dryRun=true", None),
      call(
        "PUT /hooks/room-title",
        "",
        Some(json!({ "url": "http://receiver/agency/hooks/room-title", "token": "t" }))
      ),
      call("GET /log-lines", "run=run-1&after=3", None),
      call("GET /runs/7", "", None),
      call("GET /runs", "round=round%3A-7%3A1%3A1", None),
      call("PATCH /runs/9", "", Some(json!({ "messageId": 12 }))),
    ]
  );
}

#[tokio::test]
async fn a_session_fault_comes_back_with_its_outcome_code() {
  let unit = StandIn::start().await;

  unit.answer(
    "DELETE /sessions/agent-a@-7",
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
      "event: turn.created\ndata: {\"id\":1,\"kind\":\"turn.created\",\"at\":7,\"data\":{\"session\":\"s\",\"run\":\"r\",\"turnId\":2,\"at\":5}}\n\n",
      ": a comment\n\n",
      "event: turn.ended\ndata: {\"id\":2,\"kind\":\"turn.ended\",\"at\":9,\"data\":{\"session\":\"s\",\"run\":\"r\",\"outcome\":\"answered\",\"answer\":\"hi\"}}\n\n",
    ),
  );

  let agency = HttpAgency::new(&unit.peer("t"));
  let events: Vec<Event> = agency
    .events(&["turn.created", "turn.ended"])
    .await
    .unwrap()
    .map(|event| event.unwrap())
    .collect()
    .await;

  assert_eq!(
    events,
    [
      Event {
        kind: "turn.created".to_owned(),
        data: json!({ "session": "s", "run": "r", "turnId": 2, "at": 5 }),
        id: Some(1),
        at: Some(7),
      },
      Event {
        kind: "turn.ended".to_owned(),
        data: json!({ "session": "s", "run": "r", "outcome": "answered", "answer": "hi" }),
        id: Some(2),
        at: Some(9),
      },
    ]
  );
  assert_eq!(unit.took(), [call("GET /events", "kinds=turn.created%2Cturn.ended", None)]);
}

#[tokio::test]
async fn an_agent_is_added_and_removed_on_the_routes_the_agency_serves_and_a_refused_field_comes_back() {
  let unit = StandIn::start().await;
  let agency = HttpAgency::new(&unit.peer("t"));
  let row = ConsoleAgentRow {
    name: "agent-d".to_owned(),
    label: "Agent D".to_owned(),
    provider: Some("provider-a".to_owned()),
    model: Some("model-a".to_owned()),
    worker: "worker-a".to_owned(),
    order: Some(json!([{ "provider": "provider-a", "model": "model-a", "effort": "high" }])),
    quota_share: Some(json!(0.2)),
  };
  unit.answer("POST /companions", 201, json!({ "ok": true }));
  unit.answer_empty("DELETE /companions/agent-d", 204);

  assert_eq!(agency.add_console_agent(&row).await.unwrap(), Ok(()));
  assert_eq!(agency.remove_console_agent("agent-d").await.unwrap(), Ok(()));
  assert_eq!(agency.remove_console_agent("agent-d").await.unwrap(), Ok(()));

  unit.answer(
    "POST /companions",
    400,
    json!({ "field": "model", "error": "The model has no address." }),
  );
  unit.answer(
    "DELETE /companions/agent-d",
    400,
    json!({ "field": "agent", "error": "agent-d is not an agent added on the console." }),
  );

  let refused = FieldRefusal {
    field: "model".to_owned(),
    error: "The model has no address.".to_owned(),
  };

  assert_eq!(agency.add_console_agent(&row).await.unwrap(), Err(refused));
  assert_eq!(agency.remove_console_agent("agent-d").await.unwrap().unwrap_err().field, "agent");

  unit.answer("POST /companions", 400, json!({ "code": "bad-request", "error": "Not JSON." }));

  let failure = agency.add_console_agent(&row).await.unwrap_err();

  assert_eq!(
    (failure.status(), failure.code()),
    (Some(400), Some("bad-request")),
    "a 400 that names no field is the call's failure"
  );

  let added = json!({
    "name": "agent-d",
    "label": "Agent D",
    "provider": "provider-a",
    "model": "model-a",
    "worker": "worker-a",
    "order": [{ "provider": "provider-a", "model": "model-a", "effort": "high" }],
    "quotaShare": 0.2,
  });

  assert_eq!(
    unit.took(),
    [
      call("POST /companions", "", Some(added.clone())),
      call("DELETE /companions/agent-d", "", None),
      call("DELETE /companions/agent-d", "", None),
      call("POST /companions", "", Some(added.clone())),
      call("DELETE /companions/agent-d", "", None),
      call("POST /companions", "", Some(added)),
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
    unit.answer("POST /sessions/agent-a@-7/turns/4/inputs", 201, json!({ "said": said }));

    assert_eq!(agency.push("agent-a@-7", 4, "more words").await.unwrap(), read);
  }

  unit.answer(
    "POST /sessions/agent-a@-7/turns/4/inputs",
    404,
    json!({ "code": "unknown-session", "error": "No such session." }),
  );

  assert_eq!(
    agency.push("agent-a@-7", 4, "more words").await.unwrap_err().code(),
    Some("unknown-session")
  );
  assert_eq!(
    unit.took()[0],
    call("POST /sessions/agent-a@-7/turns/4/inputs", "", Some(json!({ "words": "more words" })))
  );
}

#[tokio::test]
async fn a_passed_rest_reads_its_empty_answer_and_a_stand_in_for_no_agent_is_refused() {
  let unit = StandIn::start().await;
  let agency = HttpAgency::new(&unit.peer("t"));

  let cleared = json!({ "clearPassed": true, "until": null, "kind": null, "account": null });

  unit.answer(
    "PATCH /rest",
    200,
    json!({ "resting": false, "until": null, "kind": "limit", "accountId": null }),
  );

  assert_eq!(agency.passed_rest().await.unwrap().kind, Some(RestKind::Limit));

  unit.answer(
    "PATCH /rest",
    200,
    json!({ "resting": false, "until": null, "kind": null, "accountId": null }),
  );

  assert_eq!(agency.passed_rest().await.unwrap().kind, None);

  unit.answer(
    "GET /stand-in",
    404,
    json!({ "code": "not-found", "error": "No agent is named no-agent." }),
  );

  assert_eq!(agency.stand_in("no-agent").await.unwrap_err().status(), Some(404));
  assert_eq!(
    unit.took(),
    [
      call("PATCH /rest", "", Some(cleared.clone())),
      call("PATCH /rest", "", Some(cleared)),
      call("GET /stand-in", "agent=no-agent", None),
    ]
  );
}

#[tokio::test]
async fn a_rest_is_written_and_its_silence_read_across_the_one_patch_route() {
  let unit = StandIn::start().await;
  let agency = HttpAgency::new(&unit.peer("t"));
  let resting = Resting {
    until: 1_900_000_000,
    kind: RestKind::Limit,
    account: Some("account-a".to_owned()),
  };

  unit.answer(
    "PATCH /rest",
    200,
    json!({ "resting": true, "until": 1_900_000_000, "kind": "limit", "accountId": "account-a", "silenced": true }),
  );

  assert!(agency.write_rest(&resting).await.unwrap().silenced);
  assert_eq!(
    unit.took(),
    [call(
      "PATCH /rest",
      "",
      Some(
        json!({ "clearPassed": false, "until": 1_900_000_000, "kind": "limit", "account": "account-a" })
      )
    )]
  );
}

#[tokio::test]
async fn a_run_reads_the_moment_it_began_and_one_the_agency_holds_none_of_reads_as_none() {
  let unit = StandIn::start().await;
  let agency = HttpAgency::new(&unit.peer("t"));

  unit.answer("GET /runs/7", 200, json!({ "id": 7, "agentName": "agent-a", "startedAt": 1_700_000 }));
  unit.answer("GET /runs/8", 404, json!({ "error": "No run is numbered 8." }));

  assert_eq!(
    agency.run(7).await.unwrap(),
    Some(Run {
      started_at: 1_700_000
    })
  );
  assert_eq!(agency.run(8).await.unwrap(), None);
  assert_eq!(unit.took(), [call("GET /runs/7", "", None), call("GET /runs/8", "", None)]);
}

#[tokio::test]
async fn a_sessions_loads_are_read_kept_whole_and_forgotten_by_its_id() {
  let unit = StandIn::start().await;
  let agency = HttpAgency::new(&unit.peer("t"));
  let load = |path: &str, version: &str| Load {
    path: path.to_owned(),
    version: version.to_owned(),
  };
  let documents =
    json!([{ "path": "docs/a.md", "version": "v1" }, { "path": "room-context/-7", "version": "f" }]);

  unit.answer("GET /session-loads/s-1", 200, json!({ "sessionId": "s-1", "documents": documents }));
  unit.answer("PUT /session-loads/s-1", 200, json!({ "sessionId": "s-1", "documents": documents }));
  unit.answer_empty("DELETE /session-loads/s-1", 204);

  let loads = agency.loads("s-1").await.unwrap();

  assert_eq!(loads, [load("docs/a.md", "v1"), load("room-context/-7", "f")]);

  agency.keep_loads("s-1", &loads).await.unwrap();
  agency.forget_loads("s-1").await.unwrap();

  assert_eq!(
    unit.took(),
    [
      call("GET /session-loads/s-1", "", None),
      call("PUT /session-loads/s-1", "", Some(json!({ "documents": documents }))),
      call("DELETE /session-loads/s-1", "", None),
    ]
  );
}

#[tokio::test]
async fn the_runs_open_and_the_jobs_running_are_read_off_the_live_lists_newest_first() {
  use receiver_ports::agency::{OpenRun, RunningJob};

  let unit = StandIn::start().await;
  let agency = HttpAgency::new(&unit.peer("t"));

  unit.answer(
    "GET /runs",
    200,
    json!({ "items": [
      { "id": 12, "name": "agent-a:chat:-7:2", "startedAt": 2_000, "jobId": null, "state": "open" },
      { "id": 11, "name": "unit-a:research-1", "startedAt": 1_000, "state": "open" },
    ], "more": false }),
  );
  unit.answer(
    "GET /jobs",
    200,
    json!({ "items": [
      { "id": 906_425, "roleName": "worker", "repositoryName": "repo-a", "workerName": "repo-a-w2", "itemId": 2283, "startedAt": 3_000, "state": "running" },
      { "id": 906_426, "roleName": "builder", "repositoryName": null, "workerName": "gone-w1", "itemId": null, "startedAt": 3_100, "state": "starting" },
    ], "more": false }),
  );

  assert_eq!(
    agency.open_runs().await.unwrap(),
    [
      OpenRun {
        id: 12,
        name: "agent-a:chat:-7:2".to_owned(),
        started_at: 2_000,
        job_id: None
      },
      OpenRun {
        id: 11,
        name: "unit-a:research-1".to_owned(),
        started_at: 1_000,
        job_id: None
      },
    ]
  );
  assert_eq!(
    agency.running_jobs().await.unwrap(),
    [
      RunningJob {
        id: 906_425,
        role_name: Some("worker".to_owned()),
        repository_name: Some("repo-a".to_owned()),
        worker_name: Some("repo-a-w2".to_owned()),
        item_id: Some(2283),
        started_at: Some(3_000)
      },
      RunningJob {
        id: 906_426,
        role_name: Some("builder".to_owned()),
        repository_name: None,
        worker_name: Some("gone-w1".to_owned()),
        item_id: None,
        started_at: Some(3_100)
      }
    ],
    "each job names its repository beside its checkout, none for a checkout no repository holds"
  );
  assert_eq!(
    unit.took(),
    [
      call("GET /runs", "live=true&limit=500", None),
      call("GET /jobs", "running=true&limit=500", None)
    ]
  );
}

#[tokio::test]
async fn the_kept_events_are_read_after_a_number_and_a_turn_is_read_by_its_id() {
  let unit = StandIn::start().await;

  unit.answer(
    "GET /events",
    200,
    json!({
      "items": [{ "id": 5, "kind": "rest.changed", "at": 500, "data": { "resting": false } }],
      "more": false,
    }),
  );
  unit.answer(
    "GET /sessions/agent-a@agent/turns/3",
    200,
    json!({ "id": 3, "state": "ended", "outcome": "answered", "answer": "hi", "text": "hi" }),
  );

  let agency = HttpAgency::new(&unit.peer("t"));
  let kept = agency.kept_events(Some(4)).await.unwrap();
  let read = agency.turn_read("agent-a@agent", 3).await.unwrap();

  assert_eq!(
    kept.items,
    [Event {
      kind: "rest.changed".to_owned(),
      data: json!({ "resting": false }),
      id: Some(5),
      at: Some(500),
    }]
  );
  assert!(!kept.more);
  assert_eq!(
    read,
    TurnRead {
      state: "ended".to_owned(),
      outcome: Some("answered".to_owned()),
      answer: Some("hi".to_owned()),
    }
  );
  assert!(read.ended());
  assert_eq!(
    unit.took(),
    [
      call("GET /events", "limit=1000&after=4", None),
      call("GET /sessions/agent-a@agent/turns/3", "", None)
    ]
  );
}

#[tokio::test]
async fn one_supervisor_is_read_by_its_name_with_its_seeding_and_none_where_none_is() {
  let unit = StandIn::start().await;
  let agency = HttpAgency::new(&unit.peer("t"));

  unit.answer(
    "GET /supervisors/agent-a",
    200,
    json!({ "name": "agent-a", "label": "Agent A", "seeded": true, "resetPoint": 1_700_000_000_123_i64 }),
  );
  unit.answer(
    "GET /supervisors/agent-b",
    200,
    json!({ "name": "agent-b", "label": "Agent B", "seeded": false, "resetPoint": null }),
  );
  unit.answer(
    "GET /supervisors/agent-c",
    404,
    json!({ "code": "not-found", "error": "no such supervisor" }),
  );

  let seeded = agency.supervisor("agent-a").await.unwrap().expect("agent-a stands");
  let added = agency.supervisor("agent-b").await.unwrap().expect("agent-b stands");

  assert!(seeded.seeded());
  assert!(!added.seeded());
  assert_eq!(
    agency.supervisor("agent-c").await.unwrap(),
    None,
    "no supervisor of the name reads as none"
  );
  assert_eq!(
    unit.took(),
    [
      call("GET /supervisors/agent-a", "", None),
      call("GET /supervisors/agent-b", "", None),
      call("GET /supervisors/agent-c", "", None),
    ]
  );
}

#[tokio::test]
async fn a_supervisors_reset_point_is_read_alone_and_none_where_no_supervisor_has_the_name() {
  let unit = StandIn::start().await;
  let agency = HttpAgency::new(&unit.peer("t"));

  unit.answer(
    "GET /supervisors/agent-a/reset-point",
    200,
    json!({ "supervisor": "agent-a", "resetPoint": 1_700_000_000_123_i64 }),
  );
  unit.answer(
    "GET /supervisors/agent-b/reset-point",
    200,
    json!({ "supervisor": "agent-b", "resetPoint": null }),
  );
  unit.answer(
    "GET /supervisors/agent-c/reset-point",
    404,
    json!({ "code": "not-found", "error": "no such supervisor" }),
  );

  assert_eq!(
    agency.reset_point("agent-a").await.unwrap().and_then(|point| point.reset_point),
    Some(1_700_000_000_123)
  );
  assert_eq!(
    agency.reset_point("agent-b").await.unwrap().expect("agent-b stands").reset_point,
    None,
    "a supervisor never reset names no point"
  );
  assert_eq!(
    agency.reset_point("agent-c").await.unwrap(),
    None,
    "no supervisor of the name reads as none"
  );
  assert_eq!(
    unit.took(),
    [
      call("GET /supervisors/agent-a/reset-point", "", None),
      call("GET /supervisors/agent-b/reset-point", "", None),
      call("GET /supervisors/agent-c/reset-point", "", None),
    ]
  );
}

#[tokio::test]
async fn the_sessions_listed_read_each_scope_the_agency_names() {
  let unit = StandIn::start().await;
  let agency = HttpAgency::new(&unit.peer("t"));
  let member = |id: &str, agent: &str, scope: &str, room: Option<i64>| {
    let mut member = json!({ "id": id, "agentName": agent, "scope": scope });

    member["roomId"] = json!(room);
    member
  };

  unit.answer(
    "GET /sessions",
    200,
    json!({ "items": [
      member("agent-a", "agent-a", "agent", None),
      member("agent-a@-7", "agent-a", "room", Some(-7)),
      member("builder@run:4", "builder", "role", None),
      member("agent-a@conversation:2", "agent-a", "conversation", None),
    ], "more": false }),
  );

  let scopes: Vec<Scope> =
    agency.sessions().await.unwrap().into_iter().map(|session| session.scope).collect();

  assert_eq!(
    scopes,
    [Scope::Agent, Scope::Room { room: -7 }, Scope::Role, Scope::Conversation],
    "a role's run is listed as a role's session, never as an agent's"
  );
  assert_eq!(
    [Scope::Agent, Scope::Room { room: -7 }, Scope::Role, Scope::Conversation].map(|scope| scope.word()),
    ["agent", "room", "role", "conversation"]
  );
}

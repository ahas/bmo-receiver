//! The instruction queue answered where the receiver serves it (`docs/boundary-api.md`, 「For bmo-job」, 「For
//! bmo-artifact」 and 「For the brain」): the rows each route writes, what it tells the instruction pass, and how
//! far a row has gone. The tests stand on the receiver's shared schema, writing rows of their own; the queue's own
//! limits are bmo-job's, read through its port.

use axum::http::StatusCode;
use sea_orm::Value;
use serde_json::json;

use crate::common::Scratch;
use crate::common::queue::{Front, get, post, put, remove, stood};
use crate::common::units::{AGENT, OWNER, registration};

// How many instruction rows hold every one of these pieces, in whatever order their facts are written.
async fn told(scratch: &Scratch, pieces: &[String]) -> i64 {
  let wheres = (1..=pieces.len()).map(|at| format!(r#""text" LIKE ${at}"#)).collect::<Vec<String>>();

  scratch
    .count(
      &format!(r#"SELECT COUNT(*) FROM "instructions" WHERE {}"#, wheres.join(" AND ")),
      pieces.iter().map(|piece| format!("%{piece}%").into()).collect(),
    )
    .await
}

async fn rows(scratch: &Scratch, sql: &str, values: Vec<Value>) -> i64 {
  scratch.count(sql, values).await
}

async fn texts(scratch: &Scratch, sql: &str) -> Vec<String> {
  scratch.texts(sql).await
}

// The room's registration standing, as the receiver writes it on first seeing the room.
async fn registered(scratch: &Scratch, chat_id: i64) {
  scratch
    .run(
      r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES ($1, 'a room')"#,
      vec![chat_id.into()],
    )
    .await;
}

// The agents the agency registers, the first the default answerer and every one in `resting` asleep, and the
// room naming `named` among its answerers.
async fn answering(front: &Front, scratch: &Scratch, chat_id: i64, named: &[&str], resting: &[&str]) {
  let mut agents = [registration(AGENT, "Agent A", ["a"].to_vec(), None, true)].to_vec();

  for name in ["agent-b", "agent-c"] {
    let mut agent = registration(name, name, [name].to_vec(), None, false);

    agent.awake = !resting.contains(&name);
    agents.push(agent);
  }

  front.standing.agency.registering(agents);

  for name in named {
    scratch
      .run(
        r#"INSERT INTO "room_answerers" ("chat_id", "agent") VALUES ($1, $2)"#,
        vec![chat_id.into(), (*name).into()],
      )
      .await;
  }
}

#[tokio::test]
async fn an_order_queued_for_a_room_writes_its_row_and_is_read_back_as_pending() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  registered(&scratch, 7).await;

  let (made, queued) = post(
    &front,
    "/instructions",
    json!({
      "agent": AGENT, "chat": 7, "said": { "order": "check the queue and say nothing." },
      "sort": "job", "summary": "the queue standing",
    }),
  )
  .await;

  assert_eq!(made, StatusCode::CREATED);
  let id = queued["id"].as_i64().unwrap();

  assert_eq!(
    get(&front, &format!("/instructions/{id}")).await,
    (StatusCode::OK, json!({ "state": "pending" })),
  );
  assert_eq!(
    rows(
      &scratch,
      r#"SELECT COUNT(*) FROM "instructions" WHERE "id" = $1 AND "sort" = 'job' AND "agent" = $2"#,
      vec![id.into(), AGENT.into()]
    )
    .await,
    1,
  );
  assert_eq!(
    texts(&scratch, &format!(r#"SELECT "text" FROM "instructions" WHERE "id" = {id}"#)).await,
    ["check the queue and say nothing."],
  );
  assert_eq!(front.kicked.times(), 0);
}

#[tokio::test]
async fn an_instruction_withdrawn_before_a_took_it_leaves_nothing_and_a_handed_one_is_not_withdrawn() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  registered(&scratch, 7).await;

  let (_, queued) = post(
    &front,
    "/instructions",
    json!({ "chat": 7, "said": { "order": "an order for the room." }, "sort": "job" }),
  )
  .await;
  let id = queued["id"].as_i64().unwrap();

  assert_eq!(
    remove(&front, &format!("/instructions/{id}")).await,
    (StatusCode::OK, json!({ "withdrawn": true }))
  );
  assert_eq!(
    rows(&scratch, r#"SELECT COUNT(*) FROM "instructions" WHERE "id" = $1"#, vec![id.into()]).await,
    0
  );
  assert_eq!(get(&front, &format!("/instructions/{id}")).await.0, StatusCode::NOT_FOUND);

  let (_, queued) = post(
    &front,
    "/instructions",
    json!({ "chat": 7, "said": { "order": "an order a turn took." }, "sort": "job" }),
  )
  .await;
  let handed = queued["id"].as_i64().unwrap();

  scratch
    .run(r#"UPDATE "instructions" SET "handed_at" = 1 WHERE "id" = $1"#, vec![handed.into()])
    .await;

  assert_eq!(
    remove(&front, &format!("/instructions/{handed}")).await,
    (StatusCode::OK, json!({ "withdrawn": false })),
  );
  assert_eq!(
    get(&front, &format!("/instructions/{handed}")).await,
    (StatusCode::OK, json!({ "state": "handed" })),
  );
}

#[tokio::test]
async fn a_fact_queued_for_a_room_is_framed_for_the_rooms_voice_to_say() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  registered(&scratch, 7).await;

  let (_, queued) = post(
    &front,
    "/instructions",
    json!({ "chat": 7, "said": { "fact": { "code": "job-ended", "item": 12 } }, "sort": "job" }),
  )
  .await;
  let id = queued["id"].as_i64().unwrap();

  assert_eq!(
    rows(
      &scratch,
      r#"SELECT COUNT(*) FROM "instructions" WHERE "id" = $1 AND "chat_id" = $2"#,
      vec![id.into(), 7.into()]
    )
    .await,
    1,
  );
  assert_eq!(
    told(&scratch, &[r#""code":"job-ended""#.to_owned(), "END OF FACTS".to_owned()]).await,
    1,
  );
  assert!(
    texts(&scratch, &format!(r#"SELECT "text" FROM "instructions" WHERE "id" = {id}"#))
      .await
      .first()
      .cloned()
      .unwrap_or_default()
      .contains("tell this chat what the system recorded."),
  );
}

#[tokio::test]
async fn a_runs_result_queued_for_its_room_keeps_the_agent_that_asked_its_road_and_its_run() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  registered(&scratch, 7).await;
  answering(&front, &scratch, 7, &["agent-b"], &[]).await;

  let (made, queued) = post(
    &front,
    "/instructions",
    json!({
      "agent": "agent-b", "chat": 7, "said": { "order": "the search came back." },
      "sort": "search", "via": "road-a", "run": 31,
    }),
  )
  .await;

  assert_eq!(made, StatusCode::CREATED);
  assert_eq!(
    rows(
      &scratch,
      r#"SELECT COUNT(*) FROM "instructions" WHERE "id" = $1 AND "chat_id" = 7 AND "agent" = 'agent-b'
           AND "via" = 'road-a' AND "run_id" = 31 AND "sort" = 'search' AND "text" = 'the search came back.'"#,
      vec![queued["id"].as_i64().unwrap().into()],
    )
    .await,
    1,
  );
}

// The row each of these asks writes for the room: whose it is, with the road, the run and the words it was given.
async fn queued_as(scratch: &Scratch, id: i64, agent: &str) -> i64 {
  rows(
    scratch,
    r#"SELECT COUNT(*) FROM "instructions" WHERE "id" = $1 AND "chat_id" = 7 AND "agent" = $2
         AND "via" = 'road-a' AND "run_id" = 31 AND "sort" = 'search' AND "text" = 'the search came back.'"#,
    vec![id.into(), agent.into()],
  )
  .await
}

async fn run_ending(front: &Front, asker: &str) -> i64 {
  let (made, queued) = post(
    front,
    "/instructions",
    json!({
      "agent": asker, "chat": 7, "said": { "order": "the search came back." },
      "sort": "search", "via": "road-a", "run": 31,
    }),
  )
  .await;

  assert_eq!(made, StatusCode::CREATED);

  queued["id"].as_i64().unwrap()
}

#[tokio::test]
async fn a_runs_result_whose_asker_no_longer_answers_a_room_naming_nobody_goes_to_the_default_answerer() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  registered(&scratch, 7).await;
  answering(&front, &scratch, 7, &[], &[]).await;

  let id = run_ending(&front, "agent-b").await;

  assert_eq!(queued_as(&scratch, id, AGENT).await, 1);
}

#[tokio::test]
async fn a_runs_result_whose_asker_rests_goes_to_the_first_awake_answerer_the_room_names() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  registered(&scratch, 7).await;
  answering(&front, &scratch, 7, &["agent-b", "agent-c"], &["agent-b"]).await;

  let resting = run_ending(&front, "agent-b").await;
  let gone = run_ending(&front, AGENT).await;

  assert_eq!(queued_as(&scratch, resting, "agent-c").await, 1);
  assert_eq!(queued_as(&scratch, gone, "agent-c").await, 1);
}

#[tokio::test]
async fn an_instruction_for_a_room_whose_registration_is_gone_goes_to_the_owners_chat_naming_the_room() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  front
    .standing
    .agency
    .registering([registration(AGENT, "Agent A", ["a"].to_vec(), None, true)].to_vec());

  // A row of the owner's own, of the kind the removed room's row names.
  let (_, own) = post(
    &front,
    "/instructions",
    json!({ "said": { "order": "the owner's own row." }, "sort": "job", "kind": "search-ended" }),
  )
  .await;
  let (made, queued) = post(
    &front,
    "/instructions",
    json!({
      "agent": "agent-b", "chat": 9, "said": { "order": "the search came back." },
      "sort": "search", "kind": "search-ended", "via": "road-a", "run": 31,
    }),
  )
  .await;

  assert_eq!(made, StatusCode::CREATED);
  let id = queued["id"].as_i64().unwrap();

  // It is the owner's chat's voice that takes it, on no road and of no kind, with its run kept.
  assert_eq!(
    rows(
      &scratch,
      r#"SELECT COUNT(*) FROM "instructions" WHERE "id" = $1 AND "chat_id" = $2 AND "agent" = $3
           AND "via" IS NULL AND "kind" IS NULL AND "run_id" = 31 AND "sort" = 'search'"#,
      vec![id.into(), OWNER.into(), AGENT.into()],
    )
    .await,
    1,
  );
  let text = texts(&scratch, &format!(r#"SELECT "text" FROM "instructions" WHERE "id" = {id}"#)).await;

  assert!(text[0].starts_with("This came back from work started in room 9,"), "{}", text[0]);
  assert!(text[0].ends_with("\n\nthe search came back."), "{}", text[0]);

  // Nothing is written for the removed room, and the owner's own row of that kind stands.
  assert_eq!(
    rows(&scratch, r#"SELECT COUNT(*) FROM "instructions" WHERE "chat_id" = 9"#, vec![]).await,
    0
  );
  assert_eq!(
    get(&front, &format!("/instructions/{}", own["id"].as_i64().unwrap())).await,
    (StatusCode::OK, json!({ "state": "pending" })),
  );
}

#[tokio::test]
async fn a_notice_voiced_in_a_room_is_queued_for_the_rooms_voice_and_replaces_one_of_the_same_kind() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  front
    .standing
    .agency
    .registering([registration(AGENT, "Agent A", ["a"].to_vec(), None, true)].to_vec());

  let (made, queued) = post(
    &front,
    "/rooms/7/notices",
    json!({
      "fact": { "code": "rest-lifted" },
      "intent": "tell the chat you are answering again.",
      "follow_up": "nothing follows.",
      "optional": false,
      "kind": "rest-lifted",
    }),
  )
  .await;

  assert_eq!(made, StatusCode::CREATED);
  assert_eq!(
    told(
      &scratch,
      &[
        r#""code":"rest-lifted""#.to_owned(),
        "tell the chat you are answering again.".to_owned(),
        "nothing follows.".to_owned(),
      ],
    )
    .await,
    1,
  );

  let (_, again) = post(
    &front,
    "/rooms/7/notices",
    json!({
      "fact": { "code": "rest-lifted" },
      "intent": "tell the chat you are answering again.",
      "optional": true,
      "kind": "rest-lifted",
    }),
  )
  .await;

  assert_eq!(
    rows(
      &scratch,
      r#"SELECT COUNT(*) FROM "instructions" WHERE "chat_id" = 7 AND "kind" = 'rest-lifted'"#,
      vec![]
    )
    .await,
    1,
  );
  assert_ne!(again["id"].as_i64().unwrap(), queued["id"].as_i64().unwrap());
  assert_eq!(front.kicked.times(), 0);
}

#[tokio::test]
async fn a_quiet_instruction_is_queued_under_the_sort_its_kind_names_and_wakes_the_rooms_turn() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  for kind in ["watch", "reminder"].to_vec() {
    let (made, _) = post(
      &front,
      "/instructions/quiet",
      json!({ "agent": AGENT, "chatId": 7, "text": "the memo the brain wrote.", "kind": kind }),
    )
    .await;

    assert_eq!(made, StatusCode::OK);
    assert_eq!(
      rows(
        &scratch,
        r#"SELECT COUNT(*) FROM "instructions" WHERE "sort" = $1 AND "text" = $2"#,
        vec![kind.into(), "the memo the brain wrote.".into()],
      )
      .await,
      1,
    );
  }

  // Read as a GET, the path names no instruction: the state route takes a number, and `quiet` is none.
  assert_eq!(get(&front, "/instructions/quiet").await.0, StatusCode::NOT_FOUND);
  assert_eq!(front.kicked.times(), 2);
}

#[tokio::test]
async fn an_owner_notice_is_queued_for_the_chat_agent_in_the_owners_private_chat() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;
  let (made, _) = post(
    &front,
    "/owner/notice",
    json!({ "code": "rule-candidate", "agent": "someone", "candidate": "a rule" }),
  )
  .await;

  assert_eq!(made, StatusCode::OK);
  assert_eq!(
    rows(
      &scratch,
      r#"SELECT COUNT(*) FROM "instructions" WHERE "chat_id" = $1 AND "kind" = 'rule-candidate'"#,
      vec![OWNER.into()],
    )
    .await,
    1,
  );
  assert_eq!(
    told(
      &scratch,
      &[
        r#""code":"rule-candidate""#.to_owned(),
        "tell the owner what the system recorded.".to_owned()
      ]
    )
    .await,
    1,
  );
  assert_eq!(front.kicked.times(), 1);
}

#[tokio::test]
async fn a_unit_declared_a_holder_keeps_its_declaration_and_declaring_again_replaces_it() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  assert_eq!(
    put(
      &front,
      "/room-holders/unit-a",
      json!({ "address": "http://127.0.0.1:9", "holdings": [{ "name": "rows", "effect": "kept" }] }),
    )
    .await,
    (StatusCode::NO_CONTENT, json!({})),
  );
  assert_eq!(
    rows(
      &scratch,
      r#"SELECT COUNT(*) FROM "settings" WHERE "key" = 'room_holder:unit-a'"#,
      vec![]
    )
    .await,
    1,
  );

  put(
    &front,
    "/room-holders/unit-a",
    json!({ "address": "http://127.0.0.1:8", "holdings": [{ "name": "rows", "effect": "warns" }] }),
  )
  .await;

  assert_eq!(
    rows(
      &scratch,
      r#"SELECT COUNT(*) FROM "settings" WHERE "key" = 'room_holder:unit-a'"#,
      vec![]
    )
    .await,
    1,
  );
  assert!(
    texts(&scratch, r#"SELECT "value" FROM "settings" WHERE "key" = 'room_holder:unit-a'"#)
      .await
      .first()
      .cloned()
      .unwrap_or_default()
      .contains("warns"),
  );
}

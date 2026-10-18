//! The instruction queue answered where the receiver serves it (`docs/boundary-api.md`, 「For bmo-job」 and
//! 「For the brain」): the rows each route writes, what it tells the instruction pass, and how far a row has
//! gone. The tests stand on the receiver's shared schema, writing rows of their own; the queue's own limits are
//! bmo-job's, read through its port.

use axum::http::StatusCode;
use sea_orm::Value;
use serde_json::json;

use crate::common::Scratch;
use crate::common::queue::{get, post, put, remove, stood};
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

#[tokio::test]
async fn an_order_queued_for_a_room_writes_its_row_and_is_read_back_as_pending() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;
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

//! The going-down line the service says to the owner as it stops (docs/voiced-notices.md, 「Deploy and restart」):
//! the deployment's, where a fresh record says a deployment asked for the stop, the unannounced one otherwise, and
//! nothing where the owner turned restart notices off. The line is the web unit's to word; the tests read the
//! service's own mark for it.

use crate::common::Scratch;
use crate::common::units::{AGENT, OWNER, deployment, registration, standing, unrun};
use bmo_receiver::chat::restart::{notice_enabled, queue_back_notice, stop_line};
use bmo_receiver::data_root::DataRoot;
use receiver_policy::restart::RESTART;
use serde_json::json;
use tempfile::TempDir;

const NOW: i64 = 1_790_000_000_000;

async fn record(scratch: &Scratch, reason: serde_json::Value) {
  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value") VALUES ('restart_reason', $1)
         ON CONFLICT ("key") DO UPDATE SET "value" = excluded."value""#,
      vec![reason.to_string().into()],
    )
    .await;
}

#[tokio::test]
async fn a_deployments_stop_line_names_its_items_where_a_fresh_record_stands() {
  let scratch = Scratch::new().await;
  let data = TempDir::new().unwrap();
  let receiver = standing(scratch.db(), DataRoot::new(data.path()), vec![]).receiver;

  record(&scratch, json!({ "purpose": "why", "commit": "abc", "items": [1358, 1360], "at": NOW })).await;

  assert_eq!(
    stop_line(&receiver, NOW + 5_000).await,
    Some("going-down-for-update in en".to_owned()),
    "the deployment's line, naming its items"
  );

  record(&scratch, json!({ "purpose": "why", "commit": "abc", "items": [], "at": NOW })).await;

  assert_eq!(stop_line(&receiver, NOW + 5_000).await, Some("going-down-for-update in en".to_owned()));
  // The record stands for the start to spend.
  assert_eq!(
    scratch
      .count(r#"SELECT COUNT(*) FROM "settings" WHERE "key" = 'restart_reason'"#, vec![])
      .await,
    1
  );
  assert_eq!(scratch.count(r#"SELECT COUNT(*) FROM "instructions""#, vec![]).await, 0);
}

#[tokio::test]
async fn the_unannounced_stop_line_stands_without_a_fresh_record_and_none_where_notices_are_off() {
  let scratch = Scratch::new().await;
  let data = TempDir::new().unwrap();
  let receiver = standing(scratch.db(), DataRoot::new(data.path()), vec![]).receiver;
  let unannounced = Some("going-down in en".to_owned());

  assert_eq!(stop_line(&receiver, NOW).await, unannounced);

  record(&scratch, json!({ "purpose": "why", "commit": "abc", "items": [7], "at": NOW })).await;

  assert_eq!(
    stop_line(&receiver, NOW + RESTART.fresh_minutes * 60_000 + 1).await,
    unannounced,
    "a record older than the policy's fresh stretch speaks for no stop"
  );

  scratch
    .run(
      r#"INSERT INTO "chat_notices" ("chat_id", "kind", "enabled") VALUES ($1, 'restart', 0)"#,
      vec![OWNER.into()],
    )
    .await;

  assert_eq!(stop_line(&receiver, NOW).await, None);
  assert!(!notice_enabled(scratch.db(), OWNER, "restart").await);
}

#[tokio::test]
async fn the_restart_back_notice_goes_to_the_owners_private_room_where_a_notices_room_is_named() {
  const NOTICES: i64 = -9;

  let scratch = Scratch::new().await;
  let data = TempDir::new().unwrap();
  let mut given = deployment();

  given.notices_room = Some(NOTICES);

  let standing = unrun(scratch.db(), DataRoot::new(data.path()), vec![], given);

  standing.agency.registering(vec![registration(AGENT, "Agent A", vec![], None, true)]);
  queue_back_notice(&standing.receiver).await;

  assert_eq!(
    scratch
      .texts(r#"SELECT "kind" || ' ' || "chat_id" FROM "instructions" WHERE "kind" = 'restart-back'"#)
      .await,
    [format!("restart-back {OWNER}")],
    "the back notice stands in the owner's private room"
  );
  assert_eq!(
    scratch
      .count(r#"SELECT COUNT(*) FROM "instructions" WHERE "chat_id" = -9"#, vec![])
      .await,
    0,
    "the notices room hears nothing of it"
  );
}

fn own(at: i64) -> bmo_receiver::chat::relaunch::Deployed {
  bmo_receiver::chat::relaunch::Deployed {
    unit: "unit-a".to_owned(),
    version: "1.2.0".to_owned(),
    message: Some("the deploy's message".to_owned()),
    commit: Some("0123456789ab".to_owned()),
    at,
  }
}

// The back notice a start leaves: the recorded reason where one stands, the receiver's own fresh deployment record
// where none does, and the unannounced restart where neither speaks.
#[tokio::test]
async fn a_start_with_no_recorded_reason_tells_its_own_fresh_deployment_and_never_an_unannounced_restart() {
  use bmo_receiver::chat::restart::{back_notice, take_reason};

  let scratch = Scratch::new().await;
  let now = bmo_receiver::now_ms();
  let fresh = own(now - 60_000);
  let reason = take_reason(scratch.db(), Some(&fresh))
    .await
    .expect("the own record speaks for the start");

  assert_eq!(reason["purpose"], "the deploy's message");
  assert_eq!(reason["commit"], "0123456789ab");

  let told = back_notice(Some(&reason)).fact;

  assert!(told.contains("the deploy's message") && told.contains("0123456789ab"), "{told}");
  assert!(!told.contains(&RESTART.unannounced.fact), "{told}");

  let stale = own(now - (RESTART.fresh_minutes + 1) * 60_000);

  assert_eq!(
    take_reason(scratch.db(), Some(&stale)).await,
    None,
    "a stale record speaks for no start"
  );
  assert_eq!(take_reason(scratch.db(), None).await, None);

  record(&scratch, json!({ "purpose": "the recorded purpose", "commit": "fedcba", "at": now })).await;

  let recorded = take_reason(scratch.db(), Some(&fresh)).await.unwrap();

  assert_eq!(
    recorded["purpose"], "the recorded purpose",
    "a recorded reason comes first, and is spent"
  );
  assert_eq!(take_reason(scratch.db(), None).await, None);
}

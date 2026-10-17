//! The going-down line the service says to the owner as it stops (docs/voiced-notices.md, 「Deploy and restart」):
//! the deployment's, where a fresh record says a deployment asked for the stop, the unannounced one otherwise, and
//! nothing where the owner turned restart notices off. The line is the web unit's to word; the tests read the
//! service's own mark for it.

use crate::common::Scratch;
use crate::common::units::{OWNER, standing};
use bmo_receiver::chat::restart::{notice_enabled, stop_line};
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

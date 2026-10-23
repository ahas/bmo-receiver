//! The relaunch instruction (`docs/wake.md`, 「Quiet instructions」): a round that finds its session reopened is
//! told, in one quiet row, of the deployments since the session last started, read from stand-in deployment
//! records.

use std::path::Path;
use std::time::Duration;

use serde_json::json;
use tempfile::tempdir;

use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::conversation::Hooks;
use bmo_receiver::chat::relaunch::Relaunches;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::now_ms;

use crate::common::Scratch;
use crate::common::units::{AGENT, deployment, unrun};

const ROOM: i64 = 7;

// A unit's deploy root with its record, and the release's version where one is given.
fn deployed(roots: &Path, folder: &str, record: serde_json::Value, version: Option<&str>) {
  let root = roots.join(folder);

  std::fs::create_dir_all(root.join("deploy")).unwrap();
  std::fs::write(root.join("deployed.json"), record.to_string()).unwrap();

  if let Some(version) = version {
    std::fs::write(root.join("deploy").join("VERSION"), format!("{version}\n")).unwrap();
  }
}

// An RFC 3339 time, as the testbed writes a record's moment.
fn written(at: i64) -> String {
  chrono::DateTime::from_timestamp_millis(at).unwrap().to_rfc3339()
}

// The relaunch rows the room holds: each its text and how it rides.
async fn rows(scratch: &Scratch) -> Vec<String> {
  scratch
    .texts(&format!(
      r#"SELECT "text" || ' | ' || "rides"::text || ' ' || "kind" FROM "instructions" WHERE "chat_id" = {ROOM} AND "sort" = 'relaunch' ORDER BY "id""#
    ))
    .await
}

#[tokio::test]
async fn a_reopened_session_is_told_of_the_deployments_since_it_last_started() {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let roots = tempdir().unwrap();
  let standing = unrun(scratch.db(), DataRoot::new(data.path()), vec![], deployment());
  let receiver = standing.receiver.clone();
  let now = now_ms();

  deployed(
    roots.path(),
    "bmo-unit-a",
    json!({ "commit": "a".repeat(40), "subject": "subject a", "message": "message a", "version": "1.2.0", "at": written(now - 60_000) }),
    Some("9.9.9"),
  );
  deployed(
    roots.path(),
    "bmo-unit-b",
    json!({ "commit": "0123456789abcdef", "subject": "subject b", "at": now - 30_000 }),
    Some("0.3.0"),
  );
  deployed(
    roots.path(),
    "bmo-unit-c",
    json!({ "commit": "c".repeat(40), "subject": "long gone", "at": written(now - 7_200_000) }),
    None,
  );
  deployed(roots.path(), "unit-d", json!({ "subject": "no deploy root", "at": now }), None);
  std::fs::create_dir_all(roots.path().join("bmo-unit-e")).unwrap();

  // The first round of the process finds the session reopened: the deployments within the fresh while are named,
  // oldest first, the record's own version and message before the release's and the subject.
  let around = Around::new(receiver.clone()).reading_deploys(Some(roots.path().to_path_buf()));

  around.riders(AGENT, ROOM).await;

  let follow = "Keep it as the reason this session started afresh. It needs no answer of its own: say nothing of it in the room.";

  assert_eq!(
    rows(&scratch).await,
    [format!(
      "This session was relaunched because unit-a 1.2.0 was deployed: message a; and because unit-b 0.3.0 was deployed: subject b.\n{follow} | 1 session-relaunched"
    )],
  );

  // A later round of the same session, with nothing relaunching it, writes nothing.
  around.riders(AGENT, ROOM).await;
  assert_eq!(rows(&scratch).await.len(), 1);

  // The agency deployed since the session's start relaunches it: only that deployment is named, and the row still
  // waiting is replaced.
  tokio::time::sleep(Duration::from_millis(20)).await;
  deployed(
    roots.path(),
    "bmo-agency",
    json!({ "commit": "d".repeat(40), "subject": "agency subject", "at": now_ms() - 5 }),
    None,
  );
  tokio::time::sleep(Duration::from_millis(20)).await;
  around.riders(AGENT, ROOM).await;
  assert_eq!(
    rows(&scratch).await,
    [format!(
      "This session was relaunched because agency dddddddddddd was deployed: agency subject.\n{follow} | 1 session-relaunched"
    )],
  );

  // The next process's first round, with nothing deployed since, writes nothing more.
  Relaunches::new(Some(roots.path().to_path_buf())).note(&receiver, AGENT, ROOM).await;
  assert_eq!(rows(&scratch).await.len(), 1);

  // Named no deploy roots, nothing is read or written.
  Around::new(receiver.clone()).riders(AGENT, 8).await;
  assert_eq!(
    scratch
      .texts(r#"SELECT "key" FROM "settings" WHERE "key" LIKE '%session_start%' ORDER BY "key""#)
      .await,
    [format!("{AGENT}_session_start:{ROOM}")],
  );
}

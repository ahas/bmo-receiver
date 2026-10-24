//! The relaunch instruction (`docs/wake.md`, 「Quiet instructions」): a round that finds its session reopened writes
//! one quiet row for the owner's private chat, naming the deployments since the session last started and asking the
//! agent to brief him on them, read from stand-in deployment records.

use std::path::Path;
use std::time::Duration;

use serde_json::json;
use tempfile::tempdir;

use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::conversation::Hooks;
use bmo_receiver::chat::relaunch::{Relaunches, deployments, instruction};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::now_ms;

use crate::common::Scratch;
use crate::common::units::{AGENT, OWNER, deployment, unrun};

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

// A moment as the instruction names it.
fn named(at: i64) -> String {
  chrono::DateTime::from_timestamp_millis(at)
    .unwrap()
    .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

// The instruction around the facts.
fn brief(fact: &str) -> String {
  format!(
    "{fact}\n\nWhat this is for: tell the owner, in his private chat, what went live: each deployment's unit, \
     version, message or subject, commit and the moment it went live.\n\nWhat follows: Write the brief now, in the \
     owner's language and in your own words: one short paragraph naming each deployment with the unit, version, \
     message or subject, commit and moment above, and nothing of what a release carries beyond them. Do not mention \
     tools or other internals. Do not answer <<SKIP>>: the owner is to hear it. Where this turn also carries the \
     owner's own words, answer him first and give the brief after."
  )
}

// The relaunch rows a chat holds: each its text and how it rides.
async fn rows(scratch: &Scratch, chat_id: i64) -> Vec<String> {
  scratch
    .texts(&format!(
      r#"SELECT "text" || ' | ' || "rides"::text || ' ' || "kind" FROM "instructions" WHERE "chat_id" = {chat_id} AND "sort" = 'relaunch' ORDER BY "id""#
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

  assert_eq!(
    rows(&scratch, OWNER).await,
    [format!(
      "{} | 1 session-relaunched",
      brief(&format!(
        "This session was relaunched after these deployments went live: unit-a 1.2.0 went live at {}: message a \
         (commit aaaaaaaaaaaa); unit-b 0.3.0 went live at {}: subject b (commit 0123456789ab).",
        named(now - 60_000),
        named(now - 30_000),
      ))
    )],
  );
  // The briefing is the owner's alone: the room that found the session reopened hears nothing of it.
  assert!(rows(&scratch, ROOM).await.is_empty());

  // A later round of the same session, with nothing relaunching it, writes nothing.
  around.riders(AGENT, ROOM).await;
  assert_eq!(rows(&scratch, OWNER).await.len(), 1);

  // The agency deployed since the session's start relaunches it: only that deployment is named, with its commit
  // standing in for a version the record and the release name none, and the row still waiting is replaced.
  tokio::time::sleep(Duration::from_millis(20)).await;
  let agency_at = now_ms() - 5;
  deployed(
    roots.path(),
    "bmo-agency",
    json!({ "commit": "d".repeat(40), "subject": "agency subject", "at": agency_at }),
    None,
  );
  tokio::time::sleep(Duration::from_millis(20)).await;
  around.riders(AGENT, ROOM).await;
  assert_eq!(
    rows(&scratch, OWNER).await,
    [format!(
      "{} | 1 session-relaunched",
      brief(&format!(
        "This session was relaunched after these deployments went live: agency dddddddddddd went live at {}: agency \
         subject (commit dddddddddddd).",
        named(agency_at),
      ))
    )],
  );

  // The next process's first round, with nothing deployed since, writes nothing more.
  Relaunches::new(Some(roots.path().to_path_buf())).note(&receiver, AGENT, ROOM).await;
  assert_eq!(rows(&scratch, OWNER).await.len(), 1);

  // Named no deploy roots, nothing is read or written.
  Around::new(receiver.clone()).riders(AGENT, 8).await;
  assert_eq!(
    scratch
      .texts(r#"SELECT "key" FROM "settings" WHERE "key" LIKE '%session_start%' ORDER BY "key""#)
      .await,
    [format!("{AGENT}_session_start:{ROOM}")],
  );
}

#[test]
fn a_record_missing_its_commit_or_message_is_named_by_what_it_holds() {
  let roots = tempdir().unwrap();

  deployed(
    roots.path(),
    "bmo-unit-a",
    json!({ "subject": "no commit", "version": "1.0.0", "at": 1_700_000_000_000i64 }),
    None,
  );
  deployed(
    roots.path(),
    "bmo-unit-b",
    json!({ "version": "2.0.0", "at": 1_700_000_001_000i64 }),
    None,
  );

  let text = instruction(&deployments(roots.path(), None));

  assert!(text.contains("unit-a 1.0.0 went live at 2023-11-14T22:13:20Z: no commit"));
  assert!(text.contains("unit-b 2.0.0 went live at 2023-11-14T22:13:21Z."));
}

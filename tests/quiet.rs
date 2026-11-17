//! What `quiet` counts as work under way (docs/boundary-api.md, 「The service and its commands」): a turn the
//! receiver carries, which names its round, however long it has run, and no other run, whatever its name or its
//! room says; and the code it ends with, 1 for work named, 2 where the agency or the store did not answer, and
//! 0 where none stands.

use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

use axum::Json;
use axum::Router;
use axum::http::StatusCode;
use axum::routing::get;
use bmo_receiver::quiet::{jobs_under_way, under_way};
use receiver_ports::agency::{OpenRun, RunningJob};
use serde_json::{Value, json};

use crate::integration::common::Scratch;

const NOW: i64 = 1_000_000_000_000;
const MINUTE: i64 = 60_000;

fn open(id: i64, name: &str, minutes_ago: i64) -> OpenRun {
  OpenRun {
    id,
    name: name.to_owned(),
    started_at: NOW - minutes_ago * MINUTE,
    job_id: None,
    round: None,
    room_id: None,
  }
}

#[test]
fn a_turn_the_receiver_carries_counts_by_its_round_and_no_room_or_name_counts() {
  let runs = [
    // A round's turn, and a turn outside any round under a round of its own.
    OpenRun {
      round: Some("round-7".to_owned()),
      room_id: Some(-3),
      ..open(1, "agent-a:chat:-3:100", 240)
    },
    OpenRun {
      round: Some("round-8".to_owned()),
      ..open(2, "agent-a:talk", 2)
    },
    // A job's turn names its room and no round, and the agency answers no job on it.
    OpenRun {
      room_id: Some(-3),
      ..open(3, "unit-a:job-17", 2)
    },
    // A job on a checkout whose name holds a long-work word, and a run of another unit.
    open(4, "repo-research:job-906913", 2),
    open(5, "agent-a:chat:7", 300),
  ];
  let named = under_way(&runs);

  assert_eq!(named.len(), 2, "only a turn naming its round counts: {named:?}");
  assert!(named[0].starts_with("run 1 agent-a:chat:-3:100"));
  assert!(named[1].starts_with("run 2 agent-a:talk"));
  assert!(under_way(&[]).is_empty(), "nothing open is quiet");
}

#[test]
fn every_job_starting_or_running_counts_however_long_it_has_run_named_by_its_repository_and_checkout() {
  let jobs = [
    RunningJob {
      id: 906_425,
      role_name: Some("worker".to_owned()),
      repository_name: Some("repo-a".to_owned()),
      worker_name: Some("repo-a-w2".to_owned()),
      item_id: Some(2283),
      started_at: Some(NOW - 600 * MINUTE),
    },
    RunningJob {
      id: 906_426,
      role_name: None,
      repository_name: None,
      worker_name: Some("gone-w1".to_owned()),
      item_id: None,
      started_at: None,
    },
  ];
  let named = jobs_under_way(&jobs);

  assert_eq!(named.len(), 2, "{named:?}");
  assert!(
    named[0].starts_with("job 906425 as worker in repo-a (checkout repo-a-w2) on item 2283"),
    "{named:?}"
  );
  assert!(
    named[1].starts_with("job 906426 as a role in checkout gone-w1 on item none"),
    "a checkout no repository holds is named alone: {named:?}"
  );
  assert!(jobs_under_way(&[]).is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn quiet_ends_one_for_work_named_and_zero_where_none_stands() {
  let scratch = Scratch::new().await;
  let folder = tempfile::tempdir().unwrap();

  let none = agency_answering(json!([])).await;
  let output = quiet(&configuration(folder.path(), &scratch.address(), &none)).await;

  assert_eq!(output.status.code(), Some(0), "{}", said(&output));

  let running = agency_answering(json!([{
    "id": 7,
    "name": "agent-a:chat:3",
    "startedAt": 1_000_000_000_000_i64,
    "round": "round-7",
  }]))
  .await;
  let output = quiet(&configuration(folder.path(), &scratch.address(), &running)).await;

  assert_eq!(output.status.code(), Some(1), "{}", said(&output));
  assert!(
    String::from_utf8_lossy(&output.stdout).contains("run 7 agent-a:chat:3 under way since 1000000000000"),
    "the work under way is named: {}",
    said(&output)
  );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn quiet_ends_two_where_the_store_or_the_agency_was_not_read() {
  let folder = tempfile::tempdir().unwrap();

  // No store answers at the address the configuration names.
  let none = agency_answering(json!([])).await;
  let output =
    quiet(&configuration(folder.path(), "postgres:///receiver?host=/nonexistent/bmo", &none)).await;

  assert_eq!(output.status.code(), Some(2), "{}", said(&output));
  assert!(
    String::from_utf8_lossy(&output.stdout).contains("the store did not answer what stands under way"),
    "{}",
    said(&output)
  );

  // The store answers, and the agency does not.
  let scratch = Scratch::new().await;
  let refusing = agency_refusing().await;
  let output = quiet(&configuration(folder.path(), &scratch.address(), &refusing)).await;

  assert_eq!(output.status.code(), Some(2), "{}", said(&output));
  assert!(
    String::from_utf8_lossy(&output.stdout).contains("what stands under way was not read"),
    "{}",
    said(&output)
  );
}

// An agency answering `GET /runs` with `items`, on a loopback port; the address it answers at.
async fn agency_answering(items: Value) -> String {
  let app = Router::new().route(
    "/runs",
    get(move || {
      let items = items.clone();

      async move { Json(json!({ "items": items })) }
    }),
  );

  format!("http://{}", listening(app).await)
}

// An agency that does not answer `GET /runs`, on a loopback port.
async fn agency_refusing() -> String {
  let app = Router::new().route(
    "/runs",
    get(|| async {
      (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "code": "unread", "error": "the runs were not read" })),
      )
    }),
  );

  format!("http://{}", listening(app).await)
}

async fn listening(app: Router) -> String {
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = listener.local_addr().unwrap().to_string();

  tokio::spawn(async move {
    let _ = axum::serve(listener, app).await;
  });

  address
}

// A configuration file in `folder` whose store is `database` and whose agency answers at `agency`; the other
// units are named but never reached.
fn configuration(folder: &Path, database: &str, agency: &str) -> PathBuf {
  let quoted = |text: &str| serde_json::json!(text).to_string();
  let data_root = folder.join("data");
  let mut lines = vec![
    format!("listen = {}", quoted("127.0.0.1:0")),
    format!("token = {}", quoted("quiet-token")),
    format!("database = {}", quoted(database)),
    format!("data_root = {}", quoted(&data_root.to_string_lossy())),
    "owner = 42".to_owned(),
    format!("agent = {}", quoted("agent-a")),
    format!("sender_name = {}", quoted("Agent A")),
    format!("language = {}", quoted("en")),
  ];

  for (unit, base) in [
    ("agency", agency),
    ("gate", "http://127.0.0.1:1"),
    ("job", "http://127.0.0.1:1"),
    ("web", "http://127.0.0.1:1"),
    ("warmer", "http://127.0.0.1:1"),
  ] {
    lines.extend([
      format!("[{unit}]"),
      format!("base = {}", quoted(base)),
      format!("token = {}", quoted("unused")),
    ]);
  }

  let at = folder.join("settings.toml");

  std::fs::write(&at, lines.join("\n")).unwrap();

  at
}

// The binary's `quiet` run on `config`, bounded so a wait that never ends fails the test rather than the suite.
async fn quiet(config: &Path) -> Output {
  tokio::time::timeout(
    Duration::from_secs(60),
    tokio::process::Command::new(env!("CARGO_BIN_EXE_bmo-receiver"))
      .arg("--config")
      .arg(config)
      .arg("quiet")
      .output(),
  )
  .await
  .expect("quiet ended within its bound")
  .unwrap()
}

fn said(output: &Output) -> String {
  let out = String::from_utf8_lossy(&output.stdout).into_owned();
  let err = String::from_utf8_lossy(&output.stderr).into_owned();

  out + err.as_str()
}

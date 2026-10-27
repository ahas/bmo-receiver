//! What `quiet` counts as work under way (docs/boundary-api.md, 「The service and its commands」): a long-work run
//! however long it has run, any other open run started within the span, and nothing older.

use bmo_receiver::quiet::{jobs_under_way, under_way};
use receiver_ports::agency::{OpenRun, RunningJob};

const NOW: i64 = 1_000_000_000_000;
const MINUTE: i64 = 60_000;

fn open(id: i64, name: &str, minutes_ago: i64) -> OpenRun {
  OpenRun {
    id,
    name: name.to_owned(),
    started_at: NOW - minutes_ago * MINUTE,
    job_id: None,
  }
}

fn job(id: i64, minutes_ago: i64) -> OpenRun {
  OpenRun {
    job_id: Some(id * 10),
    ..open(id, "unit-a:job-17", minutes_ago)
  }
}

#[test]
fn a_long_run_counts_however_long_it_has_run_and_another_only_within_the_span() {
  let runs = [
    open(1, "agent-a:research:7", 240),
    open(2, "agent-a:chat:-3:100", 2),
    open(3, "agent-a:chat:-3:50", 300),
    job(4, 2),
  ];
  let named = under_way(&runs, NOW);

  assert_eq!(named.len(), 2, "a job's run counts not: {named:?}");
  assert!(named[0].starts_with("run 1 agent-a:research:7"));
  assert!(named[1].starts_with("run 2 agent-a:chat:-3:100"));
  assert!(under_way(&[], NOW).is_empty(), "nothing open is quiet");
}

#[test]
fn every_job_starting_or_running_counts_however_long_it_has_run() {
  let jobs = [
    RunningJob {
      id: 906_425,
      worker_name: Some("worker-a".to_owned()),
      item_id: Some(2283),
      started_at: Some(NOW - 600 * MINUTE),
    },
    RunningJob {
      id: 906_426,
      worker_name: None,
      item_id: None,
      started_at: None,
    },
  ];
  let named = jobs_under_way(&jobs);

  assert_eq!(named.len(), 2, "{named:?}");
  assert!(named[0].starts_with("job 906425 of worker-a on item 2283"));
  assert!(jobs_under_way(&[]).is_empty());
}

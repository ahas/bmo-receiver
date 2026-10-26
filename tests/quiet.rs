//! What `quiet` counts as work under way (docs/boundary-api.md, 「The service and its commands」): a long-work run
//! however long it has run, any other open run started within the span, and nothing older.

use bmo_receiver::quiet::under_way;
use receiver_ports::agency::OpenRun;

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
  let named = under_way(&runs, NOW, false);

  assert_eq!(named.len(), 2, "a job's run counts not: {named:?}");
  assert!(named[0].starts_with("run 1 agent-a:research:7"));
  assert!(named[1].starts_with("run 2 agent-a:chat:-3:100"));
  assert!(under_way(&[], NOW, false).is_empty(), "nothing open is quiet");
}

#[test]
fn with_jobs_every_open_run_of_a_job_counts_however_long_it_has_run() {
  let runs = [job(4, 2), job(5, 600), open(6, "agent-a:chat:-3:50", 300)];
  let named = under_way(&runs, NOW, true);

  assert_eq!(named.len(), 2, "{named:?}");
  assert!(named.iter().all(|line| line.contains("job")));
}

//! What `quiet` counts as work under way (docs/boundary-api.md, 「The service and its commands」): a turn of a
//! round, or a turn answering a room that is no job's, however long it has run, and no other run, whatever its
//! name says.

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
    round: None,
    room_id: None,
  }
}

#[test]
fn a_round_s_turn_and_a_room_s_turn_count_by_what_they_are_and_no_name_counts() {
  let runs = [
    // A double's round: a role's run that names its round and its room.
    OpenRun {
      round: Some("round-7".to_owned()),
      room_id: Some(-3),
      ..open(1, "round-agent-a--3-100", 240)
    },
    // A room's chat turn that names no round.
    OpenRun {
      room_id: Some(-3),
      ..open(2, "agent-a:chat:-3:100", 2)
    },
    // A job's turn in a room, and a job on a checkout whose name holds a long-work word.
    OpenRun {
      room_id: Some(-3),
      job_id: Some(40),
      ..open(3, "unit-a:job-17", 2)
    },
    OpenRun {
      job_id: Some(41),
      ..open(4, "repo-research:job-906913", 2)
    },
    // A run of another unit that answers no room.
    open(5, "agent-a:research:7", 300),
  ];
  let named = under_way(&runs);

  assert_eq!(named.len(), 2, "only a round's or a room's own turn counts: {named:?}");
  assert!(named[0].starts_with("run 1 round-agent-a--3-100"));
  assert!(named[1].starts_with("run 2 agent-a:chat:-3:100"));
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

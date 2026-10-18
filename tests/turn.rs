//! The turn a verb runs in, as the process's environment says it (docs/agent-roles.md, 「Who a send goes out as」).

use std::collections::BTreeMap;
use tempfile::tempdir;

use bmo_receiver::verbs::turn::environment;

fn env(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
  pairs.iter().map(|(name, value)| (name.to_string(), value.to_string())).collect()
}

#[test]
fn a_process_outside_a_place_reads_its_own_environment() {
  let own = env(&[("BMO_TURN_AGENT", "agent-a"), ("HOME", "the-home")]);

  assert_eq!(environment(&own), own);
}

#[test]
fn a_process_in_a_place_takes_the_runs_variables_and_none_for_a_persons_turn() {
  let place = tempdir().unwrap();
  let path = place.path().to_string_lossy().into_owned();
  let started = env(&[("BMO_AGENT_PLACE", &path), ("BMO_TURN_AGENT", "stale"), ("HOME", "the-home")]);

  std::fs::write(place.path().join("run.json"), r#"{"person":false,"env":{"BMO_TURN_AGENT":"agent-a"}}"#)
    .unwrap();
  assert_eq!(environment(&started).get("BMO_TURN_AGENT").map(String::as_str), Some("agent-a"));

  std::fs::write(place.path().join("run.json"), r#"{"person":true,"env":{"BMO_TURN_AGENT":"agent-a"}}"#)
    .unwrap();
  assert_eq!(environment(&started).get("BMO_TURN_AGENT"), None);
  assert_eq!(environment(&started).get("HOME").map(String::as_str), Some("the-home"));
}

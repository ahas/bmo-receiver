//! The turn a verb runs in, as the process's environment says it (the agency's `docs/agent-roles.md`, 「Who a send goes out as」):
//! a process a resident program started takes the run's own variables from the run its place says is going — none
//! for a person's turn — and every other process has them in its own environment.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

// The variable naming a resident program's place, whose run's variables a verb reads.
const PLACE_VARIABLE: &str = "BMO_AGENT_PLACE";

// The variable naming the agent whose turn this process works in.
const TURN_AGENT: &str = "BMO_TURN_AGENT";

// The variable naming the room whose turn this process works in.
const TURN_CHAT: &str = "BMO_TURN_CHAT";

// The job runner's own marker, set by the runner once it holds its checkout's lock.
const JOB_RUNNER_MARKER: &str = "BMO_TASK_LOCKED";

// The variables that belong to one run rather than to the program, and the prefix naming them.
const TURN_VARIABLES: [&str; 3] = ["BMO_RUN_NAME", "BMO_TASK_LOCKED", "BMO_WORK_ORDER_STEP"];
const TURN_PREFIX: &str = "BMO_TURN_";

// The run standing in a place, as much of it as a verb reads.
#[derive(Deserialize)]
struct PlaceRun {
  #[serde(default)]
  person: bool,
  #[serde(default)]
  env: BTreeMap<String, String>,
}

/// Whether a variable belongs to one run rather than to the program.
fn is_turn_variable(name: &str) -> bool {
  TURN_VARIABLES.contains(&name) || name.starts_with(TURN_PREFIX)
}

/// The process's environment with its run's own variables in it.
pub fn environment(env: &BTreeMap<String, String>) -> BTreeMap<String, String> {
  let Some(place) = env.get(PLACE_VARIABLE).filter(|place| !place.is_empty()) else {
    return env.clone();
  };
  let run = std::fs::read_to_string(Path::new(place).join("run.json"))
    .ok()
    .and_then(|text| serde_json::from_str::<PlaceRun>(&text).ok());
  let mut own: BTreeMap<String, String> = env
    .iter()
    .filter(|(name, _)| !is_turn_variable(name))
    .map(|(name, value)| (name.clone(), value.clone()))
    .collect();

  if let Some(run) = run.filter(|run| !run.person) {
    own.extend(run.env);
  }

  own
}

fn process_environment() -> BTreeMap<String, String> {
  environment(&std::env::vars().collect())
}

/// The agent whose turn this process works in, or none for a process no agent's turn started.
pub fn agent() -> Option<String> {
  process_environment()
    .get(TURN_AGENT)
    .map(|named| named.trim().to_owned())
    .filter(|named| !named.is_empty())
}

/// The moment the turn this process works in took its words, in epoch milliseconds, or none for a process
/// no agent's turn started.
pub fn began() -> Option<i64> {
  process_environment()
    .get(room_calls::turn::BEGAN_VARIABLE)
    .and_then(|at| at.trim().parse().ok())
}

/// The room whose turn this process works in, or none for a process no room's turn started.
pub fn chat() -> Option<i64> {
  process_environment().get(TURN_CHAT).and_then(|chat| chat.trim().parse().ok())
}

/// Whether a job is the one running this: a Claude Code session the job runner holds.
pub fn inside_job() -> bool {
  let set = |value: Option<&String>| value.is_some_and(|value| !value.is_empty());

  set(std::env::var("CLAUDECODE").ok().as_ref()) && set(process_environment().get(JOB_RUNNER_MARKER))
}

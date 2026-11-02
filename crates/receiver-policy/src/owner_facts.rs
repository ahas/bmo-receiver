//! What the owner's session is told of the agency's events that no other part words (docs/policy.md): a worker
//! fallen to a lesser model, the overtime order, and a road the agency moved, returned or found closed.

use serde::Deserialize;

/// A worker fallen to a lesser model: the notice's fact, what it is for, what follows, its kind and lifetime.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerDowngraded {
  pub fact: String,
  pub job: String,
  pub intent: String,
  pub dropped: String,
  pub kept: String,
  pub kind: String,
}

/// The order a run past its clock hands the chat agent.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Overtime {
  pub head: String,
  pub ask: String,
  pub what: String,
  pub run: String,
  pub running: String,
  pub bound: String,
  pub activity: String,
  pub activity_at: String,
  pub activity_unknown: String,
  pub activity_none: String,
  pub nothing_cut: String,
  pub steps: Vec<String>,
  pub kind: String,
  pub sort: String,
}

/// A work key's road the agency moved, returned or found closed, as the owner is told it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Road {
  pub kind: String,
  pub sort: String,
  pub road: String,
  pub road_account: String,
  pub road_none: String,
  pub reason: String,
  pub reasons_joint: String,
  pub reasons_none: String,
  pub moved: String,
  pub returned: String,
  pub closed: String,
  pub command: String,
  pub no_command: String,
  pub reopens: String,
  pub intent: String,
  pub follow_up: String,
  pub frame: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerFacts {
  pub worker_downgraded: WorkerDowngraded,
  pub overtime: Overtime,
  pub road: Road,
}

crate::part!(pub OWNER_FACTS: OwnerFacts = "owner-facts.toml");

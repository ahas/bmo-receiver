//! What the owner's session is told of the agency's events that no other part words (docs/policy.md): a
//! job's run fallen to a lesser model, the overtime order, a road the agency moved, returned or found closed,
//! and a job's question settled on its recommendation.

use serde::Deserialize;

/// A job's run fallen to a lesser model: the notice's fact, where the run ran with its repository and
/// without one, what it is for, what follows, the word for a role the agency does not name, and its kind.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerDowngraded {
  pub fact: String,
  pub place: String,
  pub place_none: String,
  pub job: String,
  pub intent: String,
  pub dropped: String,
  pub kept: String,
  pub role_none: String,
  pub kind: String,
}

/// The owner notice of a question settled on its recommendation once its due time passed: its code, its fact,
/// the item as the fact names it, and what it is for.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuestionSettled {
  pub code: String,
  pub fact: String,
  pub item: String,
  pub intent: String,
}

/// The order a run past its clock hands the default answerer.
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

/// A supervisor's or a role's road the agency moved, returned or found closed, as the owner is told it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Road {
  pub kind: String,
  pub sort: String,
  pub supervisor: String,
  pub role: String,
  pub row_none: String,
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
  pub question_settled: QuestionSettled,
}

crate::part!(pub OWNER_FACTS: OwnerFacts = "owner-facts.toml");

//! The back notice a restart leaves the owner's session (docs/voiced-notices.md, 「Deploy and restart」), and the
//! deployment briefing naming what went live since the owner was last briefed (docs/wake.md, 「Quiet
//! instructions」).

use serde::Deserialize;

/// A restart no deployment announced.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unannounced {
  pub fact: String,
  pub intent: String,
  pub follow_up: String,
}

/// A deployment's restart.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Deployed {
  pub back: String,
  pub unit: String,
  pub subject: String,
  pub purpose: String,
  pub no_purpose: String,
  pub commit: String,
  pub item: String,
  pub titled_item: String,
  pub no_item: String,
  pub job: String,
  pub cut_turn: String,
  pub cut_job: String,
  pub cut_none: String,
  pub intent: String,
  pub job_follow_up: String,
  pub follow_up: String,
}

/// The deployment briefing (docs/wake.md, 「Quiet instructions」).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Briefing {
  pub kind: String,
  pub sort: String,
  pub fact: String,
  pub deployed: String,
  pub no_message: String,
  pub deployed_no_commit: String,
  pub no_message_no_commit: String,
  pub joiner: String,
  pub intent: String,
  pub follow_up: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Restart {
  pub fresh_minutes: i64,
  pub ride_minutes: i64,
  pub kind: String,
  pub unannounced: Unannounced,
  pub deployed: Deployed,
  pub briefing: Briefing,
}

crate::part!(pub RESTART: Restart = "restart.toml");

//! The back notice a restart leaves the owner's session (docs/voiced-notices.md, 「Deploy and restart」), and the
//! instruction naming why a session was relaunched (docs/wake.md, 「Quiet instructions」).

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
  pub purpose: String,
  pub no_purpose: String,
  pub commit: String,
  pub item: String,
  pub titled_item: String,
  pub no_item: String,
  pub job: String,
  pub intent: String,
  pub job_follow_up: String,
  pub follow_up: String,
}

/// The relaunch instruction (docs/wake.md, 「Quiet instructions」).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relaunched {
  pub units: Vec<String>,
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
  pub ttl_minutes: i64,
  pub unannounced: Unannounced,
  pub deployed: Deployed,
  pub relaunched: Relaunched,
}

crate::part!(pub RESTART: Restart = "restart.toml");

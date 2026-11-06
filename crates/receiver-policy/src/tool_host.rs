//! What the receiver hears of the tool host's catalogue and tells the supervisors of it (docs/boundary-api.md,
//! 「The tool host」; docs/wake.md, 「Quiet instructions」): the stream's clocks, the marker and rule of a change's
//! notice, and the words a tool placed, updated or removed is told in.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolHost {
  pub reopen_seconds: u64,
  pub silent_seconds: u64,
  pub kind: String,
  pub sort: String,
  pub summary: String,
  pub marker: String,
  pub rule: String,
  pub placed: String,
  pub updated: String,
  pub removed: String,
  pub entry: String,
  pub joint: String,
  pub no_entries: String,
}

crate::part!(pub TOOL_HOST: ToolHost = "tool-host.toml");

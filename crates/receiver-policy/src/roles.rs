//! What sending into a room asks of an agent, as far as the receiver reads it (docs/chat-mcp.md, 「How a
//! call runs」): the bundle that opens sending, and the bundles every supervisor holds. A role's own bundles
//! are the agency's.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Roles {
  /// The bundle that opens sending into a room.
  pub send: String,
  /// The bundles every supervisor's session holds.
  pub supervisor: Vec<String>,
}

impl Roles {
  /// Whether bundles of these names give sending into a room.
  pub fn sends<S: AsRef<str>>(&self, bundles: &[S]) -> bool {
    bundles.iter().any(|bundle| bundle.as_ref() == self.send)
  }

  /// Whether a supervisor sends into the rooms it answers.
  pub fn supervisor_sends(&self) -> bool {
    self.sends(&self.supervisor)
  }
}

crate::part!(pub ROLES: Roles = "roles.toml");

//! The roles an agent may hold, as far as the receiver reads them (docs/chat-mcp.md, 「How a call runs」): the
//! bundles each gives, and the bundle that opens sending into a room.

use serde::Deserialize;

/// One role: its name and the bundles it gives.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Role {
  pub name: String,
  pub bundles: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Roles {
  /// The bundle that opens sending into a room.
  pub send: String,
  pub roles: Vec<Role>,
}

impl Roles {
  pub fn role(&self, name: &str) -> Option<&Role> {
    self.roles.iter().find(|role| role.name == name)
  }

  /// The bundles a role gives; none for a role the policy does not declare.
  pub fn bundles_of(&self, name: &str) -> &[String] {
    self.role(name).map_or(&[], |role| &role.bundles)
  }

  /// Whether a role gives sending into a room.
  pub fn sends(&self, name: &str) -> bool {
    self.bundles_of(name).contains(&self.send)
  }
}

crate::part!(pub ROLES: Roles = "roles.toml");

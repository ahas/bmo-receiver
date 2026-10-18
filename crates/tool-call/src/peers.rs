//! Where the units a tool answers through are found (docs/chat-mcp.md, 「Where each unit is found」): the
//! variables the tool's own environment names, else those the unit's `tool-env` verb wrote into its env file
//! under the record's data root, since the tool host hands a placed executable a clean environment.

use std::path::Path;

use receiver_ports::http::Peer;

/// The variable naming the brain's base address.
pub const BRAIN_ADDRESS_VARIABLE: &str = "BRAIN_ADDRESS";

/// The variable naming the bearer token the brain takes.
pub const BRAIN_TOKEN_VARIABLE: &str = "BRAIN_TOKEN";

/// The env file under the data root the brain's `tool-env` verb writes those two into.
pub const BRAIN_ENV_FILE: &str = "env/brain.env";

/// The variable naming the port agency's server answers on.
pub const AGENCY_PORT_VARIABLE: &str = "AGENCY_CONTROL_PORT";

/// The variable naming the bearer token agency's server takes.
pub const AGENCY_TOKEN_VARIABLE: &str = "AGENCY_CONTROL_TOKEN";

/// The env file under the data root agency's server takes its own environment from.
pub const AGENCY_ENV_FILE: &str = "env/agency.env";

/// The port agency's server answers on where none is named.
pub const AGENCY_PORT: u16 = 3220;

// One variable: this process's environment first, then the env file's.
fn variable(name: &str, file: &[(String, String)]) -> Option<String> {
  std::env::var(name)
    .ok()
    .or_else(|| file.iter().find(|(key, _)| key == name).map(|(_, value)| value.clone()))
    .map(|value| value.trim().to_owned())
    .filter(|value| !value.is_empty())
}

// Every variable an env file under the data root names, none where there is no such file.
fn named(file: &Path) -> Vec<(String, String)> {
  dotenvy::from_path_iter(file)
    .map(|pairs| pairs.filter_map(Result::ok).collect())
    .unwrap_or_default()
}

/// The unit at an address, under the token it is called with; a unit naming no token is called with none.
fn at(base: &str, token: &str) -> Peer {
  Peer {
    base: base.trim_end_matches('/').to_owned(),
    token: token.to_owned(),
  }
}

/// The brain the record's call reaches, or why it names none: a tool that answers the brain needs both its
/// address and its token, from its own environment or the data root's env file.
pub fn brain(data_root: &Path) -> Result<Peer, String> {
  let path = data_root.join(BRAIN_ENV_FILE);
  let file = named(&path);

  let base = variable(BRAIN_ADDRESS_VARIABLE, &file).ok_or_else(|| {
    format!("no {BRAIN_ADDRESS_VARIABLE} in this tool's environment or in {}", path.display())
  });

  Ok(at(&base?, &variable(BRAIN_TOKEN_VARIABLE, &file).unwrap_or_default()))
}

/// Agency's server the record's call reaches, at the port it names or the one it answers on by default.
pub fn agency(data_root: &Path) -> Peer {
  let file = named(&data_root.join(AGENCY_ENV_FILE));
  let port = variable(AGENCY_PORT_VARIABLE, &file)
    .and_then(|port| port.parse().ok())
    .unwrap_or(AGENCY_PORT);

  at(
    &format!("http://127.0.0.1:{port}"),
    &variable(AGENCY_TOKEN_VARIABLE, &file).unwrap_or_default(),
  )
}

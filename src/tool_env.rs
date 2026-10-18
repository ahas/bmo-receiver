//! Where the receiver's tools find it (docs/boundary-api.md, 「Its tools」): the data root's env file, the
//! service's base address and token written into it from the configuration, readable by the user alone and
//! rewritten only where it would change.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use crate::config::Config;

/// The env file under the data root the tools read the receiver's address and token from.
pub const ENV_FILE: &str = "env/receiver.env";

/// The variable naming the receiver's base address.
pub const ADDRESS_VARIABLE: &str = "RECEIVER_ADDRESS";

/// The variable naming the bearer token the receiver takes.
pub const TOKEN_VARIABLE: &str = "RECEIVER_TOKEN";

/// What writing the env file came to, and where it stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Written {
  Written(PathBuf),
  Unchanged(PathBuf),
}

/// The base address a tool on this machine reaches the service at: its listen address over `http://`, an
/// unspecified host answered as the loopback one; a listen address naming a host by name stands as it is.
pub fn base_address(listen: &str) -> String {
  let listen = listen.trim();
  let Ok(mut address) = listen.parse::<SocketAddr>() else {
    return format!("http://{listen}");
  };

  if address.ip().is_unspecified() {
    address.set_ip(match address.ip() {
      IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::LOCALHOST),
      IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::LOCALHOST),
    });
  }

  format!("http://{address}")
}

// Whether an env file holds the value as it is, with no quoting for its reader to undo.
fn bare(value: &str) -> bool {
  !value.is_empty()
    && !value.chars().any(|character| {
      character.is_whitespace() || matches!(character, '"' | '\'' | '`' | '#' | '$' | '\\')
    })
}

/// The env file's text for this configuration, or why a value cannot stand in it.
pub fn env_text(config: &Config) -> Result<String, String> {
  let address = base_address(&config.listen);

  for (name, value) in [(ADDRESS_VARIABLE, address.as_str()), (TOKEN_VARIABLE, config.token.as_str())] {
    if !bare(value) {
      return Err(format!(
        "{name} cannot stand bare in {ENV_FILE}: it is empty or holds whitespace, a quote, '#', '$' or '\\'"
      ));
    }
  }

  Ok(format!("{ADDRESS_VARIABLE}={address}\n{TOKEN_VARIABLE}={}\n", config.token))
}

/// The env file written under the configured data root, where it would change; nothing is written where a value
/// cannot stand in it.
pub fn write_env(config: &Config) -> Result<Written, String> {
  let path = config.data_root.join(ENV_FILE);
  let text = env_text(config)?;

  if std::fs::read(&path).is_ok_and(|standing| standing == text.as_bytes()) {
    return Ok(Written::Unchanged(path));
  }

  if let Some(folder) = path.parent() {
    std::fs::create_dir_all(folder).map_err(|why| format!("{}: {why}", folder.display()))?;
  }

  std::fs::write(&path, text.as_bytes()).map_err(|why| format!("{}: {why}", path.display()))?;
  std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
    .map_err(|why| format!("{}: {why}", path.display()))?;

  Ok(Written::Written(path))
}

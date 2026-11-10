//! Where the receiver's tools find it (docs/boundary-api.md, 「Its tools」): the binary's `tool-env` writes the
//! data root's env file from the configuration as a deployment runs it, readable by the user alone, rewritten
//! only where it would change, refused with nothing written for a value an env file cannot hold bare.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use bmo_receiver::tool_env::{ADDRESS_VARIABLE, ENV_FILE, TOKEN_VARIABLE, base_address};

// A configuration file in `folder` for a service listening on `listen` with `token`, its data root `data`; the
// database and the other units are named only, since writing the env file reaches none of them.
fn configuration(folder: &Path, data: &Path, listen: &str, token: &str) -> PathBuf {
  // A string as a TOML basic string, whose escapes are JSON's.
  let quoted = |text: &str| serde_json::json!(text).to_string();
  let path = |at: &Path| quoted(&at.to_string_lossy());
  let mut config = vec![
    format!("listen = {}", quoted(listen)),
    format!("token = {}", quoted(token)),
    format!("database = {}", quoted("postgres://nowhere/receiver")),
    format!("data_root = {}", path(data)),
    "owner = 42".to_owned(),
    format!("agent = {}", quoted("agent-a")),
    format!("sender_name = {}", quoted("Agent A")),
    format!("language = {}", quoted("en")),
  ];

  for peer in ["agency", "gate", "job", "web", "warmer"] {
    config.extend([
      format!("[{peer}]"),
      format!("base = {}", quoted("http://[::1]:9")),
      format!("token = {}", quoted("unused")),
    ]);
  }

  let at = folder.join("settings.toml");

  std::fs::write(&at, config.join("\n")).unwrap();

  at
}

// The binary run as the deployment's step runs it, from the folder its configuration stands in.
fn tool_env(config: &Path) -> Output {
  Command::new(env!("CARGO_BIN_EXE_bmo-receiver"))
    .current_dir(config.parent().unwrap())
    .args(["--config", "settings.toml", "tool-env"])
    .env_remove(ADDRESS_VARIABLE)
    .env_remove(TOKEN_VARIABLE)
    .output()
    .unwrap()
}

fn said(output: &Output) -> (String, String) {
  (
    String::from_utf8_lossy(&output.stdout).into_owned(),
    String::from_utf8_lossy(&output.stderr).into_owned(),
  )
}

#[test]
fn the_base_address_is_the_listen_address_over_http_an_unspecified_host_the_loopback_one() {
  assert_eq!(base_address("127.0.0.1:3212"), "http://127.0.0.1:3212");
  assert_eq!(base_address(" 0.0.0.0:3212 "), "http://127.0.0.1:3212");
  assert_eq!(base_address("[::]:3212"), "http://[::1]:3212");
  assert_eq!(base_address("[::1]:3212"), "http://[::1]:3212");
  assert_eq!(base_address("localhost:3212"), "http://localhost:3212");
}

#[test]
fn the_env_file_is_the_one_the_tools_read() {
  assert_eq!(ENV_FILE, "env/receiver.env");
  assert_eq!(ADDRESS_VARIABLE, "RECEIVER_ADDRESS");
  assert_eq!(TOKEN_VARIABLE, "RECEIVER_TOKEN");
}

#[test]
fn the_env_file_is_written_from_the_configuration_privately_and_only_where_it_would_change() {
  let folder = tempfile::tempdir().unwrap();
  let data = folder.path().join("data");
  let config = configuration(folder.path(), &data, "0.0.0.0:3212", "receiver-secret");
  let written = data.join(ENV_FILE);

  let first = tool_env(&config);

  assert!(first.status.success(), "{:?}", said(&first));
  assert_eq!(said(&first).0, format!("bmo-receiver: wrote {}\n", written.display()));
  assert_eq!(
    std::fs::read_to_string(&written).unwrap(),
    "RECEIVER_ADDRESS=http://127.0.0.1:3212\nRECEIVER_TOKEN=receiver-secret\n",
  );
  assert_eq!(std::fs::metadata(&written).unwrap().permissions().mode() & 0o777, 0o600);

  let modified = std::fs::metadata(&written).unwrap().modified().unwrap();
  let again = tool_env(&config);

  assert!(again.status.success(), "{:?}", said(&again));
  assert_eq!(
    said(&again).0,
    format!("bmo-receiver: {} already stands as written\n", written.display())
  );
  assert_eq!(std::fs::metadata(&written).unwrap().modified().unwrap(), modified);

  let rotated = configuration(folder.path(), &data, "127.0.0.1:4000", "rotated");
  let asked = tool_env(&rotated);

  assert!(asked.status.success(), "{:?}", said(&asked));
  assert_eq!(
    std::fs::read_to_string(&written).unwrap(),
    "RECEIVER_ADDRESS=http://127.0.0.1:4000\nRECEIVER_TOKEN=rotated\n",
  );
}

#[test]
fn a_value_an_env_file_cannot_hold_bare_writes_nothing() {
  for token in ["two words", "quoted'", "a#b", "$HOME", "back\\slash", ""] {
    let folder = tempfile::tempdir().unwrap();
    let data = folder.path().join("data");
    let config = configuration(folder.path(), &data, "127.0.0.1:3212", token);
    let refused = tool_env(&config);

    assert_eq!(refused.status.code(), Some(1), "{token:?}: {:?}", said(&refused));
    assert!(
      said(&refused).1.contains("RECEIVER_TOKEN cannot stand bare in env/receiver.env"),
      "{token:?}"
    );
    assert!(!data.join(ENV_FILE).exists(), "{token:?}");
  }
}

#[test]
fn the_example_configuration_loads_as_it_stands() {
  let at = PathBuf::from("settings.example.toml");

  bmo_receiver::config::Config::read(&at).unwrap_or_else(|why| panic!("{why}"));
}

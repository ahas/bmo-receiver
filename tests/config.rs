//! The configuration as the binary reads it (docs/boundary-api.md, 「Configuration」): where the brain answers,
//! named in the file or else read off the brain's own env file under the data root.

use std::path::Path;

use bmo_receiver::config::Config;
use receiver_ports::Peer;

// A configuration whose data root is `data`, with `extra` after its units.
fn configured(data: &Path, extra: &str) -> Config {
  let quoted = |text: &str| serde_json::json!(text).to_string();
  let mut lines = vec![
    format!("listen = {}", quoted("[::1]:9")),
    format!("token = {}", quoted("t")),
    format!("database = {}", quoted("postgres://nowhere/receiver")),
    format!("data_root = {}", quoted(&data.to_string_lossy())),
    "owner = 42".to_owned(),
    format!("agent = {}", quoted("agent-a")),
    format!("sender_name = {}", quoted("Agent A")),
    format!("language = {}", quoted("en")),
  ];

  for peer in ["agency", "gate", "job", "web", "warmer"] {
    lines.extend([format!("[{peer}]"), format!("base = {}", quoted("http://[::1]:9"))]);
  }

  lines.push(extra.to_owned());

  let folder = tempfile::tempdir().unwrap();
  let at = folder.path().join("settings.toml");

  std::fs::write(&at, lines.join("\n")).unwrap();

  Config::read(&at).unwrap()
}

#[test]
fn the_brain_is_the_one_the_file_names_else_the_one_its_env_file_names_else_none() {
  let data = tempfile::tempdir().unwrap();

  assert_eq!(configured(data.path(), "").brain(), None, "neither names it");

  std::fs::create_dir_all(data.path().join("env")).unwrap();
  std::fs::write(
    data.path().join("env/brain.env"),
    "BRAIN_ADDRESS=http://[::1]:7\nBRAIN_TOKEN=brain-token\n",
  )
  .unwrap();

  assert_eq!(
    configured(data.path(), "").brain(),
    Some(Peer {
      base: "http://[::1]:7".to_owned(),
      token: "brain-token".to_owned(),
    }),
    "the brain's own env file stands in"
  );
  assert_eq!(
    configured(data.path(), "[brain]\nbase = \"http://127.0.0.1:8\"\ntoken = \"named\"").brain(),
    Some(Peer {
      base: "http://127.0.0.1:8".to_owned(),
      token: "named".to_owned(),
    }),
    "the configuration's own stands over it"
  );

  std::fs::write(data.path().join("env/brain.env"), "BRAIN_TOKEN=brain-token\n").unwrap();

  assert_eq!(
    configured(data.path(), "").brain(),
    None,
    "an env file naming no address names no brain"
  );
}

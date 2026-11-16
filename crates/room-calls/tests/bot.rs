//! The running receiver as a placed tool and a shell's command reach it: what its env file under the data root
//! says of its address, its token and the deployment it answers for (docs/chat-mcp.md, 「Where each unit is
//! found」).

use room_calls::bot::{
  ADDRESS_VARIABLE, ENV_FILE, OWNER_VARIABLE, SOURCE_VARIABLE, Service, TOKEN_VARIABLE,
};
use room_calls::sends::{HANDED_WITHIN, SENDS_PATH};
use serde_json::Value;
use std::path::Path;
use tempfile::tempdir;

#[test]
fn the_receiver_is_read_from_its_file_under_the_data_root() {
  let root = tempdir().unwrap();

  std::fs::create_dir_all(root.path().join("env")).unwrap();
  std::fs::write(
    root.path().join(ENV_FILE),
    format!(
      "{OWNER_VARIABLE}=700\n{ADDRESS_VARIABLE}=https://receiver.invalid\n{TOKEN_VARIABLE}=\"secret\"\n{SOURCE_VARIABLE}=/srv/bmo\n",
    ),
  ).unwrap();

  let receiver = Service::of(root.path());

  // A variable the test's own process holds stands first, so only what it lacks is held to the file.
  if std::env::var_os(OWNER_VARIABLE).is_none() {
    assert_eq!(receiver.owner, Some(700));
  }
  if std::env::var_os(ADDRESS_VARIABLE).is_none() {
    assert_eq!(receiver.address, "https://receiver.invalid");
  }
  if std::env::var_os(TOKEN_VARIABLE).is_none() {
    assert_eq!(receiver.token.as_deref(), Some("secret"));
  }
  if std::env::var_os(SOURCE_VARIABLE).is_none() {
    assert_eq!(receiver.source.as_deref(), Some(Path::new("/srv/bmo")));
  }
}

#[test]
fn a_data_root_with_no_file_names_only_what_this_process_holds() {
  let receiver = Service::of(tempdir().unwrap().path());

  if std::env::var_os(ADDRESS_VARIABLE).is_none() {
    assert!(receiver.address.is_empty());
  }
  if std::env::var_os(TOKEN_VARIABLE).is_none() {
    assert_eq!(receiver.token, None);
  }
}

#[tokio::test]
async fn a_receiver_with_no_token_is_not_asked() {
  if std::env::var_os(TOKEN_VARIABLE).is_some() {
    return;
  }

  let asked = Service::default().post(SENDS_PATH, &Value::Null, HANDED_WITHIN).await;

  assert!(asked.unwrap_err().contains(TOKEN_VARIABLE));
}

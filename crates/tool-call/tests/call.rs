//! The receiver's side of the tool host's call protocol: a request read or refused, the turn's variables read from
//! the record, an answer written as the protocol writes one, and the units found under the data root.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

use serde_json::json;

use tool_call::{Answer, Asked, Session, agency, differences, read_handed, read_handed_with};

// The variable naming, in the process started for one test, the file its run marks once it passed.
const ALONE: &str = "BMO_TOOL_CALL_ALONE";

// A test that reads the unit variables the calling shell holds run alone in a process of its own, with those
// variables removed, so what it reads is its data root's alone.
struct AloneRun(PathBuf);

impl Drop for AloneRun {
  fn drop(&mut self) {
    if !std::thread::panicking() {
      std::fs::write(&self.0, "").unwrap();
    }
  }
}

fn alone(test: &str) -> Option<AloneRun> {
  if let Some(mark) = std::env::var_os(ALONE) {
    return Some(AloneRun(mark.into()));
  }

  let mark = std::env::temp_dir().join(format!("bmo-tool-call-alone-{}-{test}", std::process::id()));

  let _ = std::fs::remove_file(&mark);

  let ran = Command::new(std::env::current_exe().unwrap())
    .args([test, "--exact", "--nocapture"])
    .env(ALONE, &mark)
    .env_remove("AGENCY_CONTROL_PORT")
    .env_remove("AGENCY_CONTROL_TOKEN")
    .output()
    .unwrap();
  let said = format!("{}{}", String::from_utf8_lossy(&ran.stdout), String::from_utf8_lossy(&ran.stderr));

  assert!(ran.status.success(), "{test} failed in a process of its own:\n{said}");
  assert!(
    std::fs::remove_file(&mark).is_ok(),
    "{test} did not run in a process of its own:\n{said}"
  );

  None
}

#[test]
fn a_call_is_read_with_its_record_and_another_protocol_or_shape_is_refused() {
  let handed = read_handed(
    json!({
      "protocol": 1,
      "kind": "call",
      "entry": "bmo/say",
      "arguments": { "text": "hello" },
      "session": {
        "line": "chat",
        "capabilities": ["room.tools"],
        "dataRoot": "/data",
        "place": "/data/places/a",
        "runVariables": { "BMO_TURN_CHAT": "-5" },
      },
      "callback": "/run/socket",
    })
    .to_string()
    .as_bytes(),
  )
  .unwrap();

  assert_eq!(
    handed.asked,
    Asked::Call {
      entry: "bmo/say".into(),
      arguments: json!({ "text": "hello" })
    }
  );
  assert_eq!(handed.session.data_root.to_str(), Some("/data"));
  assert_eq!(handed.session.place.as_ref().and_then(|place| place.to_str()), Some("/data/places/a"));
  assert_eq!(
    read_handed(
      json!({ "protocol": 1, "kind": "helper", "socket": "/s", "session": {} })
        .to_string()
        .as_bytes()
    )
    .unwrap()
    .asked,
    Asked::Other,
  );
  assert!(
    read_handed(
      json!({ "protocol": 2, "kind": "call", "entry": "x", "session": {} })
        .to_string()
        .as_bytes()
    )
    .unwrap_err()
    .contains("protocol 2")
  );
  assert!(read_handed("not json".as_bytes()).unwrap_err().contains("does not read"));
}

#[test]
fn the_places_run_variables_stand_over_the_records_own_and_a_persons_turn_names_none_from_the_place() {
  let place = tempfile::tempdir().unwrap();
  let record = |run: serde_json::Value| {
    std::fs::write(place.path().join("run.json"), run.to_string()).unwrap();

    Session {
      place: Some(place.path().to_path_buf()),
      run_variables: BTreeMap::from([
        ("BMO_TURN_AGENT".to_owned(), "agent-b".to_owned()),
        ("BMO_TURN_CHAT".to_owned(), " 1 ".to_owned()),
      ]),
      ..Session::default()
    }
  };

  assert_eq!(
    record(json!({ "env": { "BMO_TURN_AGENT": "agent-a", "BMO_TURN_CHAT": "-5" } })).turn_variables(),
    BTreeMap::from([
      ("BMO_TURN_AGENT".to_owned(), "agent-a".to_owned()),
      ("BMO_TURN_CHAT".to_owned(), "-5".to_owned()),
    ]),
  );
  assert_eq!(
    record(json!({ "person": true, "env": { "BMO_TURN_AGENT": "agent-a" } })).turn_variables(),
    BTreeMap::from([
      ("BMO_TURN_AGENT".to_owned(), "agent-b".to_owned()),
      ("BMO_TURN_CHAT".to_owned(), " 1 ".to_owned()),
    ]),
  );
  assert_eq!(Session::default().turn_variables(), BTreeMap::new());
}

#[test]
fn an_answer_is_written_as_the_protocol_writes_one() {
  assert_eq!(
    serde_json::to_value(Answer::refusal("no room")).unwrap(),
    json!({ "content": [{ "type": "text", "text": "no room" }], "isError": true })
  );
  assert_eq!(Answer::failure("down").plain(), "ERROR: down");

  let reading = Answer::structured(json!({ "lines": ["one"] }));

  assert_eq!(
    serde_json::to_value(&reading).unwrap(),
    json!({
      "content": [{ "type": "text", "text": "{\"lines\":[\"one\"]}" }],
      "isError": false,
      "structuredContent": { "lines": ["one"] },
    })
  );
}

#[test]
fn the_agency_is_found_in_the_data_roots_env_file_or_at_its_own_port() {
  let Some(_run) = alone("call::the_agency_is_found_in_the_data_roots_env_file_or_at_its_own_port") else {
    return;
  };
  let data = tempfile::tempdir().unwrap();

  assert_eq!(
    (agency(data.path()).base.as_str(), agency(data.path()).token.as_str()),
    ("http://127.0.0.1:3220", "")
  );

  std::fs::create_dir_all(data.path().join("env")).unwrap();
  std::fs::write(
    data.path().join("env/agency.env"),
    "AGENCY_CONTROL_PORT=3221\nAGENCY_CONTROL_TOKEN=quiet\n",
  )
  .unwrap();

  assert_eq!(
    (agency(data.path()).base.as_str(), agency(data.path()).token.as_str()),
    ("http://127.0.0.1:3221", "quiet")
  );
}

#[test]
fn a_manifest_that_names_no_entry_differs_from_the_executable_nowhere() {
  let dir = tempfile::tempdir().unwrap();

  std::fs::write(dir.path().join("tool.toml"), "tool = { name = \"none\", protocol = 1 }\n").unwrap();
  assert_eq!(differences(dir.path(), &[]), Vec::<String>::new());
}

// The tool host's record names no data root and no place of its own: the call's BMO_DATA_DIR gives the one, the
// record's labels the other (docs/chat-mcp.md, 「How a call runs」).
#[test]
fn a_tool_host_record_takes_its_data_root_from_the_call_and_its_place_from_its_labels() {
  // The call's data root is handed in, not read from this process's environment, which a runner may set.
  let handed = read_handed_with(
    serde_json::json!({
      "protocol": 1,
      "kind": "call",
      "entry": "bmo/say",
      "arguments": { "text": "a line" },
      "session": {
        "holderName": "agent-a",
        "namespaces": ["bmo"],
        "capabilities": ["room.tools"],
        "grants": [],
        "labels": { "line": "chat", "place": "/srv/data/agent-places/agent-a" },
        "expiresAt": null,
        "createdAt": 1
      }
    })
    .to_string()
    .as_bytes(),
    Some("/srv/data".to_owned()),
  )
  .unwrap();
  let session = handed.session;

  assert_eq!(session.data_root, std::path::PathBuf::from("/srv/data"));
  assert_eq!(session.place, Some(std::path::PathBuf::from("/srv/data/agent-places/agent-a")));

  // A record that names its own keeps them.
  let own = tool_call::Session {
    data_root: "/own".into(),
    place: Some("/own/place".into()),
    ..tool_call::Session::default()
  }
  .completed(Some("/srv/data".to_owned()));

  assert_eq!((own.data_root, own.place), ("/own".into(), Some("/own/place".into())));
}

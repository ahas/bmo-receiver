//! The receiver's side of the tool host's call protocol: a request read or refused, the turn's variables read from
//! the record, an answer written as the protocol writes one, and the units found under the data root.

use std::collections::BTreeMap;

use serde_json::json;

use tool_call::{Answer, Asked, Session, agency, differences, read_handed};

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

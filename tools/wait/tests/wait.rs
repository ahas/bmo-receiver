//! The wait tool's entries (docs/reminders.md, 「The wait tool」): the manifest held to the executable, a wait
//! answered with its note once its moment has come, and a watch answered once its check holds or it lapses.

use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

use serde_json::{Value, json};
use tempfile::tempdir;

use tool_call::{Answer, differences, read_handed};
use wait_tool::{ENTRIES, WaitInput, answer, call, wait_end};

fn folder() -> &'static Path {
  Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn handed(entry: &str, arguments: Value) -> tool_call::Handed {
  read_handed(
    json!({ "protocol": 1, "kind": "call", "entry": entry, "arguments": arguments, "session": {} })
      .to_string()
      .as_bytes(),
  )
  .unwrap()
}

fn answered(said: Value) -> Answer {
  serde_json::from_value(said).unwrap()
}

#[test]
fn the_manifest_and_the_executable_declare_the_same_entries() {
  assert_eq!(differences(folder(), ENTRIES), Vec::<String>::new());
}

#[test]
fn every_manual_the_manifest_names_stands_beside_it() {
  let manifest: toml::Table = std::fs::read_to_string(folder().join("tool.toml")).unwrap().parse().unwrap();

  for entry in manifest["entry"].as_array().unwrap() {
    let manual = entry["manual"].as_str().unwrap();

    assert!(folder().join(manual).is_file(), "{manual}");
    assert_eq!(entry["unbounded"].as_bool(), Some(true), "{manual}");
  }
}

#[test]
fn a_wait_ends_seconds_from_now_or_at_the_moment_it_names_and_is_refused_both_or_neither() {
  let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
  let input = |seconds: Option<u64>, until: Option<&str>| WaitInput {
    seconds,
    until: until.map(str::to_owned),
    note: "n".to_owned(),
  };

  assert_eq!(wait_end(&input(Some(30), None), now), Ok(now + Duration::from_secs(30)));
  assert_eq!(
    wait_end(&input(None, Some("1970-01-01T09:00:10+09:00")), now),
    Ok(SystemTime::UNIX_EPOCH + Duration::from_secs(10)),
  );
  assert_eq!(
    wait_end(&input(Some(1), Some("1970-01-01T00:00:00Z")), now),
    Err("give seconds or until, not both".to_owned())
  );
  assert_eq!(wait_end(&input(None, None), now), Err("give seconds or until".to_owned()));
  assert!(
    wait_end(&input(None, Some("tomorrow at nine")), now)
      .unwrap_err()
      .starts_with("until is not a date")
  );
  assert!(wait_end(&input(Some(u64::MAX), None), now).unwrap_err().contains("past any clock"));
}

#[tokio::test]
async fn a_wait_answers_its_note_once_its_seconds_have_passed() {
  let started = Instant::now();
  let said =
    answered(answer(handed("bmo/wait", json!({ "seconds": 1, "note": "look again" }))).await.unwrap());

  assert!(started.elapsed() >= Duration::from_secs(1));
  assert!(!said.is_error, "{}", said.plain());
  assert!(said.plain().starts_with("look again\nwaited until "), "{}", said.plain());
}

#[tokio::test]
async fn a_moment_already_past_is_answered_at_once_and_a_moment_that_does_not_read_is_refused() {
  let said = call("wait", json!({ "until": "2000-01-01T00:00:00Z", "note": "late" })).await;

  assert_eq!(said.plain(), "late\nwaited until 2000-01-01T00:00:00Z");

  let refused = call("wait", json!({ "until": "soon", "note": "n" })).await;

  assert!(refused.is_error);
  assert!(refused.plain().starts_with("until is not a date"), "{}", refused.plain());
}

#[tokio::test]
async fn a_watch_answers_once_its_check_exits_zero_with_what_it_wrote() {
  let scratch = tempdir().unwrap();
  let flag = scratch.path().join("flag");
  let check = format!("if [ -e {0} ]; then echo found; exit 0; fi; touch {0}; exit 1", flag.display());
  let said = call("watch", json!({ "check": check, "every": 1, "note": "it is there" })).await;

  assert!(!said.is_error, "{}", said.plain());
  assert_eq!(said.plain(), "it is there\nthe check held on run 2; it wrote:\nfound");
}

#[tokio::test]
async fn a_watch_whose_check_never_holds_lapses_within_its_seconds() {
  let started = Instant::now();
  let said = call(
    "watch",
    json!({ "check": "echo not yet; exit 3", "every": 1, "within": 1, "note": "n" }),
  )
  .await;

  assert!(started.elapsed() < Duration::from_secs(5));
  assert!(!said.is_error, "{}", said.plain());
  assert_eq!(
    said.plain(),
    "lapsed: n\nthe check did not hold within 1 seconds, over 2 runs; its last run wrote:\nnot yet"
  );
}

#[tokio::test]
async fn a_watch_with_no_check_and_an_unknown_entry_are_refused() {
  let refused = call("watch", json!({ "check": "  ", "note": "n" })).await;

  assert!(refused.is_error);
  assert_eq!(refused.plain(), "check names no command");

  let unknown = answered(answer(handed("bmo/remind", json!({}))).await.unwrap());

  assert!(unknown.is_error);
  assert_eq!(unknown.plain(), "ERROR: this executable holds no entry bmo/remind");
}

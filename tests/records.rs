//! What a turn's tool calls wrote down: the recording verbs read however their binary was named, the heredoc
//! fed to one, and the calls that record nothing.

use std::collections::HashMap;

use bmo_receiver::chat::records::{RunCall, TurnRecord, record_of};

fn bash(command: &str, result: &str) -> RunCall {
  RunCall {
    name: Some("Bash".to_owned()),
    text: serde_json::json!({ "command": command }).to_string(),
    result: Some(result.to_owned()),
    state: Some("done".to_owned()),
  }
}

fn declared() -> HashMap<String, String> {
  [
    ("memory_add", "memory"),
    ("note_add", "note"),
    ("todo_append", "todo-append"),
    ("watch_add", "watch"),
  ]
  .into_iter()
  .map(|(name, kind)| (name.to_owned(), kind.to_owned()))
  .collect()
}

#[test]
fn a_verb_is_read_however_its_binary_was_named() {
  let note =
    record_of(&bash("D=bin; $D/bmo note --chat 1 --text 'keep it short'", "note 39 kept"), &declared());

  assert_eq!(
    note,
    Some(TurnRecord {
      kind: "note".to_owned(),
      text: "keep it short".to_owned(),
      id: Some(39),
      wakes: false,
    }),
  );

  let memory = record_of(
    &bash(
      "cd x && bun scripts/cargo.ts run -p bmo-server --bin bmo -- memory --add --type situation --condition \"when asked\" --on deployed",
      "memory 12 written",
    ),
    &declared(),
  );

  assert_eq!(
    memory.map(|one| (one.kind, one.text, one.id, one.wakes)),
    Some(("rule".to_owned(), "when asked".to_owned(), Some(12), true)),
  );
}

#[test]
fn a_heredoc_fed_to_the_verb_is_what_it_wrote() {
  let fed =
    record_of(&bash("bmo note --chat 1 <<'EOF'\nthe body\nEOF\necho done", "note 4 kept"), &declared());

  assert_eq!(fed.map(|one| one.text), Some("the body".to_owned()));

  let substituted = record_of(
    &bash("\"bin/bmo\" todo --add --title \"$(cat <<'EOF'\nA title\nEOF\n)\"", "todo: added #699"),
    &declared(),
  );

  assert_eq!(
    substituted.map(|one| (one.kind, one.text, one.id)),
    Some(("todo-add".to_owned(), "A title".to_owned(), Some(699))),
  );
}

#[test]
fn reading_and_failing_calls_record_nothing() {
  assert_eq!(record_of(&bash("bmo todo --list", ""), &declared()), None);
  assert_eq!(record_of(&bash("cd data && ls bin/bmo", ""), &declared()), None);
  assert_eq!(
    record_of(&bash("bun scripts/note.ts --chat 1 --text x", "note 1 kept"), &declared()),
    None,
  );

  let failed = RunCall {
    state: Some("failed".to_owned()),
    ..bash("bmo note --text x", "")
  };

  assert_eq!(record_of(&failed, &declared()), None);
}

#[test]
fn a_tool_call_counts_by_the_record_its_catalogue_declares() {
  let call = |name: &str, args: serde_json::Value, result: &str| RunCall {
    name: Some(name.to_owned()),
    text: args.to_string(),
    result: Some(result.to_owned()),
    state: None,
  };
  let rule = record_of(
    &call(
      "mcp__bmo__memory_add",
      serde_json::json!({ "type": "situation", "condition": " when asked ", "until": "x" }),
      "memory 5 written",
    ),
    &declared(),
  );

  assert_eq!(
    rule,
    Some(TurnRecord {
      kind: "rule".to_owned(),
      text: "when asked".to_owned(),
      id: Some(5),
      wakes: true,
    }),
  );

  let appended = record_of(
    &call("bmo/todo_append", serde_json::json!({ "item": "77", "text": "more" }), "ok"),
    &declared(),
  );

  assert_eq!(appended.map(|one| one.id), Some(Some(77)));
  assert_eq!(record_of(&call("mcp__web__web_fetch", serde_json::json!({}), "ok"), &declared()), None);
  assert_eq!(
    record_of(
      &call("mcp__bmo__note_add", serde_json::json!({ "text": "x" }), "ERROR: no"),
      &declared(),
    ),
    None,
  );
}

#[test]
fn a_command_cut_short_in_the_log_is_read_by_hand() {
  let cut = RunCall {
    name: Some("Bash".to_owned()),
    text: r#"{"command":"bin/bmo note --text \"kept\" --chat 1"#.to_owned(),
    result: None,
    state: None,
  };

  assert_eq!(record_of(&cut, &declared()).map(|one| one.text), Some("kept".to_owned()));
}

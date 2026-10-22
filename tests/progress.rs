//! A turn's run read as it goes: the rollouts drawn as they come and after the turn ends, a failed request and
//! the rollouts' bookkeeping changing nothing, and the loops' steps read as the other program's record.

use serde_json::{Value, json};

use bmo_receiver::chat::progress::{RequestSpend, RunProgress};

fn read_all(mut progress: RunProgress, lines: &[Value]) -> RunProgress {
  for line in lines {
    if let Some(read) = progress.read(&line.to_string()) {
      progress = read;
    }
  }

  progress
}

fn item(payload: Value) -> Value {
  json!({ "type": "response_item", "payload": payload })
}

fn tools(progress: &RunProgress) -> Vec<(Option<&str>, Option<&str>)> {
  progress
    .actions
    .iter()
    .filter(|action| action.kind == "tool")
    .map(|action| (action.name.as_deref(), action.state.as_deref()))
    .collect()
}

#[test]
fn a_pane_program_s_record_draws_the_turn_and_the_usage_it_ended_on() {
  let working = read_all(
    RunProgress::starting(&[]),
    &[
      json!({ "kind": "progress", "id": "turn-1", "phase": "thinking", "tool": null }),
      json!({ "kind": "progress", "id": "turn-1", "phase": "tool", "tool": "bash", "state": "running" }),
      json!({ "kind": "progress", "id": "turn-1", "context": { "used": 1200, "size": 200000 } }),
      json!({ "kind": "progress", "id": "turn-1", "usage": {
        "inputTokens": 300, "outputTokens": 12, "cacheReadTokens": 200,
      } }),
    ],
  );

  assert_eq!((working.phase, working.tool.as_deref()), ("tool", Some("bash")));
  assert_eq!(working.context_tokens, Some(1200));
  assert_eq!(
    working.spent,
    Some(RequestSpend {
      input_tokens: 300,
      cache_read_tokens: 200,
      cache_write_tokens: 0,
    })
  );
  assert_eq!(working.written_tokens, Some(12));

  let answered = read_all(
    working,
    &[json!({
      "kind": "answer", "id": "turn-1", "text": "done",
      "usage": { "inputTokens": 1000, "outputTokens": 40, "cacheReadTokens": 800, "totalTokens": 1040 },
    })],
  );

  assert_eq!(
    answered.spent,
    Some(RequestSpend {
      input_tokens: 1000,
      cache_read_tokens: 800,
      cache_write_tokens: 0,
    })
  );
  assert_eq!(answered.written_tokens, Some(40));
}

#[test]
fn a_codex_rollout_draws_the_turn_as_it_goes_and_after_it_ends() {
  let handed = [("knowledge".to_owned(), "knowledge/sample.md".to_owned(), Some("held"))];
  let working = read_all(
    RunProgress::starting(&handed),
    &[
      json!({ "type": "session_meta", "payload": { "id": "thread-1", "cwd": "/w" } }),
      item(
        json!({ "type": "message", "role": "user", "content": [{ "type": "input_text", "text": "question" }] }),
      ),
      item(json!({ "type": "reasoning", "summary": [], "encrypted_content": "sealed" })),
      item(json!({
        "type": "function_call", "name": "exec_command", "arguments": "{\"cmd\":\"ls\"}", "call_id": "call-1",
      })),
      json!({
        "type": "event_msg",
        "payload": {
          "type": "token_count",
          "info": { "last_token_usage": {
            "input_tokens": 1000, "cached_input_tokens": 900, "cache_write_input_tokens": 0,
            "output_tokens": 20, "reasoning_output_tokens": 8,
          } },
        },
      }),
    ],
  );

  assert_eq!((working.phase, working.tool.as_deref()), ("tool", Some("exec_command")));
  assert_eq!(tools(&working), vec![(Some("exec_command"), None)]);
  assert_eq!(working.actions[0].text, "knowledge/sample.md");
  assert_eq!(
    (working.context_tokens, working.cache_read_tokens, working.thinking_tokens),
    (Some(1000), Some(900), Some(8))
  );
  assert_eq!(
    working.spent,
    Some(RequestSpend {
      input_tokens: 100,
      cache_read_tokens: 900,
      cache_write_tokens: 0
    })
  );

  let ended = read_all(
    working,
    &[
      item(json!({ "type": "function_call_output", "call_id": "call-1", "output": "listed" })),
      json!({ "type": "event_msg", "payload": { "type": "token_count", "info": { "last_token_usage": {
        "input_tokens": 1100, "cached_input_tokens": 1000, "output_tokens": 6,
      } } } }),
      item(
        json!({ "type": "message", "role": "assistant", "content": [{ "type": "output_text", "text": "done here" }] }),
      ),
      json!({ "type": "event_msg", "payload": { "type": "task_complete", "last_agent_message": "done here" } }),
    ],
  );

  assert_eq!((ended.phase, ended.tool.as_deref()), ("writing", None));
  assert_eq!(tools(&ended), vec![(Some("exec_command"), Some("succeeded"))]);
  assert_eq!(ended.context_tokens, Some(1100));
  assert_eq!(
    ended.spent.map(|spent| (spent.input_tokens, spent.cache_read_tokens)),
    Some((200, 1900))
  );
  assert!(ended.written_tokens.unwrap_or(0) > 0);
}

#[test]
fn a_failed_request_and_the_rollouts_bookkeeping_change_nothing() {
  let start = RunProgress::default();

  for line in [
    json!({ "type": "event_msg", "payload": { "type": "token_count", "info": null } }),
    json!({ "type": "event_msg", "payload": { "type": "token_count", "info": { "last_token_usage": { "input_tokens": 0 } } } }),
    json!({ "type": "turn_context", "payload": { "model": "sample-model" } }),
    item(
      json!({ "type": "message", "role": "developer", "content": [{ "type": "input_text", "text": "rules" }] }),
    ),
  ] {
    assert_eq!(start.read(&line.to_string()), None);
  }
}

#[test]
fn the_loops_steps_read_as_claude_codes_record() {
  let read = read_all(
    RunProgress::default(),
    &[
      json!({ "kind": "start", "id": "turn-1", "name": "chat", "title": "sample" }),
      json!({
        "kind": "step", "id": "turn-1", "type": "assistant",
        "message": {
          "id": "message-1",
          "content": [
            { "type": "thinking", "thinking": "looking" },
            { "type": "tool_use", "id": "call-1", "name": "bash", "input": { "command": "ls" } },
          ],
          "usage": { "input_tokens": 30, "cache_read_input_tokens": 70, "output_tokens": 5 },
        },
      }),
    ],
  );

  assert_eq!((read.phase, read.tool.as_deref()), ("tool", Some("bash")));
  assert_eq!(read.context_tokens, Some(100));
  assert_eq!(
    read.spent.as_ref().map(|spent| (spent.input_tokens, spent.cache_read_tokens)),
    Some((30, 70))
  );

  let answered = read_all(
    read,
    &[json!({
      "kind": "step", "id": "turn-1", "type": "user",
      "message": { "content": [{ "type": "tool_result", "tool_use_id": "call-1", "content": "listed", "is_error": false }] },
    })],
  );

  assert_eq!(tools(&answered), vec![(Some("bash"), Some("succeeded"))]);
}

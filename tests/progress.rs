//! A turn's run read as it goes: the rollouts drawn as they come and after the turn ends, a failed request and
//! the rollouts' bookkeeping changing nothing, the loops' steps read as the other program's record, and the
//! lines a turn writes and then goes on working shown as its progress lines, its answer and its thinking never.

use serde_json::{Value, json};

use bmo_receiver::chat::progress::{
  MAX_SAID_LINES, RequestSpend, RunProgress, SAID_CHARS, Told, picture_of,
};
use bmo_receiver::secrets::marker;

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

// The live figures move only forward while a turn runs: the running totals hold a report of the whole
// spend at the greater of it and what was counted, and the answer's measured figures replace them.
#[test]
fn a_report_of_the_turn_s_whole_spend_never_takes_the_live_figure_back() {
  let working = read_all(
    RunProgress::default(),
    &[
      json!({ "kind": "progress", "id": "turn-1", "usage": {
        "inputTokens": 400, "outputTokens": 120, "cacheReadTokens": 900,
      } }),
      json!({ "kind": "progress", "id": "turn-1", "usage": {
        "inputTokens": 500, "outputTokens": 30, "cacheReadTokens": 1_000,
      } }),
    ],
  );

  assert_eq!(
    working.spent,
    Some(RequestSpend {
      input_tokens: 500,
      cache_read_tokens: 1_000,
      cache_write_tokens: 0,
    }),
    "the spend and how full the session stands are set where the newest report stands"
  );
  assert_eq!(working.written_tokens, Some(120), "the written figure holds the greater of the reports");

  let answered = read_all(
    working,
    &[json!({
      "kind": "answer", "id": "turn-1", "text": "done",
      "usage": { "inputTokens": 900, "outputTokens": 210, "cacheReadTokens": 1_200 },
    })],
  );

  assert_eq!(
    answered.written_tokens,
    Some(210),
    "the turn's measured whole spend replaces the running total"
  );
}

#[test]
fn a_progress_report_and_the_estimates_its_run_files_keep_one_running_figure() {
  let counting = read_all(
    RunProgress::default(),
    &[json!({ "kind": "progress", "id": "turn-1", "usage": {
        "inputTokens": 300, "outputTokens": 12, "cacheReadTokens": 200,
      } })],
  );

  assert_eq!(counting.written_tokens, Some(12));

  let counted = filed_all(
    counting.clone(),
    &[json!({ "id": 2, "kind": "tool", "name": "Bash", "text": "ls docs", "ref": "call-1" })],
  );

  assert_eq!(
    counted.written_tokens,
    Some(15),
    "the call's estimate is added to the whole spend so far"
  );

  let restated = read_all(
    counted,
    &[json!({ "kind": "progress", "id": "turn-1", "usage": {
      "inputTokens": 600, "outputTokens": 8, "cacheReadTokens": 300,
    } })],
  );

  assert_eq!(
    restated.written_tokens,
    Some(15),
    "the report of a smaller whole spend leaves the running figure where it reached",
  );
}

// The thinking count the program states as a running total stands the picture at the greater of it and
// what was counted; the stretches it says separately are added.
#[test]
fn the_thinking_count_a_program_reports_moves_only_forward() {
  let running = read_all(
    RunProgress::default(),
    &[
      json!({ "type": "system", "subtype": "thinking_tokens", "estimated_tokens_delta": 40 }),
      json!({ "type": "system", "subtype": "thinking_tokens", "estimated_tokens": 100 }),
      json!({ "type": "system", "subtype": "thinking_tokens", "estimated_tokens": 90 }),
    ],
  );

  assert_eq!(running.thinking_tokens, Some(100), "a running total stands where it is the greater");
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

fn said(progress: &RunProgress) -> Vec<(&str, usize)> {
  progress.said.iter().map(|line| (line.text.as_str(), line.before)).collect()
}

fn assistant(id: &str, content: Value) -> Value {
  json!({ "type": "assistant", "message": { "id": id, "content": content } })
}

#[test]
fn a_text_block_a_call_follows_is_a_progress_line_and_the_last_one_is_the_answer() {
  let handed = [("room".to_owned(), "chat 42".to_owned(), None)];
  let working = read_all(
    RunProgress::starting(&handed),
    &[
      assistant("message-1", json!([{ "type": "thinking", "thinking": "weighing what to read" }])),
      assistant("message-1", json!([{ "type": "text", "text": "Reading the plan first." }])),
      assistant(
        "message-1",
        json!([{ "type": "tool_use", "id": "call-1", "name": "Read", "input": { "file_path": "plan.md" } }]),
      ),
      json!({ "type": "user", "message": { "content": [
        { "type": "tool_result", "tool_use_id": "call-1", "content": "the plan", "is_error": false },
      ] } }),
      assistant(
        "message-2",
        json!([
          { "type": "text", "text": "The plan names two steps;\n\n  checking the second." },
          { "type": "tool_use", "id": "call-2", "name": "Bash", "input": { "command": "ls" } },
        ]),
      ),
    ],
  );

  assert_eq!(
    said(&working),
    vec![("Reading the plan first.", 1), ("The plan names two steps; checking the second.", 2)]
  );
  assert_eq!(tools(&working), vec![(Some("Read"), Some("succeeded")), (Some("Bash"), None)]);

  let answered = read_all(
    working.clone(),
    &[assistant("message-3", json!([{ "type": "text", "text": "Both steps are done." }]))],
  );

  assert_eq!(answered.phase, "writing");
  assert_eq!(answered.said, working.said);
  assert!(!serde_json::to_string(&answered).unwrap().contains("Both steps are done."));
}

#[test]
fn thinking_and_a_failed_request_s_text_are_no_progress_lines() {
  let read = read_all(
    RunProgress::default(),
    &[
      assistant("message-1", json!([{ "type": "thinking", "thinking": "a long thought" }])),
      json!({
        "type": "assistant", "isApiErrorMessage": true,
        "message": { "id": "message-2", "content": [{ "type": "text", "text": "API Error: overloaded" }] },
      }),
      assistant(
        "message-3",
        json!([{ "type": "tool_use", "id": "call-1", "name": "Bash", "input": {} }]),
      ),
    ],
  );

  assert!(read.said.is_empty(), "{:?}", read.said);
  assert_eq!(tools(&read), vec![(Some("Bash"), None)]);
}

#[test]
fn a_rollout_s_message_a_call_follows_is_a_progress_line_and_its_last_message_is_not() {
  let read = read_all(
    RunProgress::default(),
    &[
      item(json!({ "type": "reasoning", "summary": [{ "type": "summary_text", "text": "planning" }] })),
      item(
        json!({ "type": "message", "role": "assistant", "content": [{ "type": "output_text", "text": "Listing the folder." }] }),
      ),
      item(
        json!({ "type": "function_call", "name": "exec_command", "arguments": "{\"cmd\":\"ls\"}", "call_id": "call-1" }),
      ),
      item(json!({ "type": "function_call_output", "call_id": "call-1", "output": "listed" })),
      item(
        json!({ "type": "message", "role": "assistant", "content": [{ "type": "output_text", "text": "The folder holds two files." }] }),
      ),
    ],
  );

  assert_eq!(said(&read), vec![("Listing the folder.", 0)]);
}

#[test]
fn a_progress_line_is_redacted_cut_and_only_the_last_few_are_kept() {
  let secret = format!("{}={}", "SAMPLE_API_KEY", "abcdefghijklmnop");
  let long = "word ".repeat(SAID_CHARS);
  let mut lines = vec![
    assistant(
      "message-0",
      json!([{ "type": "text", "text": format!("Setting {secret} for the road.") }]),
    ),
    assistant(
      "message-0",
      json!([{ "type": "tool_use", "id": "call-0", "name": "Bash", "input": {} }]),
    ),
    assistant("message-1", json!([{ "type": "text", "text": long }])),
    assistant(
      "message-1",
      json!([{ "type": "tool_use", "id": "call-1", "name": "Bash", "input": {} }]),
    ),
  ];
  let read = read_all(RunProgress::default(), &lines);

  assert_eq!(read.said[0].text, format!("Setting SAMPLE_API_KEY={} for the road.", marker()));
  assert!(!serde_json::to_string(&read).unwrap().contains("abcdefghijklmnop"));
  assert_eq!(read.said[1].text.chars().count(), SAID_CHARS + 1);
  assert!(read.said[1].text.ends_with('…'));

  for at in 2..(MAX_SAID_LINES + 3) {
    lines.push(assistant(
      &format!("message-{at}"),
      json!([{ "type": "text", "text": format!("Step {at}.") }]),
    ));
    lines.push(assistant(
      &format!("message-{at}"),
      json!([{ "type": "tool_use", "id": format!("call-{at}"), "name": "Bash", "input": {} }]),
    ));
  }

  let read = read_all(RunProgress::default(), &lines);
  let last = MAX_SAID_LINES + 2;

  assert_eq!(read.said.len(), MAX_SAID_LINES);
  assert_eq!(
    read.said.last().map(|line| (line.text.clone(), line.before)),
    Some((format!("Step {last}."), last))
  );
  assert_ne!(read.drawn(), RunProgress::default().drawn());
}

// A road whose program keeps its own record is filed by the agency as lines already told apart, each holding its
// words and, in its metadata, the request its message answers (docs/receiver.md, 「The event stream」).
#[test]
fn a_filed_line_counts_the_request_its_message_answers_once_by_its_id() {
  let request = |id: &str, input: i64, read: i64, written: i64| {
    json!({ "request": { "id": id, "usage": {
      "input_tokens": input, "output_tokens": 5, "cache_read_input_tokens": read, "cache_creation_input_tokens": written
    } } })
  };
  let read = filed_all(
    RunProgress::default(),
    &[
      json!({ "kind": "thinking", "text": "considering", "metadata": request("m1", 10, 900, 30) }),
      json!({ "kind": "text", "text": "words", "metadata": request("m1", 10, 900, 30) }),
      json!({ "kind": "tool", "name": "Bash", "text": "ls", "ref": "t1", "metadata": request("m2", 4, 940, 6) }),
      json!({ "kind": "tool-result", "ref": "t1", "state": "succeeded" }),
    ],
  );

  assert_eq!(
    read.context_tokens,
    Some(4 + 940 + 6),
    "the session stands as the latest request found it"
  );
  assert_eq!(read.cache_read_tokens, Some(940));
  assert_eq!(read.request_id.as_deref(), Some("m2"));

  let spent = read.spent.expect("what the turn spent");

  assert_eq!(
    (spent.input_tokens, spent.cache_read_tokens, spent.cache_write_tokens),
    (10 + 4, 900 + 940, 30 + 6),
    "each request is added once, however many of its lines were filed"
  );

  // A line that names no request leaves the figures as they stand.
  let bare = filed_all(RunProgress::default(), &[json!({ "kind": "text", "text": "words" })]);

  assert_eq!((bare.context_tokens, bare.spent), (None, None));
}

fn filed_all(mut progress: RunProgress, lines: &[Value]) -> RunProgress {
  for line in lines {
    if let Some(read) = progress.filed(line.as_object().unwrap()) {
      progress = read;
    }
  }

  progress
}

#[test]
fn lines_filed_already_told_apart_move_the_phase_the_call_and_the_estimates() {
  let thinking = filed_all(
    RunProgress::default(),
    &[json!({ "id": 1, "kind": "thinking", "text": "weighing what the room asked" })],
  );

  assert_eq!((thinking.phase, thinking.tool.as_deref()), ("thinking", None));
  assert!(thinking.thinking_tokens.is_some_and(|tokens| tokens > 0));
  assert_eq!(thinking.written_tokens, None);

  let calling = filed_all(
    thinking,
    &[
      json!({ "id": 2, "kind": "text", "text": "Reading the plan first." }),
      json!({ "id": 3, "kind": "tool", "name": "Bash", "text": "ls docs", "ref": "call-1" }),
    ],
  );

  assert_eq!((calling.phase, calling.tool.as_deref()), ("tool", Some("Bash")));
  assert_eq!(tools(&calling), [(Some("Bash"), None)]);
  assert_eq!(
    calling.said.iter().map(|line| line.text.as_str()).collect::<Vec<_>>(),
    ["Reading the plan first."]
  );
  assert!(calling.written_tokens.is_some_and(|tokens| tokens > 0));

  let settled = filed_all(
    calling.clone(),
    &[
      json!({ "id": 4, "kind": "tool-result", "text": "ran 12 ms", "ref": "call-1", "state": "succeeded" }),
    ],
  );

  assert_eq!(tools(&settled), [(Some("Bash"), Some("succeeded"))]);

  let answering =
    filed_all(settled, &[json!({ "id": 5, "kind": "text", "text": "Here is what was asked." })]);

  assert_eq!((answering.phase, answering.tool.as_deref()), ("writing", None));
  // The turn's last words are its answer and no progress line.
  assert_eq!(answering.said.len(), 1);
  // Nothing of the session's fullness or a request's spend is stated on such a road.
  assert_eq!((answering.context_tokens, answering.spent.clone()), (None, None));
}

#[test]
fn a_failed_line_and_a_line_of_no_such_kind_move_nothing() {
  let start = RunProgress::default();

  for line in [
    json!({ "id": 1, "kind": "text", "text": "a refused request", "state": "failed" }),
    json!({ "id": 2, "kind": "thinking", "text": "a refused thought", "state": "failed" }),
    json!({ "id": 3, "kind": "end", "text": "ended" }),
    json!({ "id": 4, "kind": "note", "text": "a word of the bot's own" }),
    json!({ "id": 5, "text": "no kind at all" }),
    json!({ "id": 6, "kind": "tool-result", "text": "ran", "ref": "no-such-call", "state": "succeeded" }),
  ] {
    assert_eq!(start.filed(line.as_object().unwrap()), None, "{line}");
  }
}

// "the block a turn is still writing stands in the picture by estimate on top of what the lines said, under
// thinking for a thinking block and under the written figure otherwise"
#[test]
fn the_block_still_being_written_counts_on_top_of_what_the_lines_said() {
  let filed = RunProgress::default()
    .filed(json!({ "kind": "text", "text": "x".repeat(40) }).as_object().unwrap())
    .unwrap();
  let written = filed.written_tokens.unwrap();

  let thinking = picture_of(&filed.with_open(Some(("thinking", 400))));
  let words = picture_of(&filed.with_open(Some(("text", 400))));

  assert!(thinking["thinkingTokens"].as_i64().unwrap() > 0, "{thinking}");
  assert_eq!(thinking["writtenTokens"], json!(written), "{thinking}");
  assert!(words["writtenTokens"].as_i64().unwrap() > written, "{words}");
  assert_eq!(words["thinkingTokens"], Value::Null, "{words}");
}

// A block that ended is let go of before its line comes, and its line may say less than the bytes it streamed —
// a call's line holds a summary of its arguments — so the figures a look drew hold until the lines pass them.
#[test]
fn a_block_that_ended_keeps_what_its_look_drew_until_the_lines_pass_it() {
  let drawn = |picture: &RunProgress, name: &str| picture_of(picture)[name].as_i64().unwrap_or(0);
  let thinking = RunProgress::default().with_open(Some(("thinking", 600)));
  let calling = thinking.with_open(Some(("tool", 3_000)));
  let between = calling.with_open(None);
  let filed = between
    .filed(
      json!({ "kind": "tool", "name": "Write", "text": "notes.md", "ref": "call-1" })
        .as_object()
        .unwrap(),
    )
    .unwrap()
    .with_open(None);

  assert_eq!(drawn(&calling, "thinkingTokens"), 200, "the ended thinking block keeps its estimate");
  assert_eq!(drawn(&between, "writtenTokens"), 1_000, "the ended call keeps its estimate");
  assert_eq!(drawn(&filed, "writtenTokens"), 1_000, "a call's shorter line takes nothing back");

  let words = filed
    .filed(json!({ "kind": "text", "text": "y".repeat(3_300) }).as_object().unwrap())
    .unwrap()
    .with_open(None);

  assert_eq!(
    drawn(&words, "writtenTokens"),
    3 + 1_100,
    "the lines move the figure on once they pass it"
  );
}

// A request reports what it wrote on each of its lines, a later line of it the larger figure, and its
// measured output covers its thinking, its words and its calls whatever the lines' estimates came to.
#[test]
fn the_requests_measured_output_adds_up_once_each_beside_the_estimates() {
  let request = |id: &str, output: i64| json!({ "request": { "id": id, "usage": { "input_tokens": 1, "output_tokens": output } } });
  let read = filed_all(
    RunProgress::default(),
    &[
      json!({ "kind": "thinking", "text": "z".repeat(30), "metadata": request("m1", 4) }),
      json!({ "kind": "tool", "name": "Write", "text": "notes.md", "ref": "t1", "metadata": request("m1", 500) }),
      json!({ "kind": "tool-result", "ref": "t1", "state": "succeeded" }),
      json!({ "kind": "text", "text": "done", "metadata": request("m2", 30) }),
    ],
  );

  assert_eq!(read.thinking_tokens, Some(10));
  assert_eq!(
    read.written_tokens,
    Some(500 + 30 - 10),
    "each request counted once at its largest report, the thinking counted taken off"
  );

  // An agent's own stream states the same per request, on each event of its message.
  let message = |id: &str, output: i64| {
    json!({ "type": "assistant", "message": { "id": id, "usage": { "input_tokens": 1, "output_tokens": output },
      "content": [{ "type": "text", "text": "ok" }] } })
  };
  let streamed =
    read_all(RunProgress::default(), &[message("m1", 3), message("m1", 60), message("m2", 40)]);

  assert_eq!(streamed.written_tokens, Some(100));
}

#[test]
fn what_a_turn_wrote_before_words_handed_over_opened_the_next_is_its_answer_and_no_line() {
  let read = read_all(
    RunProgress::default(),
    &[
      assistant("message-1", json!([{ "type": "text", "text": "<<SEND>> the reply written first" }])),
      json!({ "type": "user", "message": { "content": "MORE FOR THIS TURN.\n1 quiet instruction" } }),
      assistant("message-2", json!([{ "type": "text", "text": "Looking at the job." }])),
      assistant(
        "message-2",
        json!([{ "type": "tool_use", "id": "call-1", "name": "Bash", "input": {} }]),
      ),
      json!({ "type": "user", "message": { "content": [
        { "type": "tool_result", "tool_use_id": "call-1", "content": "ok", "is_error": false },
      ] } }),
      assistant("message-3", json!([{ "type": "text", "text": "<<SKIP: the job passed its check>>" }])),
      json!({ "type": "user", "message": { "content": [{ "type": "text", "text": "REVIEW ROUND." }] } }),
      assistant(
        "message-4",
        json!([{ "type": "tool_use", "id": "call-2", "name": "Read", "input": {} }]),
      ),
    ],
  );

  assert_eq!(said(&read), vec![("Looking at the job.", 0)]);
  assert!(!serde_json::to_string(&read).unwrap().contains("<<"), "{:?}", read.said);
}

// The progress gap the three cases below are told against, and a moment inside and past it.
const GAP: i64 = 1_000;

// "one the screens were told already is never told again, however long the turn works or waits"
#[test]
fn a_picture_the_screens_were_told_is_never_told_again() {
  let mut told = Told::default();
  let waiting = RunProgress::default();

  assert_eq!(told.tell(&waiting, 0, GAP), Some(picture_of(&waiting)));
  assert_eq!(told.tell(&waiting, GAP / 2, GAP), None);
  assert_eq!(told.tell(&waiting, GAP * 3_600, GAP), None, "an hour's wait tells nothing again");
}

// "one whose drawn part changed goes out at once"
#[test]
fn a_picture_whose_drawn_part_changed_is_told_at_once() {
  let mut told = Told::default();
  let thinking = RunProgress::default();
  let mut calling = thinking.clone();

  calling.phase = "tool";
  calling.tool = Some("Read".into());

  told.tell(&thinking, 0, GAP);

  assert_eq!(told.tell(&calling, 1, GAP), Some(picture_of(&calling)));
}

// "one whose counts alone moved goes out at most once the progress gap after the last that went out"
#[test]
fn a_picture_whose_counts_alone_moved_is_told_at_most_once_a_gap() {
  let mut told = Told::default();
  let counted = |tokens: i64| {
    let mut progress = RunProgress::default();

    progress.thinking_tokens = Some(tokens);
    progress
  };

  told.tell(&counted(10), 0, GAP);

  assert_eq!(told.tell(&counted(20), GAP - 1, GAP), None);
  assert_eq!(told.tell(&counted(30), GAP, GAP), Some(picture_of(&counted(30))));
  assert_eq!(told.tell(&counted(40), GAP + 1, GAP), None, "the gap counts from the last told");
}

// "a text holding one of a round's answer marks is a draft of the turn's message and never a
// progress line, even where a call follows it; it is left out whole", a reason the line's bound would cut
// still counting.
#[test]
fn a_draft_holding_an_answer_mark_is_no_progress_line_even_where_a_call_follows_it() {
  let reason = "the room asked nothing new ".repeat(20);
  let working = read_all(
    RunProgress::default(),
    &[
      assistant(
        "message-1",
        json!([
          { "type": "text", "text": "<<SEND>> draft" },
          { "type": "tool_use", "id": "call-1", "name": "assign_worker", "input": {} },
        ]),
      ),
      assistant(
        "message-2",
        json!([
          { "type": "text", "text": format!("Holding back: <<SKIP: {reason}>>") },
          { "type": "tool_use", "id": "call-2", "name": "Read", "input": { "file_path": "plan.md" } },
          { "type": "text", "text": "Answering it as <<REPLY #4>> asks." },
          { "type": "tool_use", "id": "call-3", "name": "Read", "input": { "file_path": "plan.md" } },
        ]),
      ),
    ],
  );
  let picture = picture_of(&working).to_string();

  assert!(working.said.is_empty(), "{:?}", working.said);
  assert_eq!(tools(&working).len(), 3, "the calls still stand");
  assert!(!picture.contains("<<") && !picture.contains("draft"), "{picture}");
}

// "a shell here-document's opening (`<<EOF`) is none"
#[test]
fn a_here_document_s_opening_is_no_answer_mark() {
  let working = read_all(
    RunProgress::default(),
    &[assistant(
      "message-1",
      json!([
        { "type": "text", "text": "Writing the notes with cat <<EOF into a file." },
        { "type": "tool_use", "id": "call-1", "name": "Bash", "input": { "command": "ls" } },
      ]),
    )],
  );

  assert_eq!(said(&working), vec![("Writing the notes with cat <<EOF into a file.", 0)]);
}

// "a newer block takes its place, except one holding an answer mark, which is passed over"
#[test]
fn a_thinking_block_holding_an_answer_mark_is_passed_over() {
  let thought = RunProgress::default().thought("weighing what the room asked");
  let drafted = thought.clone().thought("<<SEND>> the message as it would go out");

  assert_eq!(drafted.thinking.as_deref(), Some("weighing what the room asked"));
  assert!(!picture_of(&drafted).to_string().contains("<<SEND"), "{}", picture_of(&drafted));
}

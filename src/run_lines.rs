//! What one line of a run's log says: a Claude Code record's line, or a Codex thread's rollout line, read into
//! the pieces a turn's picture is built from (the agency's `docs/run-records.md`, 「Agent log」). The agency writes these lines
//! and masks what stood where a credential did; this only reads them.

use std::sync::LazyLock;

use regex::Regex;

use crate::json::Json;

/// The tools that open a branch: a co-worker, or a research run's pooled branch.
pub const BRANCH_TOOLS: [&str; 3] = ["Task", "Agent", "mcp__web__research_branch"];

// The toolbook proxy's own server, and the calls that stand in for another tool.
const TOOLBOOK: &str = "toolbook";
const PROXIES: [&str; 3] = ["call", "act", "run"];

static CLAUDE_MCP_NAME: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^mcp__([^_]+(?:_[^_]+)*)__(.+)$").expect("an MCP name pattern"));

static CODEX_MCP_NAME: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"^([A-Za-z0-9][A-Za-z0-9_.-]*)/([A-Za-z0-9_.-]+)$").expect("an MCP name pattern")
});

/// The tool a call reached: a toolbook proxy call answered as the inner call, in the same road's spelling;
/// none where the call is its own.
pub fn reached(name: &str, inner: &str) -> Option<String> {
  let (server, tool) = match (CLAUDE_MCP_NAME.captures(name), CODEX_MCP_NAME.captures(name)) {
    (Some(found), _) => (found[1].to_owned(), found[2].to_owned()),
    (_, Some(found)) => (found[1].to_owned(), found[2].to_owned()),
    _ => return None,
  };

  if server != TOOLBOOK || !PROXIES.contains(&tool.as_str()) {
    return None;
  }

  let found = CODEX_MCP_NAME.captures(inner)?;

  Some(if CLAUDE_MCP_NAME.is_match(name) {
    format!("mcp__{}__{}", &found[1], &found[2])
  } else {
    format!("{}/{}", &found[1], &found[2])
  })
}

/// One line of a run's log, as it is read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Line {
  pub kind: &'static str,
  pub text: String,
  pub name: Option<String>,
  pub reference: Option<String>,
  pub state: Option<&'static str>,
  pub source_block: Option<i64>,
  pub source_id: Option<String>,
}

/// A call as the log records it: the tool it reached and what that tool was handed.
pub struct CalledTool {
  pub name: String,
  pub input: Json,
  pub through_toolbook: bool,
}

pub fn called_tool(name: &str, input: &Json) -> CalledTool {
  let inner = input.get("tool").and_then(Json::as_str);
  let arguments = input.get("arguments").filter(|arguments| arguments.is_object());

  match (inner.and_then(|inner| reached(name, inner)), arguments) {
    (Some(reached), Some(arguments)) => CalledTool {
      name: reached.to_owned(),
      input: arguments.clone(),
      through_toolbook: true,
    },
    _ => CalledTool {
      name: name.to_owned(),
      input: input.clone(),
      through_toolbook: false,
    },
  }
}

fn tool_name(called: &CalledTool) -> String {
  match called.input.get("subagent_type").and_then(Json::as_str) {
    Some(kind) if BRANCH_TOOLS.contains(&called.name.as_str()) && !kind.is_empty() => {
      format!("{}({kind})", called.name)
    }
    _ => called.name.clone(),
  }
}

fn text_field<'a>(value: &'a Json, name: &str) -> Option<&'a str> {
  value.get(name).and_then(Json::as_str)
}

// A tool's answer as the text it reads as.
fn flatten(content: Option<&Json>) -> String {
  match content {
    None | Some(Json::Null) => String::new(),
    Some(Json::String(text)) => text.clone(),
    Some(Json::Array(parts)) => parts.iter().map(|part| flatten(Some(part))).collect::<Vec<_>>().join("\n"),
    Some(block @ Json::Object(_)) => block
      .get("text")
      .and_then(Json::as_str)
      .map_or_else(|| block.stringify(), str::to_owned),
    Some(other) => other.stringify(),
  }
}

fn result_state(block: &Json, result: Option<&Json>) -> &'static str {
  match block.get("is_error").and_then(Json::as_bool) {
    Some(true) => return "failed",
    Some(false) => return "succeeded",
    None => {}
  }

  let Some(structured) = result.filter(|result| result.is_object()) else {
    return "unverified";
  };
  let status = text_field(structured, "status").unwrap_or_default().to_lowercase();
  let flag = |name: &str| structured.get(name).and_then(Json::as_bool);

  if flag("is_error") == Some(true)
    || flag("isError") == Some(true)
    || flag("success") == Some(false)
    || structured.get("error").is_some()
    || ["failed", "error", "errored", "cancelled"].contains(&status.as_str())
  {
    return "failed";
  }

  let kind = text_field(structured, "type").unwrap_or_default().to_lowercase();
  let applied = structured.get("structuredPatch").and_then(Json::as_array).is_some()
    || (text_field(structured, "filePath").is_some()
      && ["create", "update", "delete", "write"].contains(&kind.as_str()));

  if flag("success") == Some(true)
    || applied
    || ["completed", "succeeded", "success"].contains(&status.as_str())
  {
    return "succeeded";
  }

  if status == "running" || status == "async_launched" { "running" } else { "unverified" }
}

/// The lines one record line of a Claude Code program leaves: what the run said and thought, each tool it
/// called, and what each call handed back.
pub fn record_lines(event: &Json) -> Vec<Line> {
  if !event.is_object() {
    return vec![];
  }

  let message = event.get("message");
  let blocks = message
    .and_then(|message| message.get("content"))
    .and_then(Json::as_array)
    .unwrap_or_default();
  let source_id = message
    .and_then(|message| text_field(message, "id"))
    .or_else(|| text_field(event, "uuid"))
    .map(str::to_owned);

  match text_field(event, "type") {
    Some("assistant") => assistant_lines(
      blocks,
      source_id,
      event.get("isApiErrorMessage").and_then(Json::as_bool) == Some(true),
    ),
    Some("user") => result_lines(event, blocks, source_id),
    _ => vec![],
  }
}

fn assistant_lines(blocks: &[Json], source_id: Option<String>, failed: bool) -> Vec<Line> {
  let state = failed.then_some("failed");
  let mut lines: Vec<Line> = vec![];

  for (place, block) in blocks.iter().enumerate() {
    let source_block = Some(place as i64);

    match text_field(block, "type") {
      Some("text") if text_field(block, "text").is_some() => lines.push(Line {
        kind: "text",
        text: text_field(block, "text").unwrap_or_default().to_owned(),
        source_block,
        source_id: source_id.clone(),
        state,
        ..Line::default()
      }),
      Some("thinking") if text_field(block, "thinking").is_some_and(|text| !text.is_empty()) => {
        lines.push(Line {
          kind: "thinking",
          text: text_field(block, "thinking").unwrap_or_default().to_owned(),
          source_block,
          source_id: source_id.clone(),
          state,
          ..Line::default()
        })
      }
      Some("tool_use") => {
        let empty = Json::Object(vec![]);
        let called =
          called_tool(text_field(block, "name").unwrap_or_default(), block.get("input").unwrap_or(&empty));

        lines.push(Line {
          kind: "tool",
          name: Some(tool_name(&called)),
          reference: text_field(block, "id").map(str::to_owned),
          source_block,
          source_id: source_id.clone(),
          state: Some("requested"),
          text: called.input.stringify(),
        });
      }
      _ => {}
    }
  }

  lines
}

fn result_lines(event: &Json, blocks: &[Json], source_id: Option<String>) -> Vec<Line> {
  let result = event.get("toolUseResult");
  let mut lines: Vec<Line> = vec![];

  for (place, block) in blocks.iter().enumerate() {
    if text_field(block, "type") != Some("tool_result") {
      continue;
    }

    lines.push(Line {
      kind: "tool-result",
      reference: text_field(block, "tool_use_id").map(str::to_owned),
      source_block: Some(place as i64),
      source_id: source_id.clone(),
      state: Some(result_state(block, result)),
      text: flatten(block.get("content")),
      ..Line::default()
    });
  }

  lines
}

/// What one line of a thread's rollout is, where it is one of the turn's own response items.
pub fn rollout_item(record: &Json) -> Option<&Json> {
  (text_field(record, "type") == Some("response_item"))
    .then(|| record.get("payload"))
    .flatten()
    .filter(|item| item.is_object())
}

/// What a tool call of the rollout was handed.
pub fn rollout_input(item: &Json) -> Json {
  match (text_field(item, "arguments"), text_field(item, "input")) {
    (Some(arguments), _) => Json::parse(arguments).unwrap_or_else(|| Json::String(arguments.to_owned())),
    (None, Some(input)) => Json::String(input.to_owned()),
    (None, None) => Json::Object(vec![]),
  }
}

/// The lines one line of a thread's rollout leaves.
pub fn rollout_lines(record: &Json) -> Vec<Line> {
  let Some(item) = rollout_item(record) else {
    return vec![];
  };
  let source_id = text_field(item, "id").map(str::to_owned);
  let call = text_field(item, "call_id").map(str::to_owned);

  match text_field(item, "type").unwrap_or_default() {
    "message" if text_field(item, "role") == Some("assistant") => item
      .get("content")
      .and_then(Json::as_array)
      .unwrap_or_default()
      .iter()
      .enumerate()
      .filter(|(_, block)| text_field(block, "type") == Some("output_text"))
      .filter_map(|(place, block)| {
        text_field(block, "text").filter(|text| !text.is_empty()).map(|text| Line {
          kind: "text",
          text: text.to_owned(),
          source_block: Some(place as i64),
          source_id: source_id.clone(),
          ..Line::default()
        })
      })
      .collect(),
    "reasoning" => {
      let said: Vec<&str> = item
        .get("summary")
        .and_then(Json::as_array)
        .unwrap_or_default()
        .iter()
        .filter_map(|part| text_field(part, "text"))
        .filter(|text| !text.is_empty())
        .collect();

      if said.is_empty() {
        vec![]
      } else {
        vec![Line {
          kind: "thinking",
          text: said.join("\n\n"),
          source_id,
          ..Line::default()
        }]
      }
    }
    "function_call" | "custom_tool_call" => {
      let called = called_tool(text_field(item, "name").unwrap_or("tool"), &rollout_input(item));

      vec![Line {
        kind: "tool",
        name: Some(called.name.clone()),
        reference: call,
        source_id,
        state: Some("requested"),
        text: called.input.as_str().map_or_else(|| called.input.stringify(), str::to_owned),
        ..Line::default()
      }]
    }
    "function_call_output" | "custom_tool_call_output" => vec![Line {
      kind: "tool-result",
      reference: call,
      source_id,
      state: Some("unverified"),
      text: flatten(item.get("output")),
      ..Line::default()
    }],
    _ => vec![],
  }
}

/// The calls among a line's pieces, as the actions a turn's picture keeps of them.
pub fn called(lines: &[Line]) -> Vec<Line> {
  lines.iter().filter(|line| line.kind == "tool").cloned().collect()
}

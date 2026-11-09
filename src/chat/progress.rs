//! What a turn of a conversational agent is doing, read off the lines its run leaves in the agency's log as they
//! are written (the agency's `docs/run-records.md`, 「Agent log」; docs/receiver.md, 「The event stream」): whether it thinks, writes or calls a tool, the thinking it
//! counted, what its last request carried and what its requests spent, an estimate of what it wrote, the last
//! few things it did — what it was handed and each call with how it ended — and the last few lines it wrote and
//! then went on working, so a room watching the console sees the turn as it goes.

use serde::Serialize;

use super::talk::clip;
use crate::js::{is_space, trim};
use crate::json::Json;
use crate::run_lines::{Line, called_tool, record_lines, rollout_input, rollout_item, rollout_lines};
use crate::secrets;
use serde_json::{Value, json};

/// How many of a turn's actions the picture keeps.
pub const MAX_TURN_ACTIONS: usize = 8;

/// How many of a turn's progress lines the picture keeps.
pub const MAX_SAID_LINES: usize = 3;

/// How many characters of a progress line the picture keeps, an ellipsis marking the cut.
pub const SAID_CHARS: usize = 240;

// How many bytes of what a turn writes make a token, by estimate.
const BYTES_PER_TOKEN: usize = 3;

/// One thing a turn did: something it was handed, or a call it made with how it ended.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnAction {
  pub kind: &'static str,
  pub name: Option<String>,
  pub text: String,
  pub state: Option<String>,
  #[serde(rename = "ref")]
  pub reference: Option<String>,
}

/// A line the turn wrote and then went on working, shown to the room while the turn works: its words, run into one
/// line, and how many actions the turn had made when the call it led to was made, those dropped counted.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaidLine {
  pub text: String,
  pub before: usize,
}

/// What the turn's requests have reported, each added once.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestSpend {
  pub input_tokens: i64,
  pub cache_read_tokens: i64,
  pub cache_write_tokens: i64,
}

/// The picture of a turn as it goes.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunProgress {
  pub phase: &'static str,
  pub tool: Option<String>,
  pub thinking_tokens: Option<i64>,
  pub context_tokens: Option<i64>,
  pub cache_read_tokens: Option<i64>,
  pub request_id: Option<String>,
  pub spent: Option<RequestSpend>,
  pub written_tokens: Option<i64>,
  pub actions: Vec<TurnAction>,
  pub dropped_actions: usize,
  pub dropped_context: usize,
  pub said: Vec<SaidLine>,
  // What the turn wrote since its last call, which a call coming after it makes progress lines of and which is
  // otherwise its answer.
  #[serde(skip)]
  held: Vec<String>,
}

impl Default for RunProgress {
  fn default() -> Self {
    Self {
      phase: "thinking",
      tool: None,
      thinking_tokens: None,
      context_tokens: None,
      cache_read_tokens: None,
      request_id: None,
      spent: None,
      written_tokens: None,
      actions: vec![],
      dropped_actions: 0,
      dropped_context: 0,
      said: vec![],
      held: vec![],
    }
  }
}

impl RunProgress {
  /// The picture a turn starts from: what it was handed, by where it came from, what it is and how it went in.
  pub fn starting(handed: &[(String, String, Option<&str>)]) -> Self {
    Self::default().with_actions(
      handed
        .iter()
        .map(|(source, text, state)| TurnAction {
          kind: "context",
          name: Some(source.clone()),
          text: text.clone(),
          state: state.map(str::to_owned),
          reference: None,
        })
        .collect(),
    )
  }

  // The picture with more actions on it, the oldest dropped where the bound is met and counted.
  fn with_actions(mut self, added: Vec<TurnAction>) -> Self {
    if added.is_empty() {
      return self;
    }

    let mut all = std::mem::take(&mut self.actions);

    all.extend(added);

    let cut = all.len().saturating_sub(MAX_TURN_ACTIONS);
    let dropped: Vec<TurnAction> = all.drain(..cut).collect();

    self.dropped_actions += dropped.len();
    self.dropped_context += dropped.iter().filter(|action| action.kind == "context").count();
    self.actions = all;
    self
  }

  // The picture after one piece of a line: text held until a call shows it was no answer, and a call put on with
  // the text held before it said first; thinking, a result and a failed request's text change nothing here.
  fn after_line(mut self, line: &Line) -> Self {
    match line.kind {
      "text" if line.state != Some("failed") => {
        self.held.push(line.text.clone());
        self
      }
      "tool" => {
        let before = self.dropped_actions + self.actions.len();
        let said: Vec<SaidLine> = std::mem::take(&mut self.held)
          .iter()
          .filter_map(|text| said_line(text, before))
          .collect();

        self.said.extend(said);

        let cut = self.said.len().saturating_sub(MAX_SAID_LINES);

        self.said.drain(..cut);
        self.with_actions(vec![action_of(line)])
      }
      _ => self,
    }
  }

  // The picture after every piece of a line, in the order the line holds them.
  fn after_lines(self, lines: &[Line]) -> Self {
    lines.iter().fold(self, Self::after_line)
  }

  // The picture with one call's result written onto the call it answers; none where nothing changed.
  fn with_result(&self, reference: &str, state: Option<&str>) -> Option<Self> {
    let ended = if state == Some("failed") { "failed" } else { "succeeded" };
    let mut changed = self.clone();
    let mut found = false;

    for action in &mut changed.actions {
      if action.kind == "tool"
        && action.reference.as_deref() == Some(reference)
        && action.state.as_deref() != Some(ended)
      {
        action.state = Some(ended.to_owned());
        found = true;
      }
    }

    found.then_some(changed)
  }

  /// One line of the record or the stream against what is already known; none where it says nothing new.
  pub fn read(&self, line: &str) -> Option<Self> {
    let trimmed = trim(line);

    if !trimmed.starts_with('{') {
      return None;
    }

    let event = Json::parse(trimmed)?;

    // A pane program's own record: what the turn is doing as it goes, and the answer it ended on with its
    // usage (the agency's `docs/run-records.md`, 「A pane program's record」).
    if event.get("type").is_none()
      && let Some(kind) = event.get("kind").and_then(Json::as_str)
    {
      match kind {
        "progress" => return self.read_progress(&event),
        "answer" => return self.read_answer(&event),
        _ => {}
      }
    }

    if matches!(event.get("type").and_then(Json::as_str), Some("response_item") | Some("event_msg")) {
      return self.read_rollout(&event);
    }

    if event.get("type").and_then(Json::as_str) == Some("system")
      && event.get("subtype").and_then(Json::as_str) == Some("thinking_tokens")
    {
      let added = event
        .get("estimated_tokens_delta")
        .filter(|delta| !matches!(delta, Json::Null))
        .or_else(|| event.get("estimated_tokens"))
        .and_then(number)?;

      return Some(Self {
        phase: "thinking",
        tool: None,
        thinking_tokens: Some(self.thinking_tokens.unwrap_or(0) + added),
        ..self.clone()
      });
    }

    if event.get("type").and_then(Json::as_str) == Some("user") {
      let answered = record_lines(&event)
        .into_iter()
        .find(|line| line.kind == "tool-result" && line.reference.is_some())?;

      return self.with_result(answered.reference.as_deref().unwrap_or_default(), answered.state);
    }

    if event.get("type").and_then(Json::as_str) != Some("assistant") {
      return None;
    }

    let message = event.get("message");
    let blocks = message
      .and_then(|message| message.get("content"))
      .and_then(Json::as_array)
      .unwrap_or_default();
    let block = blocks
      .iter()
      .find(|one| one.get("type").and_then(Json::as_str) == Some("tool_use"))
      .or_else(|| blocks.first());
    let block_type = block.and_then(|block| block.get("type")).and_then(Json::as_str);
    let usage = message.and_then(|message| message.get("usage"));
    let id = message.and_then(|message| message.get("id")).and_then(Json::as_str).map(str::to_owned);
    let used = |name: &str| usage.and_then(|usage| usage.get(name)).and_then(number).unwrap_or(0);
    let context = match usage {
      None => self.context_tokens,
      Some(_) => {
        Some(used("input_tokens") + used("cache_read_input_tokens") + used("cache_creation_input_tokens"))
      }
    };
    let fresh = usage.is_some() && (id.is_none() || id != self.request_id);
    let written: i64 = blocks.iter().map(written_of).sum();
    let tool = match (block_type, block.and_then(|block| block.get("name")).and_then(Json::as_str)) {
      (Some("tool_use"), Some(name)) => {
        let input = block.and_then(|block| block.get("input")).cloned().unwrap_or(Json::Object(vec![]));

        Some(called_tool(name, &input).name)
      }
      _ => None,
    };
    let lines = record_lines(&event);
    let spent = if fresh {
      let held = self.spent.clone().unwrap_or_default();

      Some(RequestSpend {
        input_tokens: held.input_tokens + used("input_tokens"),
        cache_read_tokens: held.cache_read_tokens + used("cache_read_input_tokens"),
        cache_write_tokens: held.cache_write_tokens + used("cache_creation_input_tokens"),
      })
    } else {
      self.spent.clone()
    };
    let read = Self {
      phase: phase_of(block_type, self.phase),
      tool,
      context_tokens: context,
      cache_read_tokens: if usage.is_some() {
        Some(used("cache_read_input_tokens"))
      } else {
        self.cache_read_tokens
      },
      request_id: id.or_else(|| self.request_id.clone()),
      spent,
      written_tokens: added(self.written_tokens, written),
      ..self.clone()
    };

    Some(read.after_lines(&lines))
  }

  // One line of a Codex rollout against what is already known.
  fn read_rollout(&self, record: &Json) -> Option<Self> {
    let payload = record.get("payload")?;

    if payload.get("type").and_then(Json::as_str) == Some("token_count") {
      return self.counted(payload.get("info")?.get("last_token_usage")?);
    }

    let item = rollout_item(record)?;
    let lines = rollout_lines(record);

    match item.get("type").and_then(Json::as_str)? {
      "reasoning" => Some(Self {
        phase: "thinking",
        tool: None,
        ..self.clone()
      }),
      "message" if item.get("role").and_then(Json::as_str) == Some("assistant") => {
        let read = Self {
          phase: "writing",
          tool: None,
          written_tokens: added(self.written_tokens, lines.iter().map(|line| estimated(&line.text)).sum()),
          ..self.clone()
        };

        Some(read.after_lines(&lines))
      }
      "function_call" | "custom_tool_call" => {
        let name = item.get("name").and_then(Json::as_str).unwrap_or("tool");
        let read = Self {
          phase: "tool",
          tool: Some(called_tool(name, &rollout_input(item)).name),
          written_tokens: added(self.written_tokens, lines.iter().map(|line| estimated(&line.text)).sum()),
          ..self.clone()
        };

        Some(read.after_lines(&lines))
      }
      "function_call_output" | "custom_tool_call_output" => {
        let answered = lines.first()?;

        self.with_result(answered.reference.as_deref()?, answered.state)
      }
      _ => None,
    }
  }

  // One progress record of a pane program's turn: the phase it stands in, the tool it calls where it calls
  // one, how full the session stands and what the turn has spent so far, each where the program reported it.
  fn read_progress(&self, event: &Json) -> Option<Self> {
    let phase = match event.get("phase").and_then(Json::as_str) {
      Some("thinking") => "thinking",
      Some("writing") => "writing",
      Some("tool") => "tool",
      _ => self.phase,
    };
    let tool = match event.get("tool") {
      Some(Json::String(name)) => Some(name.clone()),
      Some(_) => None,
      None => self.tool.clone(),
    };
    let context = event
      .get("context")
      .and_then(|context| context.get("used"))
      .and_then(number)
      .or(self.context_tokens);
    // The figure the turn stands at is its whole spend so far, so it is set where it stands, never added.
    let usage = event.get("usage").filter(|usage| !matches!(usage, Json::Null));
    let used = |name: &str| usage.and_then(|usage| usage.get(name)).and_then(number);
    let spent = used("inputTokens").map(|input| RequestSpend {
      input_tokens: input,
      cache_read_tokens: used("cacheReadTokens").unwrap_or(0),
      cache_write_tokens: used("cacheWriteTokens").unwrap_or(0),
    });

    Some(Self {
      phase,
      tool,
      context_tokens: context,
      spent: spent.or(self.spent.clone()),
      written_tokens: used("outputTokens").or(self.written_tokens),
      ..self.clone()
    })
  }

  // The answer a pane program's turn ended on, with the usage it reported: the turn's whole spend and the
  // context its last request carried, where the program gave them.
  fn read_answer(&self, event: &Json) -> Option<Self> {
    let Some(usage) = event.get("usage").filter(|usage| !matches!(usage, Json::Null)) else {
      return Some(self.clone());
    };
    let used = |name: &str| usage.get(name).and_then(number).unwrap_or(0);

    Some(Self {
      spent: Some(RequestSpend {
        input_tokens: used("inputTokens"),
        cache_read_tokens: used("cacheReadTokens"),
        cache_write_tokens: used("cacheWriteTokens"),
      }),
      written_tokens: Some(used("outputTokens")),
      ..self.clone()
    })
  }

  // What one request of a rollout reported.
  fn counted(&self, usage: &Json) -> Option<Self> {
    let used = |name: &str| usage.get(name).and_then(number).unwrap_or(0);
    let input = used("input_tokens");

    if input == 0 {
      return None;
    }

    let read = used("cached_input_tokens");
    let write = used("cache_write_input_tokens");
    let held = self.spent.clone().unwrap_or_default();

    Some(Self {
      thinking_tokens: added(self.thinking_tokens, used("reasoning_output_tokens")),
      context_tokens: Some(input),
      cache_read_tokens: Some(read),
      spent: Some(RequestSpend {
        input_tokens: held.input_tokens + (input - read - write).max(0),
        cache_read_tokens: held.cache_read_tokens + read,
        cache_write_tokens: held.cache_write_tokens + write,
      }),
      ..self.clone()
    })
  }

  /// What of the picture the room's screens draw, as one word: a change in it is worth an event at once.
  pub fn drawn(&self) -> String {
    let mut parts = vec![
      self.phase.to_owned(),
      self.tool.clone().unwrap_or_default(),
      self.dropped_actions.to_string(),
    ];

    parts.extend(self.actions.iter().map(|action| {
      format!(
        "{}:{}:{}",
        action.kind,
        action.name.as_deref().unwrap_or_default(),
        action.state.as_deref().unwrap_or_default()
      )
    }));
    parts.extend(self.said.iter().map(|line| format!("{}:{}", line.before, line.text)));

    parts.join("|")
  }
}

fn number(value: &Json) -> Option<i64> {
  match value {
    Json::Number(number) => number.as_i64().or_else(|| number.as_f64().map(|number| number as i64)),
    _ => None,
  }
}

// The tokens one block wrote, by its length; thinking is the program's own count.
fn written_of(block: &Json) -> i64 {
  let text = match block.get("type").and_then(Json::as_str) {
    Some("text") => block.get("text").and_then(Json::as_str).unwrap_or_default().to_owned(),
    Some("tool_use") => block.get("input").map_or_else(|| "{}".to_owned(), Json::stringify),
    _ => String::new(),
  };

  estimated(&text)
}

// The tokens a text makes, by estimate.
fn estimated(text: &str) -> i64 {
  text.len().div_ceil(BYTES_PER_TOKEN) as i64
}

// A count with more added to it, left as it was where nothing was.
fn added(held: Option<i64>, more: i64) -> Option<i64> {
  if more == 0 { held } else { Some(held.unwrap_or(0) + more) }
}

// A call among a line's pieces, as the action the picture keeps of it.
fn action_of(line: &Line) -> TurnAction {
  TurnAction {
    kind: "tool",
    name: line.name.clone(),
    text: line.text.clone(),
    state: None,
    reference: line.reference.clone(),
  }
}

// Text the turn wrote before a call, as the progress line the room is shown: its secret-shaped runs taken out,
// its whitespace run together into one line and cut at the bound; none where nothing is left.
fn said_line(text: &str, before: usize) -> Option<SaidLine> {
  let redacted = secrets::redacted(text);
  let flat = redacted.split(is_space).filter(|word| !word.is_empty()).collect::<Vec<_>>().join(" ");

  (!flat.is_empty()).then(|| SaidLine {
    text: clip(&flat, SAID_CHARS),
    before,
  })
}

fn phase_of(block: Option<&str>, held: &'static str) -> &'static str {
  match block {
    Some("thinking") => "thinking",
    Some("text") => "writing",
    Some("tool_use") => "tool",
    _ => held,
  }
}

/// The picture as the room's screens are told it.
pub fn picture_of(progress: &RunProgress) -> Value {
  serde_json::to_value(progress).unwrap_or(json!({}))
}

//! What a turn of a conversational agent is doing, read off the lines its run leaves in the agency's log as they
//! are written (the agency's `docs/run-records.md`, 「Agent log」; docs/receiver.md, 「The event stream」): whether it thinks, writes or calls a tool, the thinking it
//! counted, what its last request carried and what its requests spent, an estimate of what it wrote, the last
//! few things it did — what it was handed and each call with how it ended — and the last few lines it wrote and
//! then went on working, so a room watching the console sees the turn as it goes.

use receiver_policy::chat_round::CHAT_ROUND;
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
  /// The last thinking block the run wrote, on a turn working in the owner's private chat; none on every other.
  pub thinking: Option<String>,
  // What the turn wrote since its last call, which a call coming after it makes progress lines of and which is
  // otherwise its answer.
  #[serde(skip)]
  held: Vec<String>,
  // The block the turn is still writing, which no line says yet: whether it is thinking, and the bytes it has
  // written so far.
  #[serde(skip)]
  open: Option<(bool, u64)>,
  // What the turn's requests measured they wrote, each counted once at the largest figure its reports gave,
  // and that largest figure of the request counted last.
  #[serde(skip)]
  measured: i64,
  #[serde(skip)]
  measured_last: i64,
  // The thinking and written figures the last look drew, below which no later look draws them.
  #[serde(skip)]
  reached: Reached,
}

// The figures a look drew, each none where it drew none.
#[derive(Clone, Debug, Default, PartialEq)]
struct Reached {
  thinking: Option<i64>,
  written: Option<i64>,
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
      thinking: None,
      held: vec![],
      open: None,
      measured: 0,
      measured_last: 0,
      reached: Reached::default(),
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

  /// The picture carrying `block` as the run's last thinking block: as it stands with its secret-shaped runs taken
  /// out, cut at the policy's ceiling with the cut marked (docs/messages.md, 「A working turn's thinking」).
  pub fn thought(mut self, block: &str) -> Self {
    self.thinking = Some(clip(&secrets::redacted(block), CHAT_ROUND.thinking.ceiling_chars));

    self
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

  /// One line the agency filed already told apart — a thinking block, words, a call or a call's result — against
  /// what is already known; none where it is no such line. Such a line names what it is, so the phase and the
  /// call are read off the line itself and what the turn thought and wrote is counted by estimate; the request
  /// its metadata names is counted exactly (docs/receiver.md, 「The event stream」).
  pub fn filed(&self, line: &serde_json::Map<String, Value>) -> Option<Self> {
    let request = line.get("metadata").and_then(|metadata| metadata.get("request"));

    Some(self.filed_kind(line)?.requested(self, request))
  }

  // The request a filed line's message answers, counted into the picture: how full the session stands by that
  // request's input, and what the turn spent with the request added once, by its id.
  fn requested(self, before: &Self, request: Option<&Value>) -> Self {
    let Some(usage) = request.and_then(|request| request.get("usage")).filter(|usage| usage.is_object())
    else {
      return self;
    };
    let used = |name: &str| usage.get(name).and_then(Value::as_i64).unwrap_or(0);
    let id = request.and_then(|request| request.get("id")).and_then(Value::as_str).map(str::to_owned);
    let fresh = id.is_none() || id != before.request_id;
    let spent = if fresh {
      let held = before.spent.clone().unwrap_or_default();

      Some(RequestSpend {
        input_tokens: held.input_tokens + used("input_tokens"),
        cache_read_tokens: held.cache_read_tokens + used("cache_read_input_tokens"),
        cache_write_tokens: held.cache_write_tokens + used("cache_creation_input_tokens"),
      })
    } else {
      before.spent.clone()
    };

    Self {
      context_tokens: Some(
        used("input_tokens") + used("cache_read_input_tokens") + used("cache_creation_input_tokens"),
      ),
      cache_read_tokens: Some(used("cache_read_input_tokens")),
      request_id: id.or_else(|| before.request_id.clone()),
      spent,
      ..self
    }
    .with_request_output(fresh, used("output_tokens"))
  }

  // The picture with one request's measured output counted: a fresh request adds what it reported, a request
  // already counted moves to the larger of its reports, and the turn's figures then reach at least what its
  // requests measured, the written figure making up what the thinking counted does not.
  fn with_request_output(self, fresh: bool, output: i64) -> Self {
    let (earlier, last) =
      if fresh { (0, output) } else { (self.measured_last, self.measured_last.max(output)) };
    let measured = self.measured - earlier + last;

    Self {
      written_tokens: reaching(self.written_tokens, self.thinking_tokens, measured),
      measured,
      measured_last: last,
      ..self
    }
  }

  fn filed_kind(&self, line: &serde_json::Map<String, Value>) -> Option<Self> {
    let word = |name: &str| line.get(name).and_then(Value::as_str);
    let text = word("text").unwrap_or_default();
    let failed = word("state") == Some("failed");
    let reference = word("ref").or_else(|| word("reference"));

    match word("kind")? {
      "thinking" if !failed => Some(Self {
        phase: "thinking",
        tool: None,
        thinking_tokens: added(self.thinking_tokens, estimated(text)),
        ..self.clone()
      }),
      "text" if !failed => {
        let read = Self {
          phase: "writing",
          tool: None,
          written_tokens: added(self.written_tokens, estimated(text)),
          ..self.clone()
        };

        Some(read.after_line(&Line {
          kind: "text",
          text: text.to_owned(),
          ..Line::default()
        }))
      }
      "tool" => {
        let name = word("name").map(str::to_owned);
        let read = Self {
          phase: "tool",
          tool: name.clone(),
          written_tokens: added(self.written_tokens, estimated(text)),
          ..self.clone()
        };

        Some(read.after_line(&Line {
          kind: "tool",
          text: text.to_owned(),
          name,
          reference: reference.map(str::to_owned),
          ..Line::default()
        }))
      }
      "tool-result" => self.with_result(reference?, word("state")),
      _ => None,
    }
  }

  /// The block the turn is still writing, of the kind named and with the bytes written so far, or none open: it
  /// stands beside what the lines have said until a line says it (docs/receiver.md, 「A turn's progress」).
  /// The look draws the figures it reaches, and no later look of the turn draws them lower: a block that ended
  /// keeps what it drew until its line, which may say less than the bytes it streamed, has come.
  pub fn with_open(&self, open: Option<(&str, u64)>) -> Self {
    let looked = Self {
      open: open.map(|(kind, bytes)| (kind == "thinking", bytes)),
      ..self.clone()
    };
    let drawn = looked.counted_open();

    Self {
      reached: Reached {
        thinking: drawn.thinking_tokens,
        written: drawn.written_tokens,
      },
      ..looked
    }
  }

  // The picture with the open block's bytes counted by estimate, under thinking for a thinking block and under
  // the written figure for words and calls, each figure at least where the last look drew it.
  fn counted_open(&self) -> Self {
    let more = |open: bool| match self.open {
      Some((thinking, bytes)) if thinking == open => (bytes as usize).div_ceil(BYTES_PER_TOKEN) as i64,
      _ => 0,
    };

    Self {
      thinking_tokens: added(self.thinking_tokens, more(true)).max(self.reached.thinking),
      written_tokens: added(self.written_tokens, more(false)).max(self.reached.written),
      ..self.clone()
    }
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
      // The event names either one stretch's count, added to what was counted, or the running count, which
      // stands the figure at the greater of the two so it never moves back while the turn runs.
      let delta = event.get("estimated_tokens_delta").and_then(number);
      let counted = match (delta, event.get("estimated_tokens").and_then(number)) {
        (Some(more), _) => added(self.thinking_tokens, more),
        (None, Some(whole)) => standing(self.thinking_tokens, whole),
        _ => return None,
      };

      return Some(Self {
        phase: "thinking",
        tool: None,
        thinking_tokens: counted,
        ..self.clone()
      });
    }

    if event.get("type").and_then(Json::as_str) == Some("user") {
      // Words handed over open the program's next turn, so what the turn before held was its answer, never a line.
      if handed_words(&event) {
        return (!self.held.is_empty()).then(|| Self {
          held: vec![],
          ..self.clone()
        });
      }

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
    let read = match usage {
      Some(_) => read.with_request_output(fresh, used("output_tokens")),
      None => read,
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
    // The record's own figures are the turn's spend so far, so the spend and how full the session stands
    // are set where they stand, while the output it states is the whole the turn's figures reach: the
    // estimates the run's filed lines add stay where they are above it, so the figures never go back.
    let usage = event.get("usage").filter(|usage| !matches!(usage, Json::Null));
    let used = |name: &str| usage.and_then(|usage| usage.get(name)).and_then(number);
    let spent = used("inputTokens").map(|input| RequestSpend {
      input_tokens: input,
      cache_read_tokens: used("cacheReadTokens").unwrap_or(0),
      cache_write_tokens: used("cacheWriteTokens").unwrap_or(0),
    });
    let written = reaching(self.written_tokens, self.thinking_tokens, used("outputTokens").unwrap_or(0));

    Some(Self {
      phase,
      tool,
      context_tokens: context,
      spent: spent.or(self.spent.clone()),
      written_tokens: written,
      ..self.clone()
    })
  }

  // The answer a pane program's turn ended on, with the usage it reported: the turn's whole spend and the
  // context its last request carried, where the program gave them. Its measured output replaces the running
  // figure and what the looks drew, so the turn ends on the program's own count.
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
      reached: Reached::default(),
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

    Some(
      Self {
        thinking_tokens: added(self.thinking_tokens, used("reasoning_output_tokens")),
        context_tokens: Some(input),
        cache_read_tokens: Some(read),
        spent: Some(RequestSpend {
          input_tokens: held.input_tokens + (input - read - write).max(0),
          cache_read_tokens: held.cache_read_tokens + read,
          cache_write_tokens: held.cache_write_tokens + write,
        }),
        ..self.clone()
      }
      .with_request_output(true, used("output_tokens")),
    )
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

// A count standing at a whole a report states, never below what it already counted: what was counted beside
// the report by estimate may stand above it, and a running count can be stated lower than it stood.
fn standing(held: Option<i64>, whole: i64) -> Option<i64> {
  if whole <= 0 { held } else { Some(held.unwrap_or(0).max(whole)) }
}

// The written figure the measured output of the turn reaches, beside the thinking counted: thinking and words
// together come to at least that output, the written figure making up what the thinking does not.
fn reaching(written: Option<i64>, thinking: Option<i64>, output: i64) -> Option<i64> {
  standing(written, output - thinking.unwrap_or(0))
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
  serde_json::to_value(progress.counted_open()).unwrap_or(json!({}))
}

// The part of a turn's picture a screen draws: all of it but the counts, the tokens, the request and its spend,
// which move on nearly every look.
#[derive(Debug, PartialEq)]
struct Drawn {
  phase: &'static str,
  tool: Option<String>,
  actions: Vec<TurnAction>,
  dropped_actions: usize,
  dropped_context: usize,
  said: Vec<SaidLine>,
  thinking: Option<String>,
}

impl Drawn {
  fn of(progress: &RunProgress) -> Drawn {
    Drawn {
      phase: progress.phase,
      tool: progress.tool.clone(),
      actions: progress.actions.clone(),
      dropped_actions: progress.dropped_actions,
      dropped_context: progress.dropped_context,
      said: progress.said.clone(),
      thinking: progress.thinking.clone(),
    }
  }
}

/// What a turn's screens were last told of its picture, and when (docs/receiver.md, 「The event stream」): a
/// picture they were told already is never told again, one whose drawn part changed is told at once, and one whose
/// counts alone moved at most once a gap.
#[derive(Debug, Default)]
pub struct Told(Option<(Value, Drawn, i64)>);

impl Told {
  /// The picture of `progress` where the screens are to be told it at `now`, `gap_ms` being the least while
  /// between two pictures whose drawn part is the same; none where they are not. A picture handed back is the one
  /// they were told from then.
  pub fn tell(&mut self, progress: &RunProgress, now: i64, gap_ms: i64) -> Option<Value> {
    let picture = picture_of(progress);
    let drawn = Drawn::of(progress);

    if let Some((last, last_drawn, at)) = &self.0
      && (*last == picture || (*last_drawn == drawn && now - at < gap_ms))
    {
      return None;
    }

    self.0 = Some((picture.clone(), drawn, now));

    Some(picture)
  }
}

/// Whether a record's user event hands words over — a prompt, or words pushed into the turn — rather than carrying a
/// tool's result or the program's own bookkeeping: such words open the program's next turn.
pub fn handed_words(event: &Json) -> bool {
  if event.get("isMeta").and_then(Json::as_bool) == Some(true)
    || event.get("isSidechain").and_then(Json::as_bool) == Some(true)
  {
    return false;
  }

  match event.get("message").and_then(|message| message.get("content")) {
    Some(Json::String(text)) => !trim(text).is_empty(),
    Some(Json::Array(blocks)) => blocks.iter().any(|block| {
      block.get("type").and_then(Json::as_str) == Some("text")
        && block.get("text").and_then(Json::as_str).is_some_and(|text| !trim(text).is_empty())
    }),
    _ => false,
  }
}

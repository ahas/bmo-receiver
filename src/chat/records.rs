//! What a chat turn wrote down (docs/answer-checks.md, 「A promise stands on a record」): a note, a memory, a
//! situation rule, a TODO row added or appended to, a watch. A run's writing verbs keep what they wrote under
//! the run they worked for; a run that kept nothing is read off its logged calls instead — a call of one of the
//! recording tools of BMO's own server, by the record the server's catalogue declares for it, or a shell command
//! that ran one of the recording verbs of the `bmo` binary, read the way the shell would hand its words over.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use crate::js::{self, SPACE};
use receiver_ports::agency::RunLine;
use regex::Regex;
use sea_orm::{DatabaseConnection, Value};
use serde::Serialize;
use serde_json::Value as Json;

use super::room::server_facts;
use crate::receiver::Receiver;
use crate::room_record::{As, RoomHolding, Rows};
use crate::{Fault, all, run};

// A run's records go with its room: a chat run's by the room its name opens with, any other run's where its log
// lines are the room's, but a job's, whose run and records stay with the job.
crate::room_holding!(RoomHolding::Goes {
  rows: Rows {
    table: "run_records",
    condition: "run LIKE 'chat:' || CAST($1 AS TEXT) || ':%'",
    room: As::Number,
    reads: &[],
  },
  column: None,
  files: None,
  why: "what a room's runs wrote down goes with the runs",
});

// The server the recording tools are served under, which a logged call of one names it by.
const SERVER: &str = "bmo";

/// One thing a turn wrote down: its kind, what it said, the row it landed in where its answer named one, and
/// whether it set something that wakes the agent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TurnRecord {
  pub kind: String,
  pub text: String,
  pub id: Option<i64>,
  pub wakes: bool,
}

// The name a call reached an MCP tool by, on either road's spelling: the server and the tool.
static CLAUDE_MCP_NAME: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^mcp__([^_]+(?:_[^_]+)*)__(.+)$").expect("an MCP name pattern"));
static CODEX_MCP_NAME: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"^([A-Za-z0-9][A-Za-z0-9_.-]*)/([A-Za-z0-9_.-]+)$").expect("an MCP name pattern")
});

pub fn mcp_call_of(name: &str) -> Option<(String, String)> {
  CLAUDE_MCP_NAME
    .captures(name)
    .or_else(|| CODEX_MCP_NAME.captures(name))
    .map(|found| (found[1].to_owned(), found[2].to_owned()))
}

/// What a writing verb kept under the run it was working for, for that turn to be asked of afterwards. A verb
/// working for no run — one a person ran in a shell — keeps nothing, since no turn will ever ask for it.
pub async fn keep_run_record<C: sea_orm::ConnectionTrait>(
  db: &C,
  run_name: Option<&str>,
  record: &TurnRecord,
  now: i64,
) -> Result<(), Fault> {
  let Some(run_name) = run_name else { return Ok(()) };

  run(
    db,
    r#"INSERT INTO "run_records" ("run", "kind", "text", "record_id", "wakes", "at") VALUES ($1, $2, $3, $4, $5, $6)"#,
    vec![
      run_name.into(),
      record.kind.as_str().into(),
      record.text.as_str().into(),
      record.id.into(),
      i64::from(record.wakes).into(),
      now.into(),
    ],
  )
    .await
    .map(drop)
}

/// A logged call: the tool it named, the arguments it was handed as the log keeps them, and what it answered
/// and how it ended, none while nothing came back.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct RunCall {
  pub name: Option<String>,
  pub text: String,
  pub result: Option<String>,
  pub state: Option<String>,
}

// The kinds a record is filed under.
const NOTE: &str = "note";
const MEMORY: &str = "memory";
const RULE: &str = "rule";
const TODO_ADD: &str = "todo-add";
const TODO_APPEND: &str = "todo-append";
const WATCH: &str = "watch";

// The memory type a rule is filed under.
const SITUATION: &str = "situation";

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
  let at = args.iter().position(|arg| arg == name)?;

  args.get(at + 1).map(String::as_str)
}

fn has(args: &[String], name: &str) -> bool {
  args.iter().any(|arg| arg == name)
}

// What a recording verb's call filed, by the verb it ran: a memory call that only lists, checks off or takes
// a row down writes nothing new, and neither does a TODO call that reads.
fn verb_kind(verb: &str, args: &[String]) -> Option<&'static str> {
  match verb {
    "note" => Some(NOTE),
    "memory" => has(args, "--add")
      .then(|| if flag(args, "--type").map(js::trim) == Some(SITUATION) { RULE } else { MEMORY }),
    "todo" => {
      if has(args, "--add") {
        Some(TODO_ADD)
      } else if has(args, "--append") {
        Some(TODO_APPEND)
      } else {
        None
      }
    }
    "watch" => Some(WATCH),
    _ => None,
  }
}

fn remembers(kind: &str) -> bool {
  kind == MEMORY || kind == RULE
}

// JavaScript's `parseInt` of a decimal: white space, a sign and the digits that lead.
fn parse_int(text: &str) -> Option<i64> {
  let text = text.trim_start_matches(js::is_space);
  let (sign, digits) = match text.strip_prefix('-') {
    Some(rest) => (-1, rest),
    None => (1, text.strip_prefix('+').unwrap_or(text)),
  };
  let leading: String = digits.chars().take_while(char::is_ascii_digit).collect();

  leading.parse::<i64>().ok().map(|number| sign * number)
}

static ANSWERED: LazyLock<[Regex; 4]> = LazyLock::new(|| {
  [
    r"(?-u:\b)note ([0-9]+) kept(?-u:\b)",
    r"(?-u:\b)memory ([0-9]+) written(?-u:\b)",
    r"(?-u:\b)watch #([0-9]+)",
    r"(?-u:\b)todo: added #([0-9]+)",
  ]
  .map(|pattern| Regex::new(pattern).expect("a script's answer"))
});

// The number the record landed under: the row an append names, and for the rest what the script answered with.
fn id_of(kind: &str, args: &[String], result: Option<&str>) -> Option<i64> {
  let named = (kind == TODO_APPEND).then(|| flag(args, "--append")).flatten();
  let answered = result.and_then(|result| {
    let pattern = match kind {
      NOTE => &ANSWERED[0],
      MEMORY | RULE => &ANSWERED[1],
      WATCH => &ANSWERED[2],
      _ => &ANSWERED[3],
    };

    pattern.captures(result).map(|found| found[1].to_owned())
  });

  parse_int(named.map(str::to_owned).or(answered).as_deref().unwrap_or_default())
}

// A heredoc's body inside a value handed over through a substitution: the first opening whose marker a later
// line closes, the shortest body before it.
fn heredoc_body(text: &str) -> Option<String> {
  let characters = js::chars(text);
  let word = |character: char| character.is_ascii_alphanumeric() || character == '_';
  let opening = |at: usize| characters.get(at) == Some(&'<') && characters.get(at + 1) == Some(&'<');

  for start in (0..characters.len()).filter(|at| opening(*at)) {
    let mut at = start + 2;

    if characters.get(at) == Some(&'-') {
      at += 1;
    }

    while characters.get(at).is_some_and(|character| js::is_space(*character)) {
      at += 1;
    }

    if characters.get(at).is_some_and(|character| *character == '\'' || *character == '"') {
      at += 1;
    }

    let longest = characters[at..].iter().take_while(|character| word(**character)).count();

    for length in (1..=longest).rev() {
      let marker = &characters[at..at + length];
      let Some(line_end) = characters[at + length..].iter().position(|character| *character == '\n') else {
        continue;
      };
      let body_start = at + length + line_end + 1;
      let closes = |newline: usize| {
        let mut from = newline + 1;

        while characters.get(from).is_some_and(|character| js::is_space(*character)) {
          from += 1;
        }

        characters.get(from..from + length) == Some(marker)
          && !characters.get(from + length).is_some_and(|character| word(*character))
      };
      let closing = (body_start..characters.len()).find(|at| characters[*at] == '\n' && closes(*at));

      if let Some(closing) = closing {
        return Some(js::trim(&js::text(&characters[body_start..closing])).to_owned());
      }
    }
  }

  None
}

// What a record says. A value handed over through a heredoc in a substitution, or a note fed on standard
// input, is the heredoc's body.
fn text_of(kind: &str, args: &[String]) -> String {
  let named = match kind {
    TODO_ADD => "--title",
    RULE => "--condition",
    MEMORY => "--action",
    WATCH => "--note",
    _ => "--text",
  };

  flag(args, named)
    .or_else(|| flag(args, "<<"))
    .map(|value| heredoc_body(value).unwrap_or_else(|| js::trim(value).to_owned()))
    .unwrap_or_default()
}

// A verb run as a command: the verbs' binary, then the verb. The binary is the word `bmo` standing alone or
// ending a path, and a checkout's own build puts a `--` between the two.
static BINARY: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(&format!(r#"(?:^|[{SPACE};&|(/"'])bmo["']?[{SPACE}]+"#)).expect("the verbs' binary")
});
static HEREDOC_MARKER: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(&format!(r#"^<<-?[{SPACE}]*['"]?([A-Za-z0-9_]+)['"]?"#)).expect("a heredoc's opening")
});

// The shell command a call ran. The log keeps a call's arguments as the JSON the tool was handed, cut short
// where it ran long, so a line that no longer parses is read for its command by hand.
fn command_of(call: &RunCall) -> Option<String> {
  if call.name.as_deref().is_some_and(|name| name != "Bash") {
    return None;
  }

  match serde_json::from_str::<Json>(&call.text) {
    Ok(Json::Null) | Err(_) => {
      let opening = r#""command":""#;

      Some(match call.text.find(opening) {
        None => call.text.clone(),
        Some(at) => unescape_json(&call.text[at + opening.len()..]),
      })
    }
    Ok(parsed) => parsed.get("command").and_then(Json::as_str).map(str::to_owned),
  }
}

fn unescape_json(text: &str) -> String {
  let characters = js::chars(text);
  let mut out = String::new();
  let mut at = 0;

  while at < characters.len() {
    match (characters[at], characters.get(at + 1)) {
      ('\\', Some(one @ ('"' | '\\' | '/' | 'n' | 'r' | 't'))) => {
        out.push(match one {
          'n' => '\n',
          'r' => '\r',
          't' => '\t',
          other => *other,
        });
        at += 2;
      }
      (character, _) => {
        out.push(character);
        at += 1;
      }
    }
  }

  out
}

// A heredoc's body as the shell hands it over: `text` as it stands with its closing line, `inner` the body
// alone.
struct Body {
  text: String,
  inner: String,
}

// The bodies of the heredocs opened on one line, read from the start of the next: each runs to a line that is
// its marker alone, or to the end where nothing closes it; and where the command goes on.
fn heredoc_bodies(command: &[char], start: usize, markers: &[String]) -> (Vec<Body>, usize) {
  let mut bodies = vec![];
  let mut rest = start;

  for marker in markers {
    let tail = js::text(&command[rest.min(command.len())..]);
    let lines: Vec<&str> = tail.split('\n').collect();
    let end = lines.iter().position(|line| js::trim(line) == marker);
    let body = match end {
      Some(end) => &lines[..=end],
      None => &lines[..],
    };
    let taken = body.iter().map(|line| line.chars().count()).sum::<usize>()
      + body.len().saturating_sub(1)
      + usize::from(end.is_some());
    let text = js::text(&command[rest.min(command.len())..(rest + taken).min(command.len())]);
    let inner = match end {
      Some(end) => lines[..end].join("\n"),
      None => text.clone(),
    };

    bodies.push(Body { text, inner });
    rest += taken;
  }

  (bodies, rest)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Open {
  Single,
  Double,
  Substitution,
}

// The words of one command from where its script was named, the way the shell splits them with the quotes
// taken off, up to the first `;`, `&&`, `|` or line end nothing holds open. A substitution stands as its own
// text, and a heredoc — fed to the command itself or inside a substitution — is read to its closing line.
fn args_from(command: &[char], from: usize) -> Vec<String> {
  let mut found: Vec<String> = vec![];
  let mut open: Vec<Open> = vec![];
  let mut heredocs: Vec<String> = vec![];
  let mut word: Option<String> = None;
  let mut at = from;
  let starts = |at: usize, with: &str| {
    let with: Vec<char> = with.chars().collect();

    command.get(at..at + with.len()) == Some(&with[..])
  };
  let marker_at = |at: usize| {
    HEREDOC_MARKER
      .captures(&js::text(&command[at..]))
      .map(|found| (found[1].to_owned(), found[0].chars().count(), found[0].to_owned()))
  };
  let add = |word: &mut Option<String>, text: &str| word.get_or_insert_with(String::new).push_str(text);

  while at < command.len() {
    let character = command[at];
    let top = open.last().copied();
    let quoted = matches!(top, Some(Open::Single | Open::Double));

    if character == '\n' && !heredocs.is_empty() && !quoted {
      if open.is_empty()
        && let Some(one) = word.take()
      {
        found.push(one);
      }

      let (bodies, end) = heredoc_bodies(command, at + 1, &std::mem::take(&mut heredocs));

      for body in bodies {
        if open.is_empty() {
          found.push(body.inner);
        } else {
          add(&mut word, &format!("\n{}", body.text));
        }
      }

      at = end;

      if open.is_empty() {
        break;
      }

      continue;
    }

    match top {
      Some(Open::Single) => {
        if character == '\'' {
          open.pop();
        } else {
          add(&mut word, &character.to_string());
        }
      }
      Some(Open::Double) => {
        if character == '"' {
          open.pop();
        } else if character == '\\' && at + 1 < command.len() {
          at += 1;
          add(&mut word, &command[at].to_string());
        } else if starts(at, "$(") {
          open.push(Open::Substitution);
          add(&mut word, "$(");
          at += 1;
        } else {
          add(&mut word, &character.to_string());
        }
      }
      Some(Open::Substitution) => {
        if character == ')' {
          open.pop();
          add(&mut word, ")");
        } else if character == '\'' || character == '"' {
          open.push(if character == '\'' { Open::Single } else { Open::Double });
          add(&mut word, &character.to_string());
        } else if starts(at, "$(") {
          open.push(Open::Substitution);
          add(&mut word, "$(");
          at += 1;
        } else if starts(at, "<<") {
          if let Some((marker, length, whole)) = marker_at(at) {
            heredocs.push(marker);
            add(&mut word, &whole);
            at += length;
            continue;
          }

          add(&mut word, &character.to_string());
        } else {
          add(&mut word, &character.to_string());
        }
      }
      None => {
        if js::is_space(character) && character != '\n' {
          if let Some(one) = word.take() {
            found.push(one);
          }
        } else if character == ';' || character == '|' || character == '\n' || starts(at, "&&") {
          break;
        } else if character == '\'' || character == '"' {
          open.push(if character == '\'' { Open::Single } else { Open::Double });
          word.get_or_insert_with(String::new);
        } else if starts(at, "$(") {
          open.push(Open::Substitution);
          add(&mut word, "$(");
          at += 1;
        } else if starts(at, "<<") {
          if let Some((marker, length, _)) = marker_at(at) {
            heredocs.push(marker);

            if let Some(one) = word.take() {
              found.push(one);
            }

            found.push("<<".to_owned());
            at += length;
            continue;
          }

          add(&mut word, &character.to_string());
        } else if character == '\\' && at + 1 < command.len() {
          if command[at + 1] != '\n' {
            add(&mut word, &command[at + 1].to_string());
          }

          at += 1;
        } else {
          add(&mut word, &character.to_string());
        }
      }
    }

    at += 1;
  }

  if let Some(one) = word {
    found.push(one);
  }

  found
}

// What a call of one of the server's tools recorded: the kind the catalogue declares for what it named, the
// words it wrote, and the number its answer names.
fn tool_call_record(tool: &str, call: &RunCall, declared: &HashMap<String, String>) -> Option<TurnRecord> {
  let args = match serde_json::from_str::<Json>(&call.text) {
    Ok(Json::Object(args)) => args,
    _ => serde_json::Map::new(),
  };
  let said = |name: &str| {
    args
      .get(name)
      .and_then(Json::as_str)
      .map(|value| js::trim(value).to_owned())
      .unwrap_or_default()
  };
  let kind = match declared.get(tool).map(String::as_str)? {
    MEMORY if args.get("type").and_then(Json::as_str) == Some(SITUATION) => RULE,
    MEMORY => MEMORY,
    NOTE => NOTE,
    TODO_ADD => TODO_ADD,
    TODO_APPEND => TODO_APPEND,
    WATCH => WATCH,
    _ => return None,
  };

  if call.result.as_deref().is_some_and(|result| result.starts_with("ERROR")) {
    return None;
  }

  let text = match kind {
    TODO_ADD => said("title"),
    RULE => Some(said("condition"))
      .filter(|text| !text.is_empty())
      .unwrap_or_else(|| said("action")),
    MEMORY => said("action"),
    WATCH => said("note"),
    _ => said("text"),
  };
  let item = said("item");
  let appended = (kind == TODO_APPEND)
    .then(|| parse_int(&item))
    .flatten()
    .filter(|number| number.unsigned_abs() < (1 << 53) && number.to_string() == item);

  Some(TurnRecord {
    kind: kind.to_owned(),
    text,
    id: appended.or_else(|| id_of(kind, &[], call.result.as_deref())),
    wakes: kind == WATCH || (remembers(kind) && (!said("on").is_empty() || !said("until").is_empty())),
  })
}

/// What a call recorded, if it was one of the writing verbs and did not fail: a call of BMO's own server's
/// recording tools, by the records `declared` names for them, or a shell command that ran a recording verb.
pub fn record_of(call: &RunCall, declared: &HashMap<String, String>) -> Option<TurnRecord> {
  if call.state.as_deref() == Some("failed") {
    return None;
  }

  if let Some((server, tool)) = call.name.as_deref().and_then(mcp_call_of) {
    return if server == SERVER { tool_call_record(&tool, call, declared) } else { None };
  }

  let command = command_of(call)?;
  let characters = js::chars(&command);

  for found in BINARY.find_iter(&command) {
    let from = command[..found.end()].chars().count();
    let words = args_from(&characters, from);
    let words = match words.split_first() {
      Some((first, rest)) if first == "--" => rest,
      _ => &words[..],
    };
    let Some((verb, args)) = words.split_first() else {
      continue;
    };
    let Some(kind) = verb_kind(verb, args) else {
      continue;
    };

    return Some(TurnRecord {
      kind: kind.to_owned(),
      text: text_of(kind, args),
      id: id_of(kind, args, call.result.as_deref()),
      wakes: kind == WATCH || (remembers(kind) && (has(args, "--on") || has(args, "--until"))),
    });
  }

  None
}

/// Every record a turn's calls made, in the order it made them.
pub fn turn_records(calls: &[RunCall], declared: &HashMap<String, String>) -> Vec<TurnRecord> {
  calls.iter().filter_map(|call| record_of(call, declared)).collect()
}

fn placeholders(count: usize, after: usize) -> String {
  (1..=count).map(|at| format!("${}", at + after)).collect::<Vec<_>>().join(", ")
}

/// What the named runs wrote down, each run's in the order it wrote them: what its scripts kept, and for a run
/// that kept nothing, what its logged calls were. A run names the log of the agent that ran it, so the agent is
/// not asked of again.
pub async fn run_records(
  receiver: &Receiver,
  _agent: &str,
  runs: &[String],
) -> Result<Vec<TurnRecord>, Fault> {
  if runs.is_empty() {
    return Ok(vec![]);
  }

  let db: &DatabaseConnection = receiver.db();
  let named: Vec<Value> = runs.iter().map(|run| run.clone().into()).collect();
  let kept = all(
    db,
    &format!(
      r#"SELECT "run", "kind", "text", "record_id", "wakes" FROM "run_records" WHERE "run" IN ({}) ORDER BY "id""#,
      placeholders(runs.len(), 0)
    ),
    named.clone(),
  )
  .await?;
  let mut records = vec![];
  let mut keeping = HashSet::new();

  for row in kept {
    let run: String = row.try_get("", "run")?;
    let wakes: i64 = row.try_get("", "wakes")?;

    keeping.insert(run);
    records.push(TurnRecord {
      kind: row.try_get("", "kind")?,
      text: row.try_get("", "text")?,
      id: row.try_get("", "record_id")?,
      wakes: wakes == 1,
    });
  }

  let unkept: Vec<&String> = runs.iter().filter(|run| !keeping.contains(*run)).collect();

  if unkept.is_empty() {
    return Ok(records);
  }

  let kind =
    |line: &RunLine| line.get("kind").and_then(Json::as_str).map(str::to_owned).unwrap_or_default();
  let ref_of = |line: &RunLine| line.get("ref").and_then(Json::as_str).map(str::to_owned);
  let text_of =
    |line: &RunLine| line.get("text").and_then(Json::as_str).map(str::to_owned).unwrap_or_default();
  let mut calls: Vec<RunCall> = vec![];

  for run in unkept {
    let lines = receiver.units().agency.run_log(run, None).await.unwrap_or_default();

    for line in &lines {
      if kind(line) != "tool" {
        continue;
      }

      let answer = ref_of(line).and_then(|want| {
        lines
          .iter()
          .find(|other| kind(other) == "tool-result" && ref_of(other) == Some(want.clone()))
      });

      calls.push(RunCall {
        name: line.get("name").and_then(Json::as_str).map(str::to_owned),
        text: text_of(line).to_owned(),
        result: answer.and_then(|one| one.get("text").and_then(Json::as_str)).map(str::to_owned),
        state: answer.and_then(|one| one.get("state").and_then(Json::as_str)).map(str::to_owned),
      });
    }
  }
  let declared: HashMap<String, String> = server_facts(receiver).await.records.into_iter().collect();

  records.extend(turn_records(&calls, &declared));

  Ok(records)
}

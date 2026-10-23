//! The wait tool's entries (docs/reminders.md, 「The wait tool」): a wait for some seconds or until a moment,
//! answered with the note it was given, and a watch that runs a shell check until it exits 0, answered with the
//! note and what the check wrote. Neither reads the turn or calls a unit: `bmo use` runs either on its own and
//! wakes the agent with the answer once the call ends.

use std::process::Stdio;
use std::time::{Duration, Instant, SystemTime};

use chrono::{DateTime, SecondsFormat, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::process::Command;

use tool_call::{Answer, Asked, Entry, Handed, MadeSchema, input_schema};

/// The catalogue's capability every entry is shown by.
const CATALOGUE: &[&str] = &["room.tools"];

// A wait reads nothing and changes nothing, so it is reached by `call`.
const READS: &[(&str, bool)] = &[("read_only", true)];

// A watch runs a command of the caller's, which may reach anything.
const REACHES_OUT: &[(&str, bool)] = &[("open_world", true)];

/// The seconds between a watch's checks where the call names none.
pub const EVERY: u64 = 60;

/// The most of a check's output a watch answers with: its last characters.
pub const OUTPUT_CHARS: usize = 2000;

// The longest a wait sleeps before it reads the clock again, so a machine that slept under it wakes on time.
const CLOCK_READ: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WaitInput {
  /// How many seconds to wait. Give this or until.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub seconds: Option<u64>,
  /// The moment to wait until, a date and time with its offset from UTC as RFC 3339 writes it, such as 2026-10-24T09:00:00+09:00. Give this or seconds.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub until: Option<String>,
  /// What the answer says once the wait is over: what you are to remember then.
  pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WatchInput {
  /// The shell command run through sh; the watch answers once a run of it exits 0.
  pub check: String,
  /// Seconds between runs of the check, 60 where not given; a run that outlasts it is ended.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub every: Option<u64>,
  /// Seconds after which a check that has not held ends the watch as lapsed; none where not given.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub within: Option<u64>,
  /// What the answer says once the check holds or the watch lapses: what you are to remember then.
  pub note: String,
}

const fn entry(
  name: &'static str,
  description: &'static str,
  input: &'static str,
  schema: MadeSchema,
  hints: &'static [(&'static str, bool)],
) -> Entry {
  Entry {
    namespace: "bmo",
    name,
    title: None,
    description,
    input,
    input_schema: schema,
    output: None,
    capabilities: CATALOGUE,
    hints,
    grant: None,
  }
}

/// Every entry the executable answers, as its manifest declares them.
pub const ENTRIES: &[Entry] = &[
  entry(
    "wait",
    "Wait some seconds, or until a moment, and answer with the note: a reminder, run as bmo use bmo/wait --async so its end wakes you in the room.",
    "wait.json",
    input_schema::<WaitInput>,
    READS,
  ),
  entry(
    "watch",
    "Run a shell check every few seconds until it exits 0, and answer with the note and what it wrote: a watch, run as bmo use bmo/watch --async so its end wakes you in the room.",
    "watch.json",
    input_schema::<WatchInput>,
    REACHES_OUT,
  ),
];

fn read<T: serde::de::DeserializeOwned>(name: &str, arguments: Value) -> Result<T, Answer> {
  serde_json::from_value(arguments)
    .map_err(|why| Answer::failure(format!("invalid arguments: {name}: {why}")))
}

// A moment as the answer names it, in UTC to the second.
fn moment(at: SystemTime) -> String {
  DateTime::<Utc>::from(at).to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// The moment a wait ends: `seconds` from `now`, or the moment `until` names; refused where both or neither is
/// given, or where `until` does not read.
pub fn wait_end(input: &WaitInput, now: SystemTime) -> Result<SystemTime, String> {
  match (input.seconds, input.until.as_deref()) {
    (Some(seconds), None) => now
      .checked_add(Duration::from_secs(seconds))
      .ok_or_else(|| format!("{seconds} seconds is past any clock")),
    (None, Some(until)) => DateTime::parse_from_rfc3339(until)
      .map(|at| SystemTime::from(at.with_timezone(&Utc)))
      .map_err(|why| {
        format!("until is not a date and time with its offset from UTC, as RFC 3339 writes one: {why}")
      }),
    (Some(_), Some(_)) => Err("give seconds or until, not both".to_owned()),
    (None, None) => Err("give seconds or until".to_owned()),
  }
}

/// A wait: the clock read again at most every half minute until the end has come, then the note.
pub async fn wait(input: WaitInput) -> Answer {
  let end = match wait_end(&input, SystemTime::now()) {
    Ok(end) => end,
    Err(why) => return Answer::refusal(why),
  };

  while let Ok(left) = end.duration_since(SystemTime::now()) {
    if left.is_zero() {
      break;
    }

    tokio::time::sleep(left.min(CLOCK_READ)).await;
  }

  Answer::text(format!("{}\nwaited until {}", input.note, moment(end)))
}

// What one run of a check came to: whether it exited 0, and what it wrote.
struct Run {
  held: bool,
  output: String,
}

// The last characters of what a check wrote, on a character's boundary.
fn tail(written: &str) -> String {
  let count = written.chars().count();

  written.chars().skip(count.saturating_sub(OUTPUT_CHARS)).collect()
}

// One run of the check through `sh`, ended where it outlasts `bound`.
async fn run_check(check: &str, bound: Duration) -> Result<Run, String> {
  let child = Command::new("sh")
    .arg("-c")
    .arg(check)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .kill_on_drop(true)
    .spawn()
    .map_err(|why| format!("the check could not be started: {why}"))?;

  match tokio::time::timeout(bound, child.wait_with_output()).await {
    Ok(Ok(output)) => {
      let mut written = String::from_utf8_lossy(&output.stdout).into_owned();

      written.push_str(&String::from_utf8_lossy(&output.stderr));

      Ok(Run {
        held: output.status.success(),
        output: tail(written.trim_end()),
      })
    }
    Ok(Err(why)) => Err(format!("the check could not be read: {why}")),
    Err(_) => Ok(Run {
      held: false,
      output: format!("the run outlasted {} seconds and was ended", bound.as_secs()),
    }),
  }
}

// What a run wrote, as the answer shows it.
fn shown(output: &str) -> String {
  if output.is_empty() { " nothing".to_owned() } else { format!(":\n{output}") }
}

/// A watch: the check run at once and then every `every` seconds, until a run exits 0 or `within` has passed.
pub async fn watch(input: WatchInput) -> Answer {
  if input.check.trim().is_empty() {
    return Answer::refusal("check names no command");
  }

  let every = Duration::from_secs(input.every.unwrap_or(EVERY).max(1));
  let within = input.within.map(Duration::from_secs);
  let started = Instant::now();
  let mut runs: u64 = 0;

  loop {
    runs += 1;

    let run = match run_check(&input.check, every).await {
      Ok(run) => run,
      Err(why) => return Answer::failure(why),
    };

    if run.held {
      return Answer::text(format!(
        "{}\nthe check held on run {runs}; it wrote{}",
        input.note,
        shown(&run.output)
      ));
    }

    let rest = match within {
      Some(within) => match within.checked_sub(started.elapsed()) {
        Some(rest) if !rest.is_zero() => rest,
        _ => {
          return Answer::text(format!(
            "lapsed: {}\nthe check did not hold within {} seconds, over {runs} runs; its last run wrote{}",
            input.note,
            within.as_secs(),
            shown(&run.output)
          ));
        }
      },
      None => every,
    };

    tokio::time::sleep(every.min(rest)).await;
  }
}

/// One call of an entry, by its name, with its arguments.
pub async fn call(name: &str, arguments: Value) -> Answer {
  let asked = async {
    Ok::<Answer, Answer>(match name {
      "wait" => wait(read(name, arguments)?).await,
      "watch" => watch(read(name, arguments)?).await,
      other => Answer::failure(format!("this executable holds no entry {other}")),
    })
  };

  asked.await.unwrap_or_else(|refused| refused)
}

/// One request answered: a call of an entry by its full name, resource requests answered as a tool that serves
/// none, and any other kind refused.
pub async fn answer(handed: Handed) -> Result<Value, String> {
  let answered = match handed.asked {
    Asked::Call {
      entry: asked,
      arguments,
    } => match asked.strip_prefix("bmo/") {
      Some(name) if ENTRIES.iter().any(|entry| entry.name == name) => call(name, arguments).await,
      _ => Answer::failure(format!("this executable holds no entry {asked}")),
    },
    Asked::ResourcesList => return Ok(json!({ "resources": [] })),
    Asked::ResourcesRead { .. } => return Ok(json!({ "error": "this executable serves no resources" })),
    Asked::Other => return Err("this executable answers calls alone".to_owned()),
  };

  serde_json::to_value(answered).map_err(|why| format!("the answer could not be written: {why}"))
}

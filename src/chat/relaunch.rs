//! The deployment briefing (docs/wake.md, 「Quiet instructions」): at the round that finds deployments gone live
//! since the agent's session last briefed the owner, one quiet row for the owner's private chat naming them and
//! asking the agent to tell the owner what went live. A deployment is no session event: nothing here says the
//! session reopened. It is written before that chat's round asks for its riders, so the round carries it.

use std::path::{Path, PathBuf};

use receiver_policy::fill_marks;
use receiver_policy::restart::RESTART;
use serde_json::Value;

use super::conversation::unified_session;
use super::instructions::{Cause, Queued, queue};
use crate::now_ms;
use crate::receiver::Receiver;
use crate::store::settings;

// The folder a unit's deploy root is named by, before the unit's name.
const ROOT_PREFIX: &str = "bmo-";

// The deployment record under a deploy root, and the link to the release that went live beside it.
const RECORD: &str = "deployed.json";
const RELEASE: &str = "deploy";
const VERSION: &str = "VERSION";

// How much of a commit stands for a version where the record and the release name none.
const COMMIT_CHARS: usize = 12;

/// One unit's deployment as its record tells it (docs/boundary-api.md, 「The deployments」).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deployed {
  pub unit: String,
  pub version: String,
  pub message: Option<String>,
  pub commit: Option<String>,
  pub at: i64,
}

// A moment as a record writes it: an RFC 3339 time, or epoch milliseconds.
fn moment(value: &Value) -> Option<i64> {
  value.as_i64().or_else(|| {
    chrono::DateTime::parse_from_rfc3339(value.as_str()?)
      .ok()
      .map(|at| at.timestamp_millis())
  })
}

// A moment as the instruction names it: RFC 3339 in UTC, to the second.
fn moment_text(at: i64) -> String {
  chrono::DateTime::from_timestamp_millis(at)
    .map(|at| at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
    .unwrap_or_else(|| at.to_string())
}

// The first of the fields a record carries words in, on one line.
fn words(record: &Value, fields: &[&str]) -> Option<String> {
  fields.iter().find_map(|field| {
    let said = record[*field].as_str()?.split_whitespace().collect::<Vec<&str>>().join(" ");

    (!said.is_empty()).then_some(said)
  })
}

/// The deployment a unit's deploy root records, or none where it holds no record that reads.
pub fn deployment(root: &Path, unit: &str) -> Option<Deployed> {
  let text = std::fs::read_to_string(root.join(RECORD)).ok()?;
  let record: Value = serde_json::from_str(&text).ok()?;
  let at = moment(&record["at"])?;
  let released = || {
    let read = std::fs::read_to_string(root.join(RELEASE).join(VERSION)).ok()?;
    let version = read.trim().to_owned();

    (!version.is_empty()).then_some(version)
  };
  let committed = || {
    let commit: String = record["commit"].as_str()?.chars().take(COMMIT_CHARS).collect();

    (!commit.is_empty()).then_some(commit)
  };
  let commit = committed();
  let version = words(&record, &["version"])
    .or_else(released)
    .or_else(|| commit.clone())
    .unwrap_or_default();

  Some(Deployed {
    unit: unit.to_owned(),
    version,
    message: words(&record, &["message", "summary", "subject"]),
    commit,
    at,
  })
}

/// Every unit's deployment the deploy roots' folder holds, oldest first; with `only`, those units' alone.
pub fn deployments(roots: &Path, only: Option<&[String]>) -> Vec<Deployed> {
  let Ok(folders) = std::fs::read_dir(roots) else {
    return vec![];
  };
  let mut found: Vec<Deployed> = folders
    .filter_map(Result::ok)
    .filter_map(|folder| {
      let name = folder.file_name().to_string_lossy().into_owned();
      let unit = name.strip_prefix(ROOT_PREFIX)?.to_owned();

      if only.is_some_and(|only| !only.contains(&unit)) {
        return None;
      }

      deployment(&folder.path(), &unit)
    })
    .collect();

  found.sort_by(|one, other| one.at.cmp(&other.at).then_with(|| one.unit.cmp(&other.unit)));
  found
}

/// The instruction asking for the owner's briefing on the deployments since the last, in the restart policy's
/// words.
pub fn instruction(deployed: &[Deployed]) -> String {
  let said = &RESTART.briefing;
  let clauses: Vec<String> = deployed
    .iter()
    .map(|one| {
      let at = moment_text(one.at);
      let marks = [("unit", one.unit.as_str()), ("version", one.version.as_str()), ("at", at.as_str())];

      match (&one.message, &one.commit) {
        (Some(message), Some(commit)) => fill_marks(
          &said.deployed,
          &[marks[0], marks[1], marks[2], ("message", message.as_str()), ("commit", commit.as_str())],
        ),
        (None, Some(commit)) => {
          fill_marks(&said.no_message, &[marks[0], marks[1], marks[2], ("commit", commit.as_str())])
        }
        (Some(message), None) => fill_marks(
          &said.deployed_no_commit,
          &[marks[0], marks[1], marks[2], ("message", message.as_str())],
        ),
        (None, None) => fill_marks(&said.no_message_no_commit, &marks),
      }
    })
    .collect();
  let fact = fill_marks(&said.fact, &[("deployed", &clauses.join(&said.joiner))]);

  crate::receiver::notices::frame(&fact, &said.intent, Some(&said.follow_up), false)
}

/// Where the units' deploy roots stand, which leaves the briefing unwritten where it names none.
pub struct Briefings {
  roots: Option<PathBuf>,
}

impl Briefings {
  pub fn new(roots: Option<PathBuf>) -> Self {
    Self { roots }
  }

  /// At a round's start: where deployments went live since the agent's session last briefed the owner, the
  /// owner's briefing written naming them and the moment moved to now. A session never briefed reads as briefed
  /// the restart policy's fresh while ago, and starts its moment from now.
  pub async fn note(&self, receiver: &Receiver, agent: &str, chat_id: i64) {
    let Some(roots) = &self.roots else {
      return;
    };
    let said = &RESTART.briefing;
    let key = match unified_session(receiver, agent).await {
      true => format!("{agent}_briefed_at"),
      false => format!("{agent}_briefed_at:{chat_id}"),
    };
    let db = receiver.db();
    let now = now_ms();
    let briefed = settings::get(db, &key).await.ok().flatten().and_then(|held| held.parse::<i64>().ok());
    let since = briefed.unwrap_or(now - RESTART.fresh_minutes * 60_000);
    let deployed: Vec<Deployed> =
      deployments(roots, None).into_iter().filter(|one| one.at > since).collect();

    if deployed.is_empty() && briefed.is_some() {
      return;
    }

    if let Err(why) = settings::set(db, &key, &now.to_string()).await {
      eprintln!("{agent}: the moment of the last briefing could not be kept: {}", why.detail);
    }

    if deployed.is_empty() {
      return;
    }

    let owner = receiver.owner_room();
    let queued = Queued {
      kind: Some(&said.kind),
      sort: Some(&said.sort),
      rides_until_ms: Some(now + RESTART.ride_minutes * 60_000),
      cause: Cause::Housekeeping,
      ..Queued::default()
    };

    match queue(db, agent, owner, &instruction(&deployed), &queued).await {
      Ok(id) => println!(
        "chat {chat_id}: {agent}'s briefing on {} deployment(s), instruction {id} for chat {owner}",
        deployed.len()
      ),
      Err(why) => eprintln!("chat {chat_id}: the deployment briefing was not written: {}", why.detail),
    }
  }
}

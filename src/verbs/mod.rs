//! The verbs of the receiver's command line that stand on the receiver's own record: what a person, a job or a
//! session runs from a shell, carried out as a function over the receiver's database and its units, answering the
//! lines it prints, so a test holds it without the binary (docs/messages.md; docs/room-files.md).
//!
//! A verb that says a line of its own does it in a run of its own: the row, the log and the process are opened,
//! written and closed through a seam the binary fills, since the run's rows and its log are the agency's and the
//! receiver holds only the words to be said.

pub mod asked_sends;
pub mod draws;
pub mod messages;
pub mod photo_repair;
pub mod reload;
pub mod room_files;
pub mod room_rules;
pub mod room_switches;
pub mod sends;
pub mod talk;
pub mod turn;

use std::path::PathBuf;

use futures_util::future::BoxFuture;

use crate::Fault;

/// Why a verb did not do what it was asked, in the line it prints before it ends with a failure.
#[derive(Debug, thiserror::Error)]
pub enum VerbError {
  /// The caller asked for something the verb refuses, said in the verb's own sentence.
  #[error("{0}")]
  Refused(String),
  /// The verb did part of what it was asked: the lines it prints of that, and the refusal of the rest.
  #[error("{refusal}")]
  Partly { lines: Vec<String>, refusal: String },
  /// The receiver's database could not be read or written.
  #[error(transparent)]
  Store(#[from] Fault),
}

/// One run of a verb's own, as its launcher opens its row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunAsked {
  pub agent: String,
  pub chat_id: i64,
  /// The unit the run is, which its log lines are grouped under.
  pub unit: String,
  /// The chat that set it off.
  pub started_by: Option<String>,
  /// The agent whose turn set the run going.
  pub asked_by: Option<String>,
}

/// A command of the receiver's own binary, as a verb runs it in a shell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandAsked {
  pub name: String,
  pub args: Vec<String>,
  pub env: Vec<(String, String)>,
}

/// One run as its launcher starts the process.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchAsked {
  pub run_id: Option<i64>,
  pub unit: String,
  pub title: String,
  pub agent: String,
  pub command: CommandAsked,
  pub scratch: Vec<PathBuf>,
  pub utility: bool,
}

/// What a run's launcher was asked to do: open the row, start the process, close the row, and write a line of
/// the run's log under its unit. The rows and the log are the agency's; the binary fills this seam with its own
/// launcher, and with none standing a verb that says a line of its own says it in the receiver itself.
pub trait Launch: Send + Sync {
  fn start(&self, asked: &RunAsked) -> BoxFuture<'_, Option<i64>>;

  fn launch(&self, asked: &LaunchAsked) -> BoxFuture<'_, bool>;

  fn close(&self, run_id: i64, outcome: &str) -> BoxFuture<'_, ()>;

  fn log(&self, unit: &str, kind: &str, text: &str) -> BoxFuture<'_, ()>;

  /// The agent whose turn set the run going, read back out of the row.
  fn asked_by(&self, run_id: i64) -> BoxFuture<'_, Option<String>>;
}

/// No launcher stands: a run of a verb's own is not started and its row kept.
#[derive(Debug, Default)]
pub struct NoLaunch;

impl Launch for NoLaunch {
  fn start(&self, _asked: &RunAsked) -> BoxFuture<'_, Option<i64>> {
    Box::pin(async { None })
  }

  fn launch(&self, _asked: &LaunchAsked) -> BoxFuture<'_, bool> {
    Box::pin(async { false })
  }

  fn close(&self, _run_id: i64, _outcome: &str) -> BoxFuture<'_, ()> {
    Box::pin(async {})
  }

  fn log(&self, _unit: &str, _kind: &str, _text: &str) -> BoxFuture<'_, ()> {
    Box::pin(async {})
  }

  fn asked_by(&self, _run_id: i64) -> BoxFuture<'_, Option<String>> {
    Box::pin(async { None })
  }
}

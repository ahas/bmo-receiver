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
pub mod room_switches;
pub mod sends;
pub mod talk;
pub mod turn;

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

//! The owner's in-room setting commands as the receiver reads them off the words typed in a room (docs/rooms.md):
//! the phrases that say each command, the endings a command may carry without changing it, and the suffix an
//! answer to a job's question may put after an option's number.

use serde::Deserialize;

/// The phrases that say each command; the first of each is its plain form.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Phrases {
  pub delegate_on: Vec<String>,
  pub delegate_off: Vec<String>,
  pub unmute: Vec<String>,
  pub mute: Vec<String>,
  pub notice_rest_on: Vec<String>,
  pub notice_rest_off: Vec<String>,
  pub notice_restart_on: Vec<String>,
  pub notice_restart_off: Vec<String>,
  pub photos_on: Vec<String>,
  pub photos_off: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Commands {
  /// The endings taken off the back of a command's words, one at a time for as long as one is there.
  pub polite_tails: Vec<String>,
  /// What an answer to a job's question may put after an option's number.
  pub option_suffixes: Vec<String>,
  pub phrases: Phrases,
}

crate::part!(pub COMMANDS: Commands = "commands.toml");

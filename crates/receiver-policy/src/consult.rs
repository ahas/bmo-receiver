//! What an agent is shown to see another agent's work (docs/consult.md, 「What an agent is shown」): the
//! rooms' windows and the other agents' work, line by line.

use serde::Deserialize;

/// One agent's window as a line.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Windows {
  pub heading: String,
  pub you: String,
  pub open: String,
  pub hears: String,
  pub closed: String,
  pub keeps_none: String,
  pub call: String,
  pub minutes: String,
  pub seconds: String,
}

/// The other agents' work, and the consult room.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Work {
  pub heading: String,
  pub nothing: String,
  pub item: String,
  pub approved: String,
  pub awaiting: String,
  pub more: String,
  pub room: String,
  pub room_titled: String,
  pub room_bare: String,
  pub settle: String,
  pub closing: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Consult {
  pub shown_items: usize,
  pub room_setting: String,
  pub said: String,
  pub number: String,
  pub room_unset: String,
  pub room_shown: String,
  pub room_chosen: String,
  pub not_answered: String,
  pub not_shared: String,
  pub nothing_shown: String,
  pub windows: Windows,
  pub work: Work,
}

crate::part!(pub CONSULT: Consult = "consult.toml");

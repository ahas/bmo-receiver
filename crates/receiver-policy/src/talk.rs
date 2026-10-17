//! The lines a console talk's turn is composed in (docs/boundary-api.md, 「For bmo-job」, `POST /talks`).

use serde::Deserialize;

/// What an item's state is called in its talk's head.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemStates {
  pub done: String,
  pub merged: String,
  pub stop: String,
  pub stop_pending: String,
  pub pause: String,
  pub pause_pending: String,
  pub held: String,
  pub held_failure: String,
  pub held_pending: String,
  pub waiting: String,
}

/// The talk about one TODO item.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
  pub words: String,
  pub head: Vec<String>,
  pub states: ItemStates,
}

/// The talk about one document.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
  pub words: String,
  pub passage_clip: usize,
  pub head: Vec<String>,
  pub passage: String,
  pub under: String,
  pub translated: String,
  pub passage_end: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Talk {
  pub turns_given: usize,
  pub turn_clip: usize,
  pub earlier: String,
  pub earlier_end: String,
  pub owner: String,
  pub agent: String,
  pub says_now: String,
  pub item: Item,
  pub document: Document,
}

crate::part!(pub TALK: Talk = "talk.toml");

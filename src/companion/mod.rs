//! The pieces shape of a conversational agent's turn (docs/companions.md; docs/agents.md, 「One runner」): a
//! companion as its turns read its row, the words a turn takes, and the room a console made for companions reads.
//! The engine, its guard and its sessions are the agency's: the receiver reads the row the agency lists and hands
//! a round to the same session routes every agent's turns go through.

pub mod pass;
pub mod turn;
pub mod words;

use std::collections::HashSet;

use receiver_policy::companion_runner::COMPANION_RUNNER;
use receiver_policy::fill_marks;
use receiver_policy::providers::PROVIDERS;

use crate::all;
use crate::receiver::{Inbound, Receiver, Room};

/// A companion as its turns read its row: its name and label, its pin, and the window it keeps.
#[derive(Clone, Debug)]
pub struct Companion {
  pub name: String,
  pub label: Option<String>,
  pub pinned_provider: Option<String>,
  pub pinned_model: Option<String>,
  pub window_seconds: Option<i64>,
}

impl Companion {
  /// The agent an agent's row names, where its row takes the pieces shape of turn: the row the agency lists, read
  /// as the agent's own fields.
  pub async fn of(receiver: &Receiver, name: &str) -> Option<Companion> {
    let listed = receiver.units().agency.agents().await.ok();

    let found = listed
      .as_ref()
      .and_then(|listed| listed.agents.iter().find(|one| one.name == name))
      .filter(|found| found.row.get("turn_shape").is_some_and(|shape| shape.as_str() == Some("pieces")))?;

    let text = |key: &str| found.row.get(key).and_then(|held| held.as_str().map(str::to_owned));

    Some(Companion {
      name: found.name.clone(),
      label: found.label.clone(),
      pinned_provider: text("pinnedProvider"),
      pinned_model: text("pinnedModel"),
      window_seconds: found.row.get("windowSeconds").and_then(|held| held.as_i64()),
    })
  }

  /// What it is called in the room.
  pub fn shown(&self) -> &str {
    self.label.as_deref().unwrap_or(&self.name)
  }
}

/// A quiet instruction of a companion's own — a reminder come due, a memory woken by an event — as its turn takes
/// it.
#[derive(Clone, Debug)]
pub struct Quiet {
  pub id: i64,
  pub text: String,
  pub events: Vec<String>,
}

/// What a turn takes: the room's words that woke the agent, and its quiet instructions.
#[derive(Clone, Debug, Default)]
pub struct Waiting {
  pub words: Vec<Inbound>,
  pub quiet: Vec<Quiet>,
}

/// The room a console made for companions reads: the owner's private chat only where it is his, the agent as the
/// number this road gave him there, and nobody's one-to-one.
pub async fn room_of(receiver: &Receiver, chat_id: i64) -> Room {
  Room {
    owner_room: receiver.roads().is_owner_room(chat_id).await,
    identity: receiver.roads().identity_in(chat_id).await,
    private_to: None,
  }
}

/// The provider a companion's turns go out on, as its pin names it, or the sentence that says why there is none:
/// no pin, or a provider the policy holds none of.
pub async fn provider_of(_receiver: &Receiver, companion: &Companion) -> Result<String, String> {
  let refused = &COMPANION_RUNNER.refused;
  let (Some(pinned), Some(_model)) = (&companion.pinned_provider, &companion.pinned_model) else {
    return Err(refused.no_pin.clone());
  };

  if PROVIDERS.provider(pinned).is_none() {
    return Err(fill_marks(&refused.no_provider, &[("provider", pinned)]));
  }

  Ok(pinned.to_owned())
}

/// The numbers the room's other answerers speak under there, as the room's own road gave them.
pub async fn answerer_numbers(receiver: &Receiver, chat_id: i64) -> HashSet<i64> {
  let rows = all(
    receiver.db(),
    r#"SELECT "sender_id" FROM "room_answerers" WHERE "chat_id" = $1"#,
    vec![chat_id.into()],
  )
  .await
  .unwrap_or_default();

  HashSet::from_iter(rows.iter().filter_map(|row| row.try_get::<i64>("", "sender_id").ok()))
}

//! A work key's road the agency moved, returned or found closed, told to the owner (docs/wake.md, 「Quiet
//! instructions」, 「A road's change」): each `road.changed` one instruction for the chat agent's session in the
//! owner's private chat, a newer fact of the same key in place of one still waiting, asking for one sentence in
//! the owner's language, a reopening command set apart exactly as the agency wrote it.

use futures_util::future::BoxFuture;
use receiver_policy::fill_marks;
use receiver_policy::owner_facts::OWNER_FACTS;
use receiver_ports::agency::Event;
use serde::Deserialize;
use serde_json::Value;

use crate::chat::instructions::{Queued, queue};
use crate::chat::restart::kick;
use crate::hearing::Hearer;
use crate::receiver::Receiver;

/// The kind the agency publishes a road's change under.
pub const ROAD_CHANGED: &str = "road.changed";

/// A road as the agency names it: its provider, its model and its login where it has one.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Road {
  pub provider: String,
  pub model: String,
  #[serde(default)]
  pub account: Option<String>,
}

/// Why a provider passed could not answer.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Reason {
  pub provider: String,
  pub reason: String,
}

/// One fact of a work key's road, as the agency publishes it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "fact", rename_all = "kebab-case")]
pub enum RoadFact {
  #[serde(rename_all = "camelCase")]
  RoadMoved {
    key: String,
    #[serde(default)]
    from: Option<Road>,
    to: Road,
    #[serde(default)]
    reasons: Vec<Reason>,
  },
  #[serde(rename_all = "camelCase")]
  RoadReturned {
    key: String,
    #[serde(default)]
    from: Option<Road>,
    to: Road,
  },
  #[serde(rename_all = "camelCase")]
  RoadClosed {
    key: String,
    #[serde(default)]
    reasons: Vec<Reason>,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    reopens_at: Option<i64>,
  },
}

// A road in the policy's words, or the head of its order where the agency names none.
fn road_words(road: Option<&Road>) -> String {
  let said = &OWNER_FACTS.road;

  match road {
    None => said.road_none.clone(),
    Some(road) => match road.account.as_deref().filter(|account| !account.is_empty()) {
      Some(account) => fill_marks(
        &said.road_account,
        &[("provider", &road.provider), ("model", &road.model), ("account", account)],
      ),
      None => fill_marks(&said.road, &[("provider", &road.provider), ("model", &road.model)]),
    },
  }
}

fn reasons_words(reasons: &[Reason]) -> String {
  let said = &OWNER_FACTS.road;

  if reasons.is_empty() {
    return said.reasons_none.clone();
  }

  reasons
    .iter()
    .map(|one| fill_marks(&said.reason, &[("provider", &one.provider), ("reason", &one.reason)]))
    .collect::<Vec<_>>()
    .join(&said.reasons_joint)
}

// A moment as the instruction names it: RFC 3339 in UTC, to the second, from the agency's epoch second.
fn moment_text(at: i64) -> String {
  chrono::DateTime::from_timestamp(at, 0)
    .map(|at| at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
    .unwrap_or_else(|| at.to_string())
}

impl RoadFact {
  /// The work key the fact is of.
  pub fn key(&self) -> &str {
    match self {
      Self::RoadMoved { key, .. } | Self::RoadReturned { key, .. } | Self::RoadClosed { key, .. } => key,
    }
  }

  /// What happened, in the policy's words: a reopening command on a line of its own, as the agency wrote it.
  pub fn fact(&self) -> String {
    let said = &OWNER_FACTS.road;

    match self {
      Self::RoadMoved {
        key,
        from,
        to,
        reasons,
      } => fill_marks(
        &said.moved,
        &[
          ("key", key),
          ("from", &road_words(from.as_ref())),
          ("to", &road_words(Some(to))),
          ("reasons", &reasons_words(reasons)),
        ],
      ),
      Self::RoadReturned { key, from, to } => fill_marks(
        &said.returned,
        &[("key", key), ("from", &road_words(from.as_ref())), ("to", &road_words(Some(to)))],
      ),
      Self::RoadClosed {
        key,
        reasons,
        command,
        reopens_at,
      } => {
        let mut parts =
          vec![fill_marks(&said.closed, &[("key", key), ("reasons", &reasons_words(reasons))])];

        parts.push(match command.as_deref().filter(|command| !command.trim().is_empty()) {
          Some(command) => fill_marks(&said.command, &[("command", command)]),
          None => said.no_command.clone(),
        });

        if let Some(at) = reopens_at {
          parts.push(fill_marks(&said.reopens, &[("at", &moment_text(*at))]));
        }

        parts.join("\n")
      }
    }
  }

  /// The instruction the owner is told by: the fact, what it is for and what follows.
  pub fn instruction(&self) -> String {
    let said = &OWNER_FACTS.road;

    fill_marks(
      &said.frame,
      &[("fact", &self.fact()), ("intent", &said.intent), ("follow_up", &said.follow_up)],
    )
  }
}

/// The roads' facts as the service hears them on the agency's stream.
pub struct RoadChanges {
  receiver: Receiver,
}

impl RoadChanges {
  pub fn new(receiver: Receiver) -> Self {
    RoadChanges { receiver }
  }

  /// One fact queued for the chat agent's session in the owner's private chat, in place of one of the same key
  /// still waiting; its instruction's id.
  pub async fn told(&self, data: &Value) -> Result<i64, String> {
    let fact = serde_json::from_value::<RoadFact>(data.clone()).map_err(|why| why.to_string())?;
    let said = &OWNER_FACTS.road;
    let kind = format!("{}:{}", said.kind, fact.key());
    let queued = Queued {
      kind: Some(&kind),
      sort: Some(&said.sort),
      ..Queued::default()
    };
    let receiver = &self.receiver;

    queue(
      receiver.db(),
      &receiver.deployment().agent,
      receiver.owner_id(),
      &fact.instruction(),
      &queued,
    )
    .await
    .map_err(|why| why.to_string())
  }
}

impl Hearer for RoadChanges {
  fn kinds(&self) -> &'static [&'static str] {
    &[ROAD_CHANGED]
  }

  fn hear<'a>(&'a self, event: &'a Event) -> BoxFuture<'a, ()> {
    Box::pin(async move {
      match self.told(&event.data).await {
        Ok(_) => kick(&self.receiver),
        Err(why) => eprintln!("bmo-receiver: the {} was not told: {why}", event.kind),
      }
    })
  }
}

//! The stand-in answering while Claude Code's road is closed, as the agency tells it (docs/voiced-notices.md, 「Rest
//! and fallback」): `fallback-answering` keeps which stand-in answers and since when, and `fallback-ended` clears
//! it, and each is told to the owner by the chat agent in the owner's own chat as a voiced notice.

use futures_util::future::BoxFuture;
use receiver_policy::chat_line::CHAT_LINE;
use receiver_policy::fill_marks;
use receiver_ports::agency::Event;
use serde::Deserialize;

use super::notices::{Voiced, queue_notice};
use super::restart::kick;
use crate::hearing::Hearer;
use crate::now_ms;
use crate::receiver::Receiver;
use crate::store::{Fault, settings};

/// The event a stand-in's move onto the rooms is published as.
pub const ANSWERING: &str = "fallback-answering";

/// The event Claude Code answering again across the rooms is published as.
pub const ENDED: &str = "fallback-ended";

/// Since when a stand-in has answered, in epoch milliseconds.
pub const SINCE_KEY: &str = "fallback_since";

/// Which stand-in answers.
pub const PROVIDER_KEY: &str = "fallback_provider";

// The kinds the two notices stand under: one line of each waits at a time.
const ANSWERING_KIND: &str = "fallback";
const ENDED_KIND: &str = "fallback-lifted";

/// A stand-in answering: the one that answered before where one stood, and the provider and model answering now.
#[derive(Debug, Deserialize)]
pub struct Answering {
  #[serde(default)]
  pub previous: Option<String>,
  pub provider: String,
  #[serde(default)]
  pub model: String,
}

/// The stand-in's moves as the service hears them on the agency's stream.
pub struct Fallbacks {
  receiver: Receiver,
}

impl Fallbacks {
  pub fn new(receiver: Receiver) -> Self {
    Fallbacks { receiver }
  }
}

impl Hearer for Fallbacks {
  fn kinds(&self) -> &'static [&'static str] {
    &[ANSWERING, ENDED]
  }

  fn hear<'a>(&'a self, event: &'a Event) -> BoxFuture<'a, ()> {
    Box::pin(async move {
      let told = if event.kind == ANSWERING {
        match serde_json::from_value::<Answering>(event.data.clone()) {
          Ok(answering) => answered_by(&self.receiver, &answering).await,
          Err(why) => Err(Fault::new(format!("a stand-in answering that does not read: {why}"))),
        }
      } else {
        ended(&self.receiver).await
      };

      match told {
        Ok(()) => kick(&self.receiver),
        Err(why) => eprintln!("bmo-receiver: the {} was not told: {}", event.kind, why.detail),
      }
    })
  }
}

/// The notice a stand-in's move onto the rooms is told by: taking over from Claude, or the next stand-in taking a
/// run the one before could not answer.
pub fn answering_notice(answering: &Answering) -> Voiced {
  let said = &CHAT_LINE.fallback;
  let now = [("label", answering.provider.as_str()), ("model", answering.model.as_str())];
  let fact = match answering.previous.as_deref().filter(|previous| !previous.is_empty()) {
    Some(previous) => fill_marks(&said.moved, &[[("previous", previous)].as_slice(), &now].concat()),
    None => fill_marks(&said.taken, &now),
  };

  Voiced {
    fact,
    intent: said.intent.clone(),
    follow_up: None,
    optional: false,
    kind: Some(ANSWERING_KIND.to_owned()),
    ttl_ms: Some(said.ttl_minutes * 60_000),
    events: vec![],
    rides_ms: None,
  }
}

/// The stand-in answering kept, since the first move of a stretch, and the owner told which answers now.
pub async fn answered_by(receiver: &Receiver, answering: &Answering) -> Result<(), Fault> {
  let db = receiver.db();

  if settings::get(db, SINCE_KEY).await?.is_none() {
    settings::set(db, SINCE_KEY, &now_ms().to_string()).await?;
  }

  settings::set(db, PROVIDER_KEY, &answering.provider).await?;
  queue_notice(receiver, receiver.owner_id(), &answering_notice(answering))
    .await
    .map(drop)
    .map_err(|why| Fault::new(why.to_string()))
}

/// The stand-in let go and the owner told the chat agent answers on its own model again.
pub async fn ended(receiver: &Receiver) -> Result<(), Fault> {
  let db = receiver.db();
  let said = &CHAT_LINE.fallback;
  let notice = Voiced {
    fact: said.lifted.clone(),
    intent: said.lifted_intent.clone(),
    follow_up: None,
    optional: false,
    kind: Some(ENDED_KIND.to_owned()),
    ttl_ms: Some(said.ttl_minutes * 60_000),
    events: vec![],
    rides_ms: None,
  };

  settings::forget(db, SINCE_KEY).await?;
  settings::forget(db, PROVIDER_KEY).await?;
  queue_notice(receiver, receiver.owner_id(), &notice)
    .await
    .map(drop)
    .map_err(|why| Fault::new(why.to_string()))
}

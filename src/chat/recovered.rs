//! A run come back after a failure, as the agency tells it (docs/voiced-notices.md, 「Rest and fallback」): the
//! agency composes the fact, what it is for, its kind and how long it stays worth saying, and the receiver voices it
//! in the owner's own chat.

use futures_util::future::BoxFuture;
use receiver_ports::agency::Event;
use serde::Deserialize;

use super::notices::{Voiced, queue_notice};
use super::restart::kick;
use crate::hearing::Hearer;
use crate::receiver::Receiver;

/// The event a run answering again while a failure stood is published as.
pub const RECOVERED: &str = "run-recovered";

/// A run come back, as the agency composed its notice.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recovered {
  pub fact: String,
  pub intent: String,
  #[serde(default)]
  pub kind: Option<String>,
  #[serde(default)]
  pub ttl_ms: Option<i64>,
}

impl Recovered {
  /// The notice it is voiced by.
  pub fn notice(self) -> Voiced {
    Voiced {
      fact: self.fact,
      intent: self.intent,
      follow_up: None,
      optional: false,
      kind: self.kind,
      ttl_ms: self.ttl_ms,
      events: vec![],
      rides_ms: None,
    }
  }
}

/// A run come back as the service hears it on the agency's stream.
pub struct Recoveries {
  receiver: Receiver,
}

impl Recoveries {
  pub fn new(receiver: Receiver) -> Self {
    Recoveries { receiver }
  }
}

impl Hearer for Recoveries {
  fn kinds(&self) -> &'static [&'static str] {
    &[RECOVERED]
  }

  fn hear<'a>(&'a self, event: &'a Event) -> BoxFuture<'a, ()> {
    Box::pin(async move {
      let recovered = match serde_json::from_value::<Recovered>(event.data.clone()) {
        Ok(recovered) => recovered,
        Err(why) => {
          eprintln!("bmo-receiver: a run come back that does not read: {why}");
          return;
        }
      };

      match queue_notice(&self.receiver, self.receiver.owner_id(), &recovered.notice()).await {
        Ok(_) => kick(&self.receiver),
        Err(why) => eprintln!("bmo-receiver: a run come back was not told: {why}"),
      }
    })
  }
}

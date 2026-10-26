//! What the agency tells of its job workers (docs/wake.md, 「What the agency's events wake」): a worker's session
//! that answered on a lesser model, voiced to the owner by the chat agent.

use futures_util::future::BoxFuture;
use receiver_policy::fill_marks;
use receiver_policy::owner_facts::OWNER_FACTS;
use receiver_ports::agency::Event;
use serde::Deserialize;
use serde_json::Value;

use crate::chat::notices::{Voiced, queue_notice};
use crate::chat::restart::kick;
use crate::hearing::Hearer;
use crate::receiver::{NotReceived, Receiver};

/// The kind a worker's fall to a lesser model is published under.
pub const DOWNGRADED: &str = "worker.changed";

/// A worker's session that answered on a lesser model: the worker, the model it was launched on, the models it
/// answered on, whether its session was dropped, and the job it ran for.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Downgraded {
  pub worker: String,
  pub launched: String,
  #[serde(default)]
  pub answered_on: Vec<String>,
  #[serde(default)]
  pub dropped: bool,
  #[serde(default)]
  pub job: Option<i64>,
}

impl Downgraded {
  /// The notice the owner is told by; none where it answered on no other model.
  pub fn notice(&self) -> Option<Voiced> {
    let said = &OWNER_FACTS.worker_downgraded;
    let model = self.answered_on.first()?;
    let job = self
      .job
      .map_or_else(String::new, |job| fill_marks(&said.job, &[("job", &job.to_string())]));
    let fact =
      fill_marks(&said.fact, &[("worker", &self.worker), ("model", model), ("launched", &self.launched)]);
    let follow_up = if self.dropped {
      said.dropped.clone()
    } else {
      fill_marks(&said.kept, &[("model", model)])
    };

    Some(Voiced {
      fact: format!("{fact}{job}"),
      intent: said.intent.clone(),
      follow_up: Some(follow_up),
      optional: false,
      kind: Some(format!("{}:{}", said.kind, self.worker)),
      ttl_ms: Some(said.ttl_minutes * 60_000),
      events: vec![],
      rides_ms: None,
    })
  }
}

/// The workers' facts as the service hears them on the agency's stream.
pub struct Workers {
  receiver: Receiver,
}

impl Workers {
  pub fn new(receiver: Receiver) -> Self {
    Workers { receiver }
  }

  async fn downgraded(&self, event: &Value) -> Result<(), String> {
    let downgraded = serde_json::from_value::<Downgraded>(event.clone()).map_err(|why| why.to_string())?;
    let Some(notice) = downgraded.notice() else {
      return Ok(());
    };

    queue_notice(&self.receiver, self.receiver.notices_room(), &notice)
      .await
      .map(drop)
      .map_err(told)
  }
}

fn told(why: NotReceived) -> String {
  why.to_string()
}

impl Hearer for Workers {
  fn kinds(&self) -> &'static [&'static str] {
    &[DOWNGRADED]
  }

  fn hear<'a>(&'a self, event: &'a Event) -> BoxFuture<'a, ()> {
    Box::pin(async move {
      match self.downgraded(&event.data).await {
        Ok(()) => kick(&self.receiver),
        Err(why) => eprintln!("bmo-receiver: the {} was not told: {why}", event.kind),
      }
    })
  }
}

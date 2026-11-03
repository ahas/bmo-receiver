//! What the agency tells of its jobs' runs (docs/wake.md, 「What the agency's events wake」): a job's run
//! that answered on a lesser model, named by its role and its repository with the checkout it ran in beside
//! it, voiced to the owner by the default answerer.

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

/// The kind a job's run falling to a lesser model is published under.
pub const DOWNGRADED: &str = "worker.changed";

/// A job's run that answered on a lesser model: the repository and the checkout of it it ran in, the role
/// it ran as, the model it was launched on, the models it answered on, whether its role's session there was
/// dropped, and the job it ran for.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Downgraded {
  /// The repository the run ran in; none for a checkout no repository holds any longer.
  #[serde(default)]
  pub repository: Option<String>,
  /// The checkout the run ran in, as the agency names it.
  pub worker: String,
  #[serde(default)]
  pub role: Option<String>,
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
    let role = self
      .role
      .as_deref()
      .map(str::trim)
      .filter(|role| !role.is_empty())
      .unwrap_or(&said.role_none);
    let repository = self.repository.as_deref().map(str::trim).filter(|repository| !repository.is_empty());
    let place = match repository {
      Some(repository) => {
        fill_marks(&said.place, &[("repository", repository), ("checkout", &self.worker)])
      }
      None => fill_marks(&said.place_none, &[("checkout", &self.worker)]),
    };
    let fact = fill_marks(
      &said.fact,
      &[("role", role), ("place", &place), ("model", model), ("launched", &self.launched)],
    );
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
      kind: Some(format!("{}:{}:{role}", said.kind, self.worker)),
      events: vec![],
      rides_ms: None,
    })
  }
}

/// The jobs' runs' facts as the service hears them on the agency's stream.
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

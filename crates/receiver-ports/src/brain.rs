//! The brain's port: whether a consolidation is due for an agent, and a session's fold reported to the agent's
//! brain (`docs/boundary-api.md`, 「The brain」; the brain's `docs/boundary-api.md`). Bodies are camelCase, behind
//! the bearer token the configuration holds for it. Every call is offered once: a refused connection ends it.
//! The agent's brain is kept by the agent's name once a lookup found it, and let go where the brain answers a
//! call on it 404, so the next call looks it up again.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};

use crate::http::{Answer, Caller, Patience, Peer};

#[derive(Deserialize)]
struct Brain {
  id: i64,
}

#[derive(Deserialize)]
struct Page {
  items: Vec<Brain>,
}

/// The brain's consolidation state, of which the receiver reads whether one is due.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Consolidation {
  consolidation_due: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Fold<'a> {
  session_key: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Folded {
  cleared_count: i64,
}

/// Every call the receiver makes on the brain, each for an agent's one brain; none where the agent has none, or
/// more than one.
pub trait BrainUnit: Send + Sync {
  /// Whether the agent's brain says a consolidation is due.
  fn due<'a>(&'a self, agent: &'a str) -> BoxFuture<'a, Answer<Option<bool>>>;

  /// A session's fold reported to the agent's brain under its key; the marks it cleared.
  fn fold<'a>(&'a self, agent: &'a str, session_key: &'a str) -> BoxFuture<'a, Answer<Option<i64>>>;
}

/// The brain over HTTP, with the agents' brains it has found.
#[derive(Debug, Clone)]
pub struct HttpBrain {
  caller: Caller,
  brains: Arc<Mutex<HashMap<String, i64>>>,
}

impl HttpBrain {
  /// The brain at the peer's address, each call offered once.
  pub fn new(peer: &Peer) -> Self {
    Self::with_caller(Caller::new(peer).with_patience(Patience {
      every: Duration::ZERO,
      ..Patience::default()
    }))
  }

  pub fn with_caller(caller: Caller) -> Self {
    Self {
      caller,
      brains: Arc::default(),
    }
  }

  fn kept(&self) -> std::sync::MutexGuard<'_, HashMap<String, i64>> {
    self.brains.lock().unwrap_or_else(PoisonError::into_inner)
  }

  // The agent's one brain: the one kept, else the one a lookup finds, kept from then on.
  async fn brain_of(&self, agent: &str) -> Answer<Option<i64>> {
    if let Some(id) = self.kept().get(agent) {
      return Ok(Some(*id));
    }

    let query = [("agentKey", agent.to_owned()), ("limit", "2".to_owned())];
    let page: Page = self.caller.get(&["brains"], &query).await?;
    let [brain] = page.items.as_slice() else {
      return Ok(None);
    };

    self.kept().insert(agent.to_owned(), brain.id);

    Ok(Some(brain.id))
  }

  // A call on the agent's brain's answer: a 404 lets the kept brain go.
  fn settled<T>(&self, agent: &str, answer: Answer<T>) -> Answer<Option<T>> {
    match answer {
      Ok(answer) => Ok(Some(answer)),
      Err(failure) if failure.status() == Some(404) => {
        self.kept().remove(agent);

        Ok(None)
      }
      Err(failure) => Err(failure),
    }
  }
}

impl BrainUnit for HttpBrain {
  fn due<'a>(&'a self, agent: &'a str) -> BoxFuture<'a, Answer<Option<bool>>> {
    Box::pin(async move {
      let Some(brain) = self.brain_of(agent).await? else {
        return Ok(None);
      };
      let brain = brain.to_string();
      let read: Answer<Consolidation> = self.caller.get(&["brains", &brain, "consolidation"], &[]).await;

      Ok(self.settled(agent, read)?.map(|read| read.consolidation_due))
    })
  }

  fn fold<'a>(&'a self, agent: &'a str, session_key: &'a str) -> BoxFuture<'a, Answer<Option<i64>>> {
    Box::pin(async move {
      let Some(brain) = self.brain_of(agent).await? else {
        return Ok(None);
      };
      let brain = brain.to_string();
      let folded: Answer<Folded> =
        self.caller.post(&["brains", &brain, "folds"], &Fold { session_key }).await;

      Ok(self.settled(agent, folded)?.map(|folded| folded.cleared_count))
    })
  }
}

//! The brain's port: the agent's brain found, whether a consolidation is due, and a session's fold reported
//! (`docs/boundary-api.md`, 「The brain」; the brain's `docs/boundary-api.md`). Bodies are camelCase, behind the
//! bearer token the configuration holds for it. Every call is offered once: a refused connection ends it.

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

/// A recall of no words, unrecorded: it hands nothing out, marks nothing and ends no row.
#[derive(Serialize)]
struct Unworded {
  words: &'static str,
  record: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Due {
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

/// Every call the receiver makes on the brain.
pub trait BrainUnit: Send + Sync {
  /// The one brain registered for the agent's key; none where it has none, or more than one.
  fn brain_of<'a>(&'a self, agent: &'a str) -> BoxFuture<'a, Answer<Option<i64>>>;

  /// Whether the brain's unconsolidated episodes have reached its threshold, as a recall answers it.
  fn due(&self, brain: i64) -> BoxFuture<'_, Answer<bool>>;

  /// A session's fold reported under its key; the marks it cleared.
  fn fold<'a>(&'a self, brain: i64, session_key: &'a str) -> BoxFuture<'a, Answer<i64>>;
}

/// The brain over HTTP.
#[derive(Debug, Clone)]
pub struct HttpBrain {
  caller: Caller,
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
    Self { caller }
  }
}

impl BrainUnit for HttpBrain {
  fn brain_of<'a>(&'a self, agent: &'a str) -> BoxFuture<'a, Answer<Option<i64>>> {
    Box::pin(async move {
      let query = [("agentKey", agent.to_owned()), ("limit", "2".to_owned())];
      let page: Page = self.caller.get(&["brains"], &query).await?;

      Ok(match page.items.as_slice() {
        [brain] => Some(brain.id),
        _ => None,
      })
    })
  }

  fn due(&self, brain: i64) -> BoxFuture<'_, Answer<bool>> {
    Box::pin(async move {
      let brain = brain.to_string();
      let asked = Unworded {
        words: "",
        record: false,
      };
      let due: Due = self.caller.post(&["brains", &brain, "recall"], &asked).await?;

      Ok(due.consolidation_due)
    })
  }

  fn fold<'a>(&'a self, brain: i64, session_key: &'a str) -> BoxFuture<'a, Answer<i64>> {
    Box::pin(async move {
      let brain = brain.to_string();
      let folded: Folded = self.caller.post(&["brains", &brain, "folds"], &Fold { session_key }).await?;

      Ok(folded.cleared_count)
    })
  }
}

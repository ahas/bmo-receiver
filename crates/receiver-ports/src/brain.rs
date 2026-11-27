//! The brain's port: whether a consolidation is due for an agent, a session's fold reported to the agent's brain,
//! what the brain holds of a person, whom a set of memories is about and the owner's room rules for a room
//! (`docs/boundary-api.md`, 「The brain」; the brain's `docs/boundary-api.md`). Bodies are camelCase, behind
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

/// A subject as the brain lists it, of which the receiver reads its id, its type and key, and its name: the text
/// of its newest active `name` memory, null where it has none.
#[derive(Deserialize)]
struct Subject {
  id: i64,
  #[serde(rename = "type", default)]
  kind: String,
  #[serde(default)]
  key: String,
  #[serde(default)]
  name: Option<String>,
}

#[derive(Deserialize)]
struct Subjects {
  items: Vec<Subject>,
}

/// A memory as the brain lists it, of which the receiver reads the subjects it is about.
#[derive(Deserialize)]
struct Memory {
  #[serde(default)]
  subjects: Vec<Subject>,
}

#[derive(Deserialize)]
struct Memories {
  items: Vec<Memory>,
}

/// What an agent's brain holds of one person: whether any live memory is about them, and whether one of those is a
/// `name` memory, the form they are called by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Known {
  pub held: bool,
  pub named: bool,
}

/// The subject type a person stands under in the brain, keyed by the sender number the receiver holds.
pub const PERSON: &str = "person";

/// The most memories one read of their subjects names.
pub const SUBJECTS_READ_MOST: usize = 200;

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

  /// What the agent's brain holds of the person keyed `key`; none where the agent has no brain, and none on a
  /// brain that keeps no people.
  fn person<'a>(&'a self, _agent: &'a str, _key: &'a str) -> BoxFuture<'a, Answer<Option<Known>>> {
    Box::pin(async { Ok(None) })
  }

  /// The keys of the people the memories named are about, each once, out of at most `SUBJECTS_READ_MOST` of them;
  /// none where the agent has no brain, and none on a brain that keeps no people.
  fn people_of<'a>(
    &'a self,
    _agent: &'a str,
    _memories: &'a [i64],
  ) -> BoxFuture<'a, Answer<Option<Vec<String>>>> {
    Box::pin(async { Ok(None) })
  }
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

  fn person<'a>(&'a self, agent: &'a str, key: &'a str) -> BoxFuture<'a, Answer<Option<Known>>> {
    Box::pin(async move {
      let Some(brain) = self.brain_of(agent).await? else {
        return Ok(None);
      };
      let brain = brain.to_string();
      let query = [("type", PERSON.to_owned()), ("key", key.to_owned()), ("limit", "1".to_owned())];
      let found: Answer<Subjects> = self.caller.get(&["brains", &brain, "subjects"], &query).await;
      let Some(found) = self.settled(agent, found)? else {
        return Ok(None);
      };
      let Some(subject) = found.items.into_iter().next() else {
        return Ok(Some(Known::default()));
      };
      let query = [
        ("subjectId", subject.id.to_string()),
        ("status", "active".to_owned()),
        ("limit", "1".to_owned()),
      ];
      let held: Answer<Memories> = self.caller.get(&["brains", &brain, "memories"], &query).await;
      let Some(held) = self.settled(agent, held)? else {
        return Ok(None);
      };

      Ok(Some(Known {
        held: !held.items.is_empty(),
        named: subject.name.is_some_and(|name| !name.trim().is_empty()),
      }))
    })
  }

  fn people_of<'a>(
    &'a self,
    agent: &'a str,
    memories: &'a [i64],
  ) -> BoxFuture<'a, Answer<Option<Vec<String>>>> {
    Box::pin(async move {
      let named: Vec<String> = memories.iter().take(SUBJECTS_READ_MOST).map(i64::to_string).collect();

      if named.is_empty() {
        return Ok(Some(vec![]));
      }

      let Some(brain) = self.brain_of(agent).await? else {
        return Ok(None);
      };
      let brain = brain.to_string();
      let query = [("ids", named.join(",")), ("limit", named.len().to_string())];
      let read: Answer<Memories> = self.caller.get(&["brains", &brain, "memories"], &query).await;
      let Some(read) = self.settled(agent, read)? else {
        return Ok(None);
      };
      let mut people: Vec<String> = vec![];

      for subject in read.items.into_iter().flat_map(|memory| memory.subjects) {
        if subject.kind == PERSON && !people.contains(&subject.key) {
          people.push(subject.key);
        }
      }

      Ok(Some(people))
    })
  }
}

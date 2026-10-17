//! The bmo-job port: what bmo-job holds of a room the receiver is about to remove (`docs/boundary-api.md`,
//! 「bmo-job」; bmo-job's `docs/api.md`, "A room's holdings"). Bodies are snake_case.

use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};

use crate::http::{Answer, Caller, Peer};

/// One thing bmo-job holds of a room, as its code `RoomHoldings` names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum RoomHolding {
  /// Open items came from the room.
  RoomOpenItems { count: i64 },
  /// Questions of jobs from the room wait for an answer.
  RoomWaitingQuestions { count: i64 },
}

/// Every call the receiver makes on bmo-job.
pub trait Job: Send + Sync {
  /// What bmo-job holds of a room, each named only where it holds any.
  fn holdings(&self, chat_id: i64) -> BoxFuture<'_, Answer<Vec<RoomHolding>>>;
}

/// bmo-job over HTTP.
#[derive(Debug, Clone)]
pub struct HttpJob {
  caller: Caller,
}

impl HttpJob {
  pub fn new(peer: &Peer) -> Self {
    Self::with_caller(Caller::new(peer))
  }

  pub fn with_caller(caller: Caller) -> Self {
    Self { caller }
  }
}

impl Job for HttpJob {
  fn holdings(&self, chat_id: i64) -> BoxFuture<'_, Answer<Vec<RoomHolding>>> {
    Box::pin(async move { self.caller.get(&["rooms", &chat_id.to_string(), "holdings"], &[]).await })
  }
}

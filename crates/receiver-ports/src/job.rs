//! The bmo-job port: what bmo-job holds of a room the receiver is about to remove (`docs/boundary-api.md`,
//! 「bmo-job」; bmo-job's `docs/api.md`, "A room's holdings"). Bodies are snake_case.

use futures_util::future::BoxFuture;
use reqwest::Method;
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

/// A question of a job that waits for the owner's answer in a room, as the receiver reads the owner's
/// words against it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenQuestion {
  pub id: i64,
  /// The options, in the order they were asked.
  pub options: Vec<String>,
  /// Whether the owner may talk it over with the asker rather than only choose.
  pub talks: bool,
}

/// A question settled with one of its options, by number counted from zero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chosen {
  pub id: i64,
  pub chosen: i64,
}

/// A line of talk over an open question.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Talked {
  pub id: i64,
  pub say: String,
}

/// The queue's standing told as moved, with who moved it where it is named.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Moved {
  #[serde(default)]
  pub detail: Option<String>,
}

#[derive(Deserialize)]
struct Listed {
  items: Vec<ItemRow>,
}

/// An item on the queue that is not finished, merged or set aside, as bmo-job keeps it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemRow {
  pub id: i64,
  /// Whether the owner has approved it.
  pub approved: bool,
  pub status: String,
  pub title: String,
  /// The worker assigned where one is.
  #[serde(default)]
  pub worker: Option<String>,
}

/// Every call the receiver makes on bmo-job.
pub trait Job: Send + Sync {
  /// What bmo-job holds of a room, each named only where it holds any.
  fn holdings(&self, chat_id: i64) -> BoxFuture<'_, Answer<Vec<RoomHolding>>>;

  /// Every item on the queue that is not finished, merged or set aside, oldest first.
  fn items(&self) -> BoxFuture<'_, Answer<Vec<ItemRow>>>;

  /// The questions of jobs that wait for the owner's answer in a room, the one asked last first.
  fn open_questions(&self, chat_id: i64) -> BoxFuture<'_, Answer<Vec<OpenQuestion>>>;

  /// A question settled with one of its options; a question that is gone or settled already is refused.
  fn settle<'a>(&'a self, chosen: &'a Chosen) -> BoxFuture<'a, Answer<()>>;

  /// A line of talk over an open question, its clock stopped for it; refused where the question takes no
  /// talk or is settled already.
  fn talk<'a>(&'a self, talked: &'a Talked) -> BoxFuture<'a, Answer<()>>;

  /// The queue's standing put into the stream as moved.
  fn queue_moved<'a>(&'a self, moved: Option<&'a str>) -> BoxFuture<'a, Answer<()>>;
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

  fn items(&self) -> BoxFuture<'_, Answer<Vec<ItemRow>>> {
    Box::pin(async move {
      let query = vec![("open", "1".to_owned())];
      let listed: Listed = self.caller.get(&["items"], &query).await?;

      Ok(listed.items)
    })
  }

  fn open_questions(&self, chat_id: i64) -> BoxFuture<'_, Answer<Vec<OpenQuestion>>> {
    Box::pin(async move {
      let query = vec![("chat", chat_id.to_string())];

      self.caller.get(&["questions", "open"], &query).await
    })
  }

  fn settle<'a>(&'a self, chosen: &'a Chosen) -> BoxFuture<'a, Answer<()>> {
    Box::pin(self.caller.done(Method::POST, &["question"], chosen))
  }

  fn talk<'a>(&'a self, talked: &'a Talked) -> BoxFuture<'a, Answer<()>> {
    Box::pin(self.caller.done(Method::POST, &["question-talk"], talked))
  }

  fn queue_moved<'a>(&'a self, moved: Option<&'a str>) -> BoxFuture<'a, Answer<()>> {
    Box::pin(async move {
      let detail = Moved {
        detail: moved.map(str::to_owned),
      };

      self.caller.done(Method::POST, &["queue", "moved"], &detail).await
    })
  }
}

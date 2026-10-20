//! What stands around the conversations of one agent in the running process (docs/bmo-window.md, 「The room's
//! window」): the rows that ride a room's next round, from the instruction pass, and the room shown typing while
//! its answer is on its way, on the room's own road, renewed on the round policy's clock until the round lets it
//! go.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use bmo_chat_client::{ChatSend, ChatSendRequest};
use receiver_policy::chat_round::CHAT_ROUND;
use tokio::task::JoinHandle;

use super::conversation::{BoxFuture, Hooks};
use crate::receiver::Receiver;

/// The conversations' surroundings in the running process.
pub struct Around {
  receiver: Receiver,
  // Each room shown typing now, by the task that renews it.
  typing: Mutex<HashMap<i64, JoinHandle<()>>>,
}

impl Around {
  pub fn new(receiver: Receiver) -> Self {
    Self {
      receiver,
      typing: Mutex::new(HashMap::default()),
    }
  }
}

// The room shown typing once, on its own road; a road that shows no typing shows nothing.
async fn type_once(receiver: &Receiver, chat_id: i64) {
  let Some(road) = receiver.roads().answering(chat_id, None).await else {
    eprintln!("chat {chat_id}: no road to show typing on");

    return;
  };
  let facts = receiver.roads().facts(&road).await;

  if !facts.can.iter().any(|one| one == "typing") {
    return;
  }

  if let Err(why) = receiver
    .speech()
    .act(road.key(), &ChatSendRequest::new(chat_id, ChatSend::Typing { on: true }))
    .await
  {
    eprintln!("chat {chat_id}: typing: {why}");
  }
}

impl Hooks for Around {
  fn riders(&self, agent: &str, chat_id: i64) -> BoxFuture<'_, ()> {
    let agent = agent.to_owned();

    Box::pin(async move {
      if let Some(pass) = self.receiver.pass() {
        pass.riders(&agent, chat_id).await;
      }
    })
  }

  fn typing(&self, chat_id: i64, on: bool) -> BoxFuture<'_, ()> {
    let mut typing = self.typing.lock().unwrap_or_else(PoisonError::into_inner);

    if !on {
      if let Some(renewing) = typing.remove(&chat_id) {
        renewing.abort();
      }

      return Box::pin(async {});
    }

    if typing.get(&chat_id).is_some_and(|renewing| !renewing.is_finished()) {
      return Box::pin(async {});
    }

    let receiver = self.receiver.clone();
    let every = Duration::from_millis(CHAT_ROUND.typing.every_ms);

    typing.insert(
      chat_id,
      tokio::spawn(async move {
        loop {
          type_once(&receiver, chat_id).await;
          tokio::time::sleep(every).await;
        }
      }),
    );

    Box::pin(async {})
  }
}

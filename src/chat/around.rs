//! What stands around the conversations of one agent in the running process (docs/bmo-window.md, 「The room's
//! window」): the rows that ride a room's next round, from the instruction pass, the deployment briefing the round
//! finds owed written first (docs/wake.md, 「Quiet instructions」), and the room shown
//! typing while its answer is on its way, on the room's own road, renewed on the round policy's clock until the
//! round lets it go.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use bmo_chat_client::{ChatSend, ChatSendRequest};
use receiver_policy::chat_round::CHAT_ROUND;
use tokio::task::JoinHandle;

use super::conversation::{BoxFuture, Hooks};
use super::relaunch::Briefings;
use crate::receiver::Receiver;

/// The conversations' surroundings in the running process.
pub struct Around {
  receiver: Receiver,
  // Each agent's typing shown in a room now, by the agent and the room, with the task that renews it: two agents
  // answering one room each show their own.
  typing: Mutex<HashMap<(String, i64), JoinHandle<()>>>,
  // Where the deploy roots stand, which writes no deployment briefing until they are named.
  briefings: Briefings,
}

impl Around {
  pub fn new(receiver: Receiver) -> Self {
    Self {
      receiver,
      typing: Mutex::new(HashMap::default()),
      briefings: Briefings::new(None),
    }
  }

  /// The same surroundings reading the units' deployment records under `roots` for a reopened session's
  /// instruction.
  pub fn reading_deploys(self, roots: Option<PathBuf>) -> Self {
    Self {
      briefings: Briefings::new(roots),
      ..self
    }
  }
}

// The room shown typing once, on its own road, in the voice the agent's words would go in there; a road that shows no
// typing shows nothing.
async fn type_once(receiver: &Receiver, agent: &str, chat_id: i64) {
  let Some(road) = receiver.roads().answering(chat_id, None).await else {
    eprintln!("chat {chat_id}: no road to show typing on");

    return;
  };
  let facts = receiver.roads().facts(&road).await;

  if !facts.can.iter().any(|one| one == "typing") {
    return;
  }

  let mut request = ChatSendRequest::new(chat_id, ChatSend::Typing { on: true });

  crate::chat::reply::voice_of(receiver, &road, agent, chat_id, &mut request).await;

  if let Err(why) = receiver.speech().act(road.key(), &request).await {
    eprintln!("chat {chat_id}: typing: {why}");
  }
}

impl Hooks for Around {
  fn riders(&self, agent: &str, chat_id: i64) -> BoxFuture<'_, ()> {
    let agent = agent.to_owned();

    Box::pin(async move {
      self.briefings.note(&self.receiver, &agent, chat_id).await;

      if let Some(pass) = self.receiver.pass() {
        pass.riders(&agent, chat_id).await;
      }
    })
  }

  fn typing(&self, agent: &str, chat_id: i64, on: bool) -> BoxFuture<'_, ()> {
    let mut typing = self.typing.lock().unwrap_or_else(PoisonError::into_inner);

    if !on {
      if let Some(renewing) = typing.remove(&(agent.to_owned(), chat_id)) {
        renewing.abort();
      }

      return Box::pin(async {});
    }

    if typing.get(&(agent.to_owned(), chat_id)).is_some_and(|renewing| !renewing.is_finished()) {
      return Box::pin(async {});
    }

    let receiver = self.receiver.clone();
    let agent = agent.to_owned();
    let every = Duration::from_millis(CHAT_ROUND.typing.every_ms);

    typing.insert(
      (agent.clone(), chat_id),
      tokio::spawn(async move {
        loop {
          type_once(&receiver, &agent, chat_id).await;
          tokio::time::sleep(every).await;
        }
      }),
    );

    Box::pin(async {})
  }
}

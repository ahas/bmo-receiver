//! The one runner of every conversational agent (the agency's `docs/agents.md`, 「One runner」): each agent's conversations in
//! its rooms, made the first time the agent is woken and kept for as long as the process stands, all on the same
//! surroundings and the same timings. The conversations of the default answerer the receiver starts with stand
//! from the start and keep its clocks; the console's talks, the warmer's touch and a typed line go to the
//! conversations of the supervisor the default answerer setting names as each comes (docs/api.md,
//! 「Supervisors」).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use super::conversation::{BoxFuture, Conversations, Hooks, Talking, Timings, Touching, Typing};
use super::talk::{TalkFailure, TalkRequest};
use super::touch::TouchAnswer;
use crate::Fault;
use crate::receiver::{Inbound, Receiver, Room};
use crate::room_record::RoomForget;
use crate::room_record::forget::Forgetting;

/// Every conversational agent's conversations: cloned freely, every clone the same.
#[derive(Clone)]
pub struct Runner(Arc<Inner>);

struct Inner {
  receiver: Receiver,
  hooks: Arc<dyn Hooks>,
  timings: Timings,
  default: Conversations,
  standing: Mutex<HashMap<String, Conversations>>,
}

impl Runner {
  /// The runner with the default answerer's conversations standing.
  pub fn new(receiver: Receiver, hooks: Arc<dyn Hooks>, timings: Timings, default_answerer: &str) -> Self {
    let default = Conversations::new(receiver.clone(), hooks.clone(), timings, default_answerer);
    let standing = HashMap::from([(default_answerer.to_owned(), default.clone())]);

    Self(Arc::new(Inner {
      receiver,
      hooks,
      timings,
      default,
      standing: Mutex::new(standing),
    }))
  }

  /// The conversations of the default answerer the runner started with.
  pub fn default_answerer(&self) -> &Conversations {
    &self.0.default
  }

  /// The conversations of the supervisor the default answerer setting names now.
  pub fn answering_unnamed(&self) -> Conversations {
    self.of(&self.0.receiver.default_answerer())
  }

  /// An agent's conversations, made on first asking.
  pub fn of(&self, agent: &str) -> Conversations {
    let mut standing = self.0.standing.lock().unwrap_or_else(PoisonError::into_inner);

    standing
      .entry(agent.to_owned())
      .or_insert_with(|| {
        Conversations::new(self.0.receiver.clone(), self.0.hooks.clone(), self.0.timings, agent)
      })
      .clone()
  }

  /// An agent's conversations, where they stand already.
  pub fn standing(&self, agent: &str) -> Option<Conversations> {
    self.0.standing.lock().unwrap_or_else(PoisonError::into_inner).get(agent).cloned()
  }

  /// Every agent's conversations standing now.
  pub fn every(&self) -> Vec<Conversations> {
    self
      .0
      .standing
      .lock()
      .unwrap_or_else(PoisonError::into_inner)
      .values()
      .cloned()
      .collect()
  }

  /// Whether no agent's conversations have anything waiting or running.
  pub fn idle(&self) -> bool {
    self.every().iter().all(Conversations::idle)
  }
}

// The runner, as the receiver hands a woken turn to it and forgets a room through it.
impl crate::receiver::Runner for Runner {
  fn hand<'a>(
    &'a self,
    agent: &'a str,
    files: &'a [Inbound],
    message: &'a Inbound,
    room: Room,
    opens_window: bool,
    window_only: bool,
  ) -> BoxFuture<'a, bool> {
    let conversations = self.of(agent);

    Box::pin(async move {
      for file in files {
        conversations.enqueue(file.clone(), room, false, window_only).await;
      }

      conversations.enqueue(message.clone(), room, opens_window, window_only).await;

      true
    })
  }

  fn owed_notes<'a>(&'a self, agent: &'a str, session: &'a str) -> BoxFuture<'a, ()> {
    Box::pin(async move {
      if let Err(why) = self.of(agent).owed_notes(None, Some(session)).await {
        eprintln!("{agent}: the situation notes owed before the handoff were not all written: {why:?}");
      }
    })
  }

  fn forget(&self, chat_id: i64) -> BoxFuture<'_, ()> {
    Box::pin(async move {
      for conversations in self.every() {
        conversations.forget_room(chat_id).await;
      }
    })
  }

  fn spoken<'a>(&'a self, agent: &'a str, chat_id: i64, line: &'a str) -> BoxFuture<'a, Option<String>> {
    let conversations = self.of(agent);

    Box::pin(async move { conversations.spoken(chat_id, line).await })
  }

  fn in_round(&self, agent: &str, chat_id: i64) -> bool {
    self.standing(agent).is_some_and(|conversations| conversations.in_round(chat_id))
  }

  fn in_briefing(&self, agent: &str, chat_id: i64) -> bool {
    self.standing(agent).is_some_and(|conversations| conversations.in_briefing(chat_id))
  }
}

// A talk goes to the default answerer's conversations as the setting stands when it comes.
impl Talking for Runner {
  fn talk<'a>(&'a self, request: &'a TalkRequest) -> BoxFuture<'a, Result<Option<String>, TalkFailure>> {
    Box::pin(async move { self.answering_unnamed().talk(request).await })
  }
}

// The warmer's touch goes to the conversations of the agent it names.
impl Touching for Runner {
  fn touch<'a>(
    &'a self,
    _receiver: &'a Receiver,
    agent: &'a str,
    idle_override_ms: Option<i64>,
  ) -> BoxFuture<'a, Result<TouchAnswer, Fault>> {
    Box::pin(async move { Ok(self.of(agent).touch(idle_override_ms).await) })
  }
}

// A line typed into an agent's program goes to that agent's conversations.
impl Typing for Runner {
  fn typed_line<'a>(
    &'a self,
    _receiver: &'a Receiver,
    agent: &'a str,
    text: &'a str,
    session_id: &'a str,
    at_ms: i64,
  ) -> BoxFuture<'a, Result<String, Fault>> {
    Box::pin(async move { self.of(agent).typed_line(text, session_id, at_ms).await })
  }
}

// Every agent's conversations forget a room whose registration is removed.
fn forget_room(receiver: &Receiver, room: i64) -> Forgetting<'_> {
  Box::pin(async move {
    let Some(runner) = receiver.runner() else { return };

    runner.forget(room).await;
  })
}

crate::room_forget!(RoomForget {
  what: "every agent's conversation in the room, its timer, its queue and its window",
  forget: forget_room,
});

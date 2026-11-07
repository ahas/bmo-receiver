//! The voice briefing turns in flight (docs/voiced-notices.md, 「The summary of a notice turn」 and 「The full
//! reading on a line's chip」): each room an agent's summary or reading turn is being carried in, held for as long
//! as the carry lasts and let go on every way out of it, and the refusal of a send the agent asks into that room
//! meanwhile, since the turn's answer is the script itself and nothing is sent into the room from it.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use receiver_codes::SendRefusal;
use room_calls::sends::Asker;

use crate::receiver::Receiver;

/// The rooms one agent's briefing turns are being carried in, each with how many are.
#[derive(Default)]
pub struct BriefingTurns(Mutex<HashMap<i64, usize>>);

impl BriefingTurns {
  fn rooms(&self) -> std::sync::MutexGuard<'_, HashMap<i64, usize>> {
    self.0.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// A briefing turn carried in the room, held until the answer is dropped.
  pub fn carrying(self: &Arc<Self>, chat_id: i64) -> Carrying {
    *self.rooms().entry(chat_id).or_default() += 1;

    Carrying {
      turns: self.clone(),
      chat_id,
    }
  }

  /// Whether a briefing turn is being carried in the room now.
  pub fn in_flight(&self, chat_id: i64) -> bool {
    self.rooms().get(&chat_id).is_some_and(|held| *held > 0)
  }
}

/// One briefing turn being carried in a room, let go as it is dropped, whichever way the carry ends.
pub struct Carrying {
  turns: Arc<BriefingTurns>,
  chat_id: i64,
}

impl Drop for Carrying {
  fn drop(&mut self) {
    let mut rooms = self.turns.rooms();

    if let Some(held) = rooms.get_mut(&self.chat_id) {
      *held = held.saturating_sub(1);

      if *held == 0 {
        rooms.remove(&self.chat_id);
      }
    }
  }
}

/// The refusal of a send an agent's session asks into a room its briefing turn is being carried in; none for an
/// asker naming no agent or no room, a job's send, or a room no briefing turn of the agent's is carried in.
pub fn refused(receiver: &Receiver, asker: &Asker) -> Option<String> {
  let agent = asker.agent_name.as_deref().map(str::trim).filter(|agent| !agent.is_empty())?;
  let room = asker.room?;

  (!asker.job && receiver.runner().is_some_and(|runner| runner.in_briefing(agent, room))).then(|| {
    SendRefusal::BriefingTurn {
      agent: agent.to_owned(),
      room,
    }
    .to_string()
  })
}

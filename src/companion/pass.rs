//! The words a room session had not taken when the service stopped (the agency's `docs/companions.md`, 「The session」), taken up
//! again as it starts where the receiver's rule still wakes the agent for them, by the agent's own conversations in
//! the one runner. Rows queued for an agent go through the one instruction pass every agent's rows go through
//! (the agency's `docs/agents.md`, 「One runner」).

use receiver_ports::agency::Scope;

use crate::receiver::Runner as RunnerSeam;
use crate::receiver::{Inbound, Receiver, rows};

/// The words every room session had not taken when the process stopped, handed to its agent's conversations in the
/// room where the receiver's rule still wakes the agent for them, each room's together. The wakes it set going.
pub async fn replay(
  receiver: &Receiver,
  runner: crate::chat::runner::Runner,
) -> Vec<tokio::task::JoinHandle<()>> {
  let sessions = receiver.units().agency.sessions().await.ok().unwrap_or_default();
  let mut turns: Vec<tokio::task::JoinHandle<()>> = vec![];

  for session in sessions {
    let room_of_session = match session.scope {
      Scope::Room { room } => Some(room),
      _ => None,
    };

    let Some(chat_id) = room_of_session else {
      continue;
    };

    let agent = session.agent;

    let Some(last_read) = rows::last_read(receiver.db(), chat_id).await.ok().flatten() else {
      continue;
    };

    let road = receiver.roads().room_road(chat_id).await.unwrap_or_default();
    let room = super::room_of(receiver, chat_id).await;
    let after = rows::messages_after(receiver.db(), chat_id, last_read, &road).await.unwrap_or_default();
    let mut untaken: Vec<Inbound> = vec![];

    for message in &after {
      if runner.hand(&agent, &[], message, room, false, false).await {
        untaken.push(Inbound {
          via_reader: Some(false),
          ..message.clone()
        });
      }
    }

    if untaken.is_empty() {
      continue;
    }

    let conversations = runner.of(&agent);

    turns.push(tokio::spawn(async move { conversations.wake(untaken, room).await }));
  }

  turns
}

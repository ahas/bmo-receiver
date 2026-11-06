//! The brain's consolidation as the receiver answers to it (docs/unified-bmo.md, 「The brain's consolidation」):
//! whether a turn's agent's brain says a consolidation is due, which the turn's index block then says; the one
//! quiet turn a session about to be folded is asked to consolidate in; and a fold the receiver asked of a session
//! reported to the agent's brain so its marks of that session clear. Each is asked once; a failure is said in the
//! journal and leaves the turn or the fold as it stands.

use receiver_policy::handover::HANDOVER;
use receiver_ports::agency::{Delivery, Turn, TurnRoom};

use super::room::room_context;
use crate::receiver::Receiver;
use crate::{now_ms, rounds};

/// Whether the agent's brain says a consolidation is due; no where the deployment names no brain, the agent has
/// none, or the brain did not answer.
pub async fn due(receiver: &Receiver, agent: &str) -> bool {
  let Some(brain) = receiver.units().brain.clone() else {
    return false;
  };

  match brain.due(agent).await {
    Ok(due) => due.unwrap_or(false),
    Err(why) => {
      eprintln!("bmo-receiver: {agent}'s brain did not say whether a consolidation is due: {why}");

      false
    }
  }
}

/// The instruction a session about to be folded consolidates its brain by: its marker, with the rule in the
/// room's block.
pub fn instruction() -> String {
  format!("{}\n", HANDOVER.chat.consolidate_marker)
}

/// The session about to be folded asked one quiet turn to consolidate its brain, answering `chat_id` with that
/// room's block, waited on until it ends; whether it was carried. The fold never waits on more than this: where the
/// turn cannot be composed, is refused or fails, the journal says the consolidation was skipped.
pub async fn before_fold(receiver: &Receiver, agent: &str, session: &str, chat_id: i64) -> bool {
  let skipped = |why: String| {
    eprintln!(
      "bmo-receiver: {agent}'s consolidation before the fold of session {session} was skipped: {why}"
    );

    false
  };
  let room = match room_context(receiver, chat_id, None).await {
    Ok(room) => room,
    Err(why) => return skipped(format!("the room {chat_id} could not be composed: {}", why.detail)),
  };
  let turn = Turn {
    session: session.to_owned(),
    prompt: instruction(),
    room: Some(TurnRoom {
      chat_id,
      text: room.text,
      mark: room.mark,
    }),
    via: None,
    privileged: None,
    answers: vec![],
    began: Some(now_ms()),
    round: rounds::current(),
  };

  match receiver.units().agency.turn(&turn).await {
    Ok(Delivery::Delivered) => true,
    Ok(Delivery::Blocked) => skipped("the session took no turn".to_owned()),
    Err(why) => skipped(why.to_string()),
  }
}

/// A fold of `session` reported to the agent's brain, under the session's id as its key; whether the brain took
/// it.
pub async fn report_fold(receiver: &Receiver, agent: &str, session: &str) -> bool {
  let Some(brain) = receiver.units().brain.clone() else {
    return false;
  };

  match brain.fold(agent, session).await {
    Ok(Some(_)) => true,
    Ok(None) => {
      eprintln!("bmo-receiver: the fold of session {session} was reported to no brain: {agent} has none");

      false
    }
    Err(why) => {
      eprintln!("bmo-receiver: the fold of session {session} was not reported to {agent}'s brain: {why}");

      false
    }
  }
}

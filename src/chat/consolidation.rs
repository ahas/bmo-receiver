//! The brain's consolidation as the receiver answers to it (docs/unified-bmo.md, 「The brain's consolidation」):
//! whether a turn's agent's brain says a consolidation is due, which the turn's index block then says, and a fold
//! the receiver asked of a session reported to the agent's brain so its marks of that session clear. Each is
//! asked of the brain once; a failure is said in the journal and leaves the turn or the fold as it stands.

use crate::receiver::Receiver;

/// Whether the agent's brain says a consolidation is due; no where the deployment names no brain, the agent has
/// none, or the brain did not answer.
pub async fn due(receiver: &Receiver, agent: &str) -> bool {
  let Some(brain) = receiver.units().brain.clone() else {
    return false;
  };
  let id = match brain.brain_of(agent).await {
    Ok(Some(id)) => id,
    Ok(None) => return false,
    Err(why) => {
      eprintln!("bmo-receiver: {agent}'s brain could not be found for its due consolidation: {why}");

      return false;
    }
  };

  match brain.due(id).await {
    Ok(due) => due,
    Err(why) => {
      eprintln!("bmo-receiver: {agent}'s brain did not say whether a consolidation is due: {why}");

      false
    }
  }
}

/// A fold of `session` reported to the agent's brain, under the session's id as its key; whether the brain took
/// it.
pub async fn report_fold(receiver: &Receiver, agent: &str, session: &str) -> bool {
  let Some(brain) = receiver.units().brain.clone() else {
    return false;
  };
  let id = match brain.brain_of(agent).await {
    Ok(Some(id)) => id,
    Ok(None) => {
      eprintln!("bmo-receiver: the fold of session {session} was reported to no brain: {agent} has none");

      return false;
    }
    Err(why) => {
      eprintln!(
        "bmo-receiver: the fold of session {session} was not reported: {agent}'s brain was not found: {why}"
      );

      return false;
    }
  };

  match brain.fold(id, session).await {
    Ok(_) => true,
    Err(why) => {
      eprintln!("bmo-receiver: the fold of session {session} was not reported to {agent}'s brain: {why}");

      false
    }
  }
}

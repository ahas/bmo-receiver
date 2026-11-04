//! A room's full session folded (docs/wake.md, 「The room's queue and the round」): the fold is a command to the
//! agent's program, carried by the agency, not words to the agent, and neither before nor after it does the room
//! hear a line of its own. Before the fold the session is asked one quiet turn to consolidate its brain, and a
//! fold the agency answered is reported to the agent's brain; a fold that failed is the journal's, and the room
//! is told nothing.

use receiver_codes::SessionReason;
use receiver_policy::providers::{Handover, PROVIDERS};
use receiver_policy::resident_turn::RESIDENT_TURN;

use super::cache::{active_route, force_reset};
use crate::receiver::Receiver;

/// Whether a room's session is past its fold: a session stands, and the context it carries is past the share the
/// resident turn folds at.
pub fn past_fold(session: Option<&str>, context_ratio: Option<f64>, fold_ratio: Option<f64>) -> bool {
  let share = fold_ratio.unwrap_or(RESIDENT_TURN.fold_ratio);
  session.is_some() && context_ratio.is_some_and(|ratio| ratio >= share)
}

/// The fold of one room's session; whether it folded.
pub async fn compact(receiver: &Receiver, agent: &str, chat_id: i64, session: Option<&str>) -> bool {
  // The session consolidates its brain first, a fold being one of the brain's two triggers; the fold goes ahead
  // whatever became of that turn (docs/unified-bmo.md, 「The brain's consolidation」).
  if let Some(session) = session {
    super::consolidation::before_fold(receiver, agent, session, chat_id).await;
  }

  let held_session = session.unwrap_or_default();

  let folded = match receiver.units().agency.fold(held_session).await {
    Ok(()) => Ok(()),
    Err(why) => Err(why.to_string()),
  };

  match folded {
    Ok(()) => {
      // The brain's marks of what it handed the session clear with the fold (docs/wake.md).
      if let Some(session) = session {
        super::consolidation::report_fold(receiver, agent, session).await;
      }

      // The folded session keeps its name and loses its detail: what it was handed in full goes again.
      if let Err(why) = super::context::forget_documents(receiver, session).await {
        eprintln!("the folded session's loads were not forgotten for chat {chat_id}: {}", why.detail);
      }

      true
    }
    Err(reason) => {
      eprintln!("session compaction failed: {reason}");

      false
    }
  }
}

/// How an agent's one session is handed over once full, as the provider of the road it stands on declares it.
pub async fn handover_of(receiver: &Receiver, agent: &str) -> Handover {
  let db = receiver.db();
  let held = active_route(db, agent).await.ok().flatten();
  let provider = held.map(|route| route.provider).unwrap_or_default();

  PROVIDERS.provider(&provider).map(|found| found.handover).unwrap_or(Handover::Fold)
}

/// An agent's one full session handed over by its own note rather than folded: the owner's word to start it
/// afresh written with the reason `full`, so its next turn asks it for its handoff file and opens the new session
/// on it, the one way a session is replaced (docs/unified-bmo.md, 「Replacing the session」); whether the word
/// was written.
pub async fn hand_off(receiver: &Receiver, agent: &str) -> bool {
  match force_reset(receiver.db(), agent, SessionReason::Full).await {
    Ok(()) => true,
    Err(why) => {
      eprintln!("{agent}: the full session's handoff was not asked for: {}", why.detail);

      false
    }
  }
}

//! A room's full session folded (docs/wake.md, 「The room's queue and the round」): the fold is a command to the
//! agent's program, carried by the agency, not words to the agent; the room is told before and after, in the
//! session's own words where it can be asked and as a fixed line where it cannot; and a fold that did not happen
//! is told to the room and to the owner once per cause, while the same cause stands no line is said again. A fold
//! the agency answered is reported to the agent's brain.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use receiver_codes::{FixedLine, HealthFact, SessionReason};
use receiver_policy::chat_health::CHAT_HEALTH;
use receiver_policy::providers::{Handover, PROVIDERS};
use receiver_policy::resident_turn::RESIDENT_TURN;

use receiver_ports::gate::TurnEvent;

use super::ask::Carry;
use super::cache::{active_route, force_reset};
use super::conversation::{Asking, Carrying, Conversations, Follower, Held};
use super::notices;
use super::notices::Voiced;
use super::sentinels::{is_skip, is_wait};
use super::window::{Material, composed, select};
use crate::form;
use crate::receiver::{Inbound, Receiver, Room, rows};
use crate::{now_ms, one};

/// The failure each room has already been told of: while it stands, a fold is attempted in silence, and the room
/// hears a line again only when what stops the fold has changed.
#[derive(Clone, Debug, Default)]
pub struct Failures(Arc<Mutex<HashMap<i64, String>>>);

impl Failures {
  pub fn new() -> Self {
    Failures(Arc::new(Mutex::new(HashMap::new())))
  }

  pub fn standing(&self, chat_id: i64) -> Option<String> {
    self.0.lock().unwrap_or_else(PoisonError::into_inner).get(&chat_id).cloned()
  }

  pub fn settle(&self, chat_id: i64) {
    self.0.lock().unwrap_or_else(PoisonError::into_inner).remove(&chat_id);
  }

  pub fn note(&self, chat_id: i64, reason: String) {
    self.0.lock().unwrap_or_else(PoisonError::into_inner).insert(chat_id, reason);
  }

  /// Lets go of what a room was told, as its registration is removed.
  pub fn forget(&self, chat_id: i64) {
    self.settle(chat_id);
  }
}

// The fold of one room's session, with the lines before and after it.
pub struct Folding {
  pub room: Room,
  pub recent: Vec<String>,
}

/// Whether a room's session is past its fold: a session stands, and the context it carries is past the share the
/// resident turn folds at.
pub fn past_fold(session: Option<&str>, context_ratio: Option<f64>, fold_ratio: Option<f64>) -> bool {
  let share = fold_ratio.unwrap_or(RESIDENT_TURN.fold_ratio);
  session.is_some() && context_ratio.is_some_and(|ratio| ratio >= share)
}

/// Whether a room hears of its own fold: never the board for the machines, the owner's own chat always, and any
/// other room where its rest notices are on.
pub async fn hears(receiver: &Receiver, chat_id: i64) -> bool {
  let db = receiver.db();

  if receiver.deployment().infra_room == Some(chat_id) {
    return false;
  }

  if receiver.roads().is_owner_room(chat_id).await {
    return true;
  }

  let row = one(
    db,
    r#"SELECT "enabled" FROM "chat_notices" WHERE "chat_id" = $1 AND "kind" = 'rest'"#,
    vec![chat_id.into()],
  )
  .await
  .ok()
  .flatten();

  row.is_none_or(|row| row.try_get::<i64>("", "enabled").ok().is_some_and(|enabled| enabled == 1))
}

/// The line the room hears before its fold, in the session's own words where it can be asked, the fixed line
/// otherwise: the one line before a fold, asked with the tools off.
pub async fn words(
  receiver: &Receiver,
  conversations: &Conversations,
  agent: &str,
  chat_id: i64,
  room: Room,
  recent: &[String],
) -> Option<String> {
  let fallback = receiver.speech().rendered(&FixedLine::Compacting).await;
  let said: Vec<String> = vec![CHAT_HEALTH.fold.prompt.clone()];
  let none: Vec<Inbound> = vec![];
  let empty: Vec<String> = vec![];
  let none_events: Vec<TurnEvent> = vec![];
  let material = Material {
    agent,
    chat_id,
    room,
    messages: &none,
    instructions: &empty,
    said: &said,
    events: &none_events,
    recent,
    holds: None,
    round: None,
    session: None,
  };
  let selection = match select(receiver, &material).await {
    Ok(selection) => selection,
    Err(why) => {
      eprintln!("compaction words failed for chat {chat_id}: {}", why.detail);

      return fallback;
    }
  };
  let began = now_ms();
  let asking = Asking {
    chat_id,
    session: String::new(),
    prompt: composed(&selection, &CHAT_HEALTH.fold.prompt),
    via: None,
    privileged: false,
    answers: vec![],
    began,
    context: selection.context.clone(),
    thread_mark: None,
  };
  let held = Held {
    agent: agent.to_owned(),
    chat_id,
    session: None,
    account: None,
    context_tokens: None,
    all_from_owner: false,
    replaced: vec![],
    last_consumed_id: None,
  };
  let follower = Follower::new(conversations, chat_id);
  let carrying = Carry::new(receiver.clone());

  match carrying.carry(&asking, &held, &follower).await {
    Ok(answered) => {
      let text = form::strip_prompt_scaffolding(&answered.text);

      if text.is_empty() || is_skip(&text) || is_wait(&text) { fallback } else { Some(text) }
    }
    Err(why) => {
      eprintln!("compaction words failed for chat {chat_id}: {why:?}");

      fallback
    }
  }
}

/// The fold of one room's session, with the lines before and after it; whether it folded.
pub async fn compact(
  receiver: &Receiver,
  conversations: &Conversations,
  failures: &Failures,
  agent: &str,
  chat_id: i64,
  session: Option<&str>,
  folding: &Folding,
) -> bool {
  let standing = failures.standing(chat_id);
  let heard = hears(receiver, chat_id).await;

  if standing.is_none()
    && heard
    && let Some(said) = words(receiver, conversations, agent, chat_id, folding.room, &folding.recent).await
  {
    let _ = receiver.speech().say(chat_id, &said).await;
  }

  let held_session = session.unwrap_or_default();

  let folded = match receiver.units().agency.fold(held_session).await {
    Ok(()) => Ok(()),
    Err(why) => Err(why.to_string()),
  };

  match folded {
    Ok(()) => {
      failures.settle(chat_id);

      // The brain's marks of what it handed the session clear with the fold (docs/wake.md).
      if let Some(session) = session {
        super::consolidation::report_fold(receiver, agent, session).await;
      }

      // The folded session keeps its name and loses its detail: what it was handed in full goes again.
      if let Err(why) = super::context::forget_documents(receiver, session).await {
        eprintln!("the folded session's loads were not forgotten for chat {chat_id}: {}", why.detail);
      }

      if standing.is_none() && heard {
        let notice = Voiced {
          fact: CHAT_HEALTH.fold.fact.clone(),
          intent: CHAT_HEALTH.fold.intent.clone(),
          follow_up: None,
          optional: false,
          kind: Some(CHAT_HEALTH.fold.kind.clone()),
          events: vec![],
          rides_ms: None,
        };

        let _ = notices::queue_notice(receiver, chat_id, &notice).await;
      }

      true
    }
    Err(reason) => {
      eprintln!("session compaction failed: {reason}");
      failures.note(chat_id, reason.clone());

      if standing.as_deref() != Some(reason.as_str()) {
        if heard && let Some(line) = receiver.speech().rendered(&FixedLine::CompactFailed).await {
          let _ = receiver.speech().say(chat_id, &line).await;
        }

        if !receiver.roads().is_owner_room(chat_id).await {
          let room = rows::chat_title(receiver.db(), chat_id)
            .await
            .ok()
            .flatten()
            .unwrap_or(chat_id.to_string());
          let fact = HealthFact::FoldFailed { room, reason };
          let owner = receiver.notices_room();
          let notice = Voiced {
            fact: fact.to_string(),
            intent: "tell the owner of a fold that did not happen.".to_owned(),
            follow_up: None,
            optional: false,
            kind: Some(format!("fold-failed:{chat_id}")),
            events: vec![],
            rides_ms: None,
          };

          let _ = notices::queue_notice(receiver, owner, &notice).await;
        }
      }

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

//! A room's full session folded (docs/unified-bmo.md, 「Session compaction」): the fold is a command to the
//! agent's program, carried by the agency, not words to the agent; the room is told before and after, in the
//! session's own words where it can be asked and as a fixed line where it cannot; and a fold that did not happen
//! is told to the room and to the owner once per cause, while the same cause stands no line is said again.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use receiver_codes::{FixedLine, HealthFact};
use receiver_policy::chat_health::CHAT_HEALTH;
use receiver_policy::handover::HANDOVER;
use receiver_policy::providers::{Handover, PROVIDERS};
use receiver_policy::resident_turn::RESIDENT_TURN;

use receiver_ports::gate::TurnEvent;

use super::ask::Carry;
use super::cache::{active_route, hand_over};
use super::conversation::{Asking, Carrying, Conversations, Follower, Held};
use super::notices;
use super::notices::Voiced;
use super::prompt::quiet_room;
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
  pub timeout: Duration,
}

/// How long a fold is given.
pub fn fold_timeout() -> Duration {
  Duration::from_secs(CHAT_HEALTH.fold.timeout_minutes * 60)
}

/// Whether a room's session is past its fold: a session stands, and the context it carries is past the share the
/// resident turn folds at.
pub fn past_fold(session: Option<&str>, context_ratio: Option<f64>) -> bool {
  session.is_some() && context_ratio.is_some_and(|ratio| ratio >= RESIDENT_TURN.fold_ratio)
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
) -> String {
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

      if text.is_empty() || is_skip(&text) || is_wait(&text) { fallback } else { text }
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

  if standing.is_none() && heard {
    let said = words(receiver, conversations, agent, chat_id, folding.room, &folding.recent).await;

    let _ = receiver.speech().say(chat_id, &said, true).await;
  }

  let held_session = session.unwrap_or_default();

  let folded = match receiver.units().agency.fold(held_session).await {
    Ok(()) => Ok(()),
    Err(why) => Err(why.to_string()),
  };

  match folded {
    Ok(()) => {
      failures.settle(chat_id);

      if standing.is_none() && heard {
        let notice = Voiced {
          fact: CHAT_HEALTH.fold.fact.clone(),
          intent: CHAT_HEALTH.fold.intent.clone(),
          follow_up: None,
          optional: false,
          kind: Some(CHAT_HEALTH.fold.kind.clone()),
          ttl_ms: Some(CHAT_HEALTH.fold.ttl_minutes * 60_000),
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
        if heard {
          let line = receiver.speech().rendered(&FixedLine::CompactFailed).await;

          let _ = receiver.speech().say(chat_id, &line, true).await;
        }

        if !receiver.roads().is_owner_room(chat_id).await {
          let room = rows::chat_title(receiver.db(), chat_id)
            .await
            .ok()
            .flatten()
            .unwrap_or(chat_id.to_string());
          let fact = HealthFact::FoldFailed { room, reason };
          let owner = receiver.deployment().owner_id;
          let notice = Voiced {
            fact: fact.to_string(),
            intent: "tell the owner of a fold that did not happen.".to_owned(),
            follow_up: None,
            optional: false,
            kind: Some(format!("fold-failed:{chat_id}")),
            ttl_ms: Some(CHAT_HEALTH.fold.ttl_minutes * 60_000),
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

/// An agent's one session handed over by its own note: the session asked once more, on the run its turns go out
/// on, for its handoff, and the note it wrote held for the fresh session its next turn opens.
pub async fn hand_off(
  receiver: &Receiver,
  conversations: &Conversations,
  agent: &str,
  chat_id: i64,
  session: Option<&str>,
) -> bool {
  let instruction = HANDOVER.chat.carried.first().cloned().unwrap_or_default();
  let said: Vec<String> = vec![instruction.clone()];
  let none: Vec<Inbound> = vec![];
  let empty: Vec<String> = vec![];
  let none_events: Vec<TurnEvent> = vec![];
  let room = quiet_room(receiver, receiver.owner_id(), chat_id).await.unwrap_or(Room::default());
  let material = Material {
    agent,
    chat_id,
    room,
    messages: &none,
    instructions: &empty,
    said: &said,
    events: &none_events,
    recent: &empty,
    holds: None,
    round: None,
  };
  let selection = match select(receiver, &material).await {
    Ok(selection) => selection,
    Err(why) => {
      eprintln!("{agent}: the handoff selection failed: {}", why.detail);

      return false;
    }
  };
  let began = now_ms();
  let asking = Asking {
    chat_id,
    session: session.unwrap_or_default().to_owned(),
    prompt: composed(&selection, &instruction),
    via: None,
    privileged: false,
    answers: vec![],
    began,
    context: selection.context.clone(),
  };
  let held = Held {
    agent: agent.to_owned(),
    chat_id,
    session: session.map(str::to_owned),
    account: None,
    context_tokens: None,
    all_from_owner: false,
    replaced: vec![],
    last_consumed_id: None,
  };
  let follower = Follower::new(conversations, chat_id);
  let carrying = Carry::new(receiver.clone());
  let note = match carrying.carry(&asking, &held, &follower).await {
    Ok(answered) => form::strip_prompt_scaffolding(&answered.text),
    Err(why) => {
      eprintln!("{agent}: the handoff failed: {why:?}");

      return false;
    }
  };

  if note.trim().is_empty()
    || note.trim().to_lowercase() == HANDOVER.chat.lost.first().cloned().unwrap_or_default()
  {
    eprintln!("{agent}: the session wrote no handoff note, it goes on as it was");

    return false;
  }

  match hand_over(receiver.db(), agent, &note).await {
    Ok(()) => true,
    Err(why) => {
      eprintln!("{agent}: the handoff note could not be held: {}", why.detail);

      false
    }
  }
}

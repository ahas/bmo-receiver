//! The cache warmer's touch of a quiet room's session (docs/bmo-window.md, 「The touch」; docs/boundary-api.md,
//! 「The cache warmer」): the touch is a turn of the room the chat agent's session last stood
//! in, handed the keepalive prompt; it goes in only where nothing else of the conversation stands, and the answer
//! to the warmer's ping names which of the four it was — warmed, busy, skipped with a code, or failed with a
//! reason.

use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};

use receiver_codes::SkipReason;
use receiver_policy::chat_health::CHAT_HEALTH;
use receiver_policy::keepalive::KEEPALIVE;
use receiver_policy::providers::PROVIDERS;

use receiver_ports::gate::TurnEvent;

use super::ask::Carry;
use super::cache::active_route;
use super::conversation::{Asking, BoxFuture, Carrying, Conversations, Follower, Held, unified_session};
use super::prompt::{quiet_room, recent_lines};
use super::turnover::last_launch_key;
use super::window::{Material, composed, select};
use crate::now_ms;
use crate::receiver::wake::rest;
use crate::receiver::{Inbound, Receiver, Room};
use crate::store::{Fault, settings};

/// What the warmer's ping names the touch of: the consumer, the target, its kind and subject, and the stretch it
/// is bound to, in the names the warmer's own document gives them.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Ping {
  pub consumer: String,
  pub target: String,
  pub kind: String,
  #[serde(default)]
  pub subject: Option<String>,
  pub bound_ms: i64,
}

/// Which of the four a touch was.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TouchOutcome {
  Warmed,
  Busy,
  Skipped,
  Failed,
}

/// The answer the warmer reads: the outcome, the code where a road was passed over, and an English reason where
/// one is worth saying.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TouchAnswer {
  pub outcome: TouchOutcome,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub code: Option<SkipReason>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub reason: Option<String>,
}

/// The launch the agent's one session last stood on: the room it stood in, whether every word of it was the
/// owner's, whether the room's tools were on, and when it went out, in epoch milliseconds.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastLaunch {
  pub chat_id: i64,
  pub privileged: bool,
  #[serde(default)]
  pub tools_allowed: bool,
  #[serde(default)]
  pub at: i64,
}

pub async fn last_launch(db: &DatabaseConnection, agent: &str) -> Option<LastLaunch> {
  settings::get(db, &last_launch_key(agent))
    .await
    .ok()
    .flatten()
    .and_then(|held| serde_json::from_str::<LastLaunch>(held.as_str()).ok())
}

/// Whether any room is in a conversation or a compaction: the touch goes in only with the agent's quiet.
pub fn busy(conversations: &Conversations) -> bool {
  !conversations.idle()
}

/// How long a session may go untouched before its cached prefix cools, as the provider of the road it stands on
/// declares it.
pub async fn keepalive_idle_ms(receiver: &Receiver, agent: &str) -> i64 {
  let provider = active_route(receiver.db(), agent)
    .await
    .ok()
    .flatten()
    .map(|route| route.provider)
    .unwrap_or_default();
  let minutes = PROVIDERS
    .provider(&provider)
    .and_then(|found| found.keepalive_idle_minutes)
    .unwrap_or(0);

  minutes as i64 * 60_000
}

/// Why the touch is passed over, with the code the warmer reads it under: the agent keeping a session in each room
/// has no one session to keep warm, and a rest means the road would not answer.
pub async fn skipped(receiver: &Receiver, agent: &str) -> Option<TouchAnswer> {
  if !unified_session(receiver, agent).await {
    return Some(TouchAnswer {
      outcome: TouchOutcome::Skipped,
      code: None,
      reason: Some("the agent keeps a session in each room".to_owned()),
    });
  }

  let resting = rest::rest(receiver).await.ok().is_some_and(|resting| resting.resting);

  if resting {
    return Some(TouchAnswer {
      outcome: TouchOutcome::Skipped,
      code: Some(SkipReason::ProviderOff),
      reason: None,
    });
  }

  let provider = active_route(receiver.db(), agent)
    .await
    .ok()
    .flatten()
    .map(|route| route.provider)
    .unwrap_or_default();

  if PROVIDERS.provider(&provider).is_none() {
    return Some(TouchAnswer {
      outcome: TouchOutcome::Skipped,
      code: Some(SkipReason::ProviderOff),
      reason: None,
    });
  }

  None
}

/// The touch of the agent's one session, in the room it last stood in, on the idle stretch the warmer names where
/// it shortens it; what the touch was.
pub async fn touch(
  receiver: &Receiver,
  conversations: &Conversations,
  agent: &str,
  idle_override_ms: Option<i64>,
) -> TouchAnswer {
  let db = receiver.db();

  if busy(conversations) {
    return TouchAnswer {
      outcome: TouchOutcome::Busy,
      code: None,
      reason: None,
    };
  }

  if let Some(skipped) = skipped(receiver, agent).await {
    return skipped;
  }

  let Some(last) = last_launch(db, agent).await else {
    return TouchAnswer {
      outcome: TouchOutcome::Skipped,
      code: None,
      reason: Some("the session never stood in a room".to_owned()),
    };
  };

  let chat_id = last.chat_id;
  let now = now_ms();
  let idle_ms = match idle_override_ms {
    Some(over) => over,
    None => keepalive_idle_ms(receiver, agent).await,
  };

  if now - last.at < idle_ms {
    return TouchAnswer {
      outcome: TouchOutcome::Skipped,
      code: None,
      reason: Some("the session answered not long enough ago".to_owned()),
    };
  }

  let room = quiet_room(receiver, receiver.deployment().owner_id, chat_id)
    .await
    .unwrap_or(Room::default());
  let recent = recent_lines(receiver, receiver.deployment().owner_id, chat_id)
    .await
    .unwrap_or_default();
  let empty: Vec<String> = vec![];
  let none: Vec<Inbound> = vec![];
  let none_events: Vec<TurnEvent> = vec![];
  let said: Vec<String> = vec![KEEPALIVE.prompt.clone()];
  let material = Material {
    agent,
    chat_id,
    room,
    messages: &none,
    instructions: &empty,
    said: &said,
    events: &none_events,
    recent: &recent,
    holds: None,
    round: None,
  };
  let selection = match select(receiver, &material).await {
    Ok(selection) => selection,
    Err(why) => {
      return TouchAnswer {
        outcome: TouchOutcome::Failed,
        code: None,
        reason: Some(why.detail),
      };
    }
  };
  let body = if selection.entries.is_empty() {
    KEEPALIVE.prompt.clone()
  } else {
    format!("{}\n\n{}", CHAT_HEALTH.touch.note, KEEPALIVE.prompt)
  };
  let asking = Asking {
    chat_id,
    session: String::new(),
    prompt: composed(&selection, &body),
    via: None,
    privileged: last.privileged,
    answers: vec![],
    began: now,
    context: selection.context.clone(),
  };
  let held = Held {
    agent: agent.to_owned(),
    chat_id,
    session: None,
    account: None,
    context_tokens: None,
    all_from_owner: last.privileged,
    replaced: vec![],
    last_consumed_id: None,
  };
  let follower = Follower::new(conversations, chat_id);
  let carrying = Carry::new(receiver.clone());

  match carrying.carry(&asking, &held, &follower).await {
    Ok(_) => TouchAnswer {
      outcome: TouchOutcome::Warmed,
      code: None,
      reason: None,
    },
    Err(why) => TouchAnswer {
      outcome: TouchOutcome::Failed,
      code: None,
      reason: Some(format!("{why:?}")),
    },
  }
}

/// The warmer's ping answered: the touch of the chat agent's one session, where the ping is of that target.
pub async fn ping(conversations: &Conversations, ping: &Ping) -> TouchAnswer {
  if ping.kind != "chat-touch" || ping.target != "chat-touch" {
    return TouchAnswer {
      outcome: TouchOutcome::Skipped,
      code: None,
      reason: Some("the ping is not of the chat touch".to_owned()),
    };
  }

  conversations.touch(Some(ping.bound_ms)).await
}

// The chat agent's conversations take the warmer's touch, behind whatever round is running.
impl crate::chat::conversation::Touching for Conversations {
  fn touch<'a>(
    &'a self,
    _receiver: &'a Receiver,
    _agent: &'a str,
    idle_override_ms: Option<i64>,
  ) -> BoxFuture<'a, Result<TouchAnswer, Fault>> {
    Box::pin(async move { Ok(Conversations::touch(self, idle_override_ms).await) })
  }
}

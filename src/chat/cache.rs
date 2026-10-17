//! The road the room's turns stand on and what each session was opened against (docs/unified-bmo.md, 「Replacing
//! the session」): kept in the receiver's own settings because the road outlives the session standing on it. A
//! turn whose road moved, whose login is another, whose prefix moved or outlived its cache, or that the owner
//! asked to start afresh goes out on a fresh session with the old one's note ahead of its words; what a turn
//! leaves is the road its session now stands on and the prefix it went out against.

use std::sync::LazyLock;

use regex::Regex;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};

use receiver_codes::SessionReason;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::fill_marks;
use receiver_policy::handover::HANDOVER;
use receiver_policy::providers::PROVIDERS;

use crate::store::{Fault, settings};
use crate::{now_ms, run};

/// Where the road an agent's one session stands on is kept, the note its replaced session left for the next
/// fresh one, the owner's word to start it afresh, and what a session's cache was opened against.
pub fn active_key(agent: &str) -> String {
  format!("{agent}_active_route")
}

pub fn pending_key(agent: &str) -> String {
  format!("{agent}_pending_summary")
}

pub fn force_key(agent: &str) -> String {
  format!("{agent}_force_reset")
}

pub fn state_key(agent: &str, session: &str) -> String {
  format!("{agent}_cache:{session}")
}

/// The road an agent's one session stands on: the provider and model it answers on, the login it was opened
/// under, the session itself and, where the session reported no session of its own, the words it answered with.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Route {
  pub provider: String,
  pub model: String,
  pub effort: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub account: Option<String>,
  #[serde(default)]
  pub session: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub transcript: Option<String>,
}

/// What the receiver knows of one session's cache between turns: the head of the composition it was opened
/// against, the moment it was last seen, and whether a turn resumed it and read nothing from the cache.
#[derive(Debug, Serialize, Deserialize)]
pub struct CacheState {
  pub prefix: String,
  pub at: i64,
  pub missed: bool,
}

/// The head of a composition that stands before the words this turn brings: what moves between two turns of the
/// same room is the new messages, the quiet instructions and what a turn waited on.
pub fn prefix_of(sent: &str) -> String {
  MOVING.replace_all(sent, "").into_owned()
}

static MOVING: LazyLock<Regex> = LazyLock::new(|| {
  let blocks = [
    (&CHAT_ROUND.blocks.fresh, &CHAT_ROUND.blocks.fresh_end),
    (&CHAT_ROUND.blocks.quiet, &CHAT_ROUND.blocks.quiet_end),
    (&CHAT_ROUND.blocks.waited, &CHAT_ROUND.blocks.waited_end),
  ];
  let parts: Vec<String> = blocks
    .iter()
    .map(|(open, close)| {
      let opening = open.split(" (").next().unwrap_or_default();

      format!(
        r"(?m)^{}(?: \(\d+\))?:[^\n]*\n(?s:.*?)^{}\s*\n?",
        regex::escape(opening),
        regex::escape(close),
      )
    })
    .collect();

  Regex::new(&parts.join("|")).expect("the blocks that move between turns")
});

static ARCHIVE_REPLAY: LazyLock<Regex> = LazyLock::new(|| {
  let opening = CHAT_ROUND.blocks.archived.split(" (").next().unwrap_or_default();

  Regex::new(&format!(
    r"(?m)^{}(?: \(\d+\))?:[^\n]*\n(?s:.*?)^{}\s*\n?",
    regex::escape(opening),
    regex::escape(&CHAT_ROUND.blocks.archived_end),
  ))
  .expect("the archive replay block")
});

/// A prompt with the archive's replay of what the session missed taken out: a fresh session is handed the
/// replaced one's note in its place.
pub fn without_archive_replay(prompt: &str) -> String {
  ARCHIVE_REPLAY.replace_all(prompt, "").into_owned()
}

/// The note a replaced session leaves for the fresh one, as the block its next turn opens with.
pub fn carried_note(note: Option<&str>) -> String {
  let lines = &HANDOVER.chat;

  match note.filter(|held| !held.trim().is_empty()) {
    Some(note) => lines
      .carried
      .iter()
      .map(|line| fill_marks(line, &[("note", note)]))
      .collect::<Vec<_>>()
      .join("\n"),
    None => lines.lost.join("\n"),
  }
}

/// The road an agent's one session stands on, where one is kept.
pub async fn active_route(db: &DatabaseConnection, agent: &str) -> Result<Option<Route>, Fault> {
  Ok(
    settings::get(db, &active_key(agent))
      .await?
      .and_then(|held| serde_json::from_str(&held).ok()),
  )
}

pub async fn set_active_route(db: &DatabaseConnection, agent: &str, route: &Route) -> Result<(), Fault> {
  settings::set(db, &active_key(agent), &serde_json::to_string(route).unwrap_or_default()).await
}

async fn forget_setting(db: &DatabaseConnection, key: &str) -> Result<(), Fault> {
  run(db, r#"DELETE FROM "settings" WHERE "key" = $1"#, vec![key.into()]).await.map(drop)
}

/// Why the road the room stands on is not one this turn may go on: another provider, model or effort, or another
/// login.
pub fn route_reason(
  held: &Route,
  provider: &str,
  model: &str,
  effort: &str,
  account: Option<&str>,
) -> Option<SessionReason> {
  if held.provider != provider || held.model != model || held.effort != effort {
    return Some(SessionReason::Route);
  }

  match account {
    Some(account) if held.account.as_deref().unwrap_or_default() != account => Some(SessionReason::Account),
    _ => None,
  }
}

/// Why the session this turn would resume may not be resumed: the owner's word, a road moved, a session with no
/// prefix to compare, a prefix moved, or a prefix its cache has let go of. `sent` is the composition the turn
/// goes out with, which is what a resumed session must have been opened against.
pub async fn reset_reason(
  db: &DatabaseConnection,
  agent: &str,
  asked: &Route,
  sent: &str,
) -> Result<Option<SessionReason>, Fault> {
  if settings::get(db, &force_key(agent)).await?.is_some_and(|forced| !forced.is_empty()) {
    return Ok(Some(SessionReason::DowngradeForced));
  }

  if let Some(moved) = active_route(db, agent).await?.and_then(|held| {
    route_reason(&held, &asked.provider, &asked.model, &asked.effort, asked.account.as_deref())
  }) {
    return Ok(Some(moved));
  }

  let Some(session) = asked.session.as_ref() else {
    return Ok(None);
  };

  let Some(state) = settings::get(db, &state_key(agent, session))
    .await?
    .and_then(|held| serde_json::from_str::<CacheState>(&held).ok())
  else {
    return Ok(Some(SessionReason::NoPrefix));
  };

  if state.prefix != prefix_of(sent) {
    return Ok(Some(SessionReason::Prefix));
  }

  let minutes = PROVIDERS.provider(&asked.provider).and_then(|found| found.keepalive_idle_minutes);

  Ok(
    minutes
      .filter(|lifetime| now_ms() - state.at >= *lifetime as i64 * 60_000)
      .map(|_| SessionReason::Expired),
  )
}

/// The owner's word to start an agent's one session afresh, with the reason it was asked for.
pub async fn force_reset(db: &DatabaseConnection, agent: &str, reason: SessionReason) -> Result<(), Fault> {
  settings::set(db, &force_key(agent), &reason.to_string()).await
}

/// The note a replaced session left, spent for the fresh session's turn.
pub async fn pending_note(db: &DatabaseConnection, agent: &str) -> Result<Option<String>, Fault> {
  let held = settings::take(db, &pending_key(agent)).await?;

  Ok(held.filter(|note| !note.trim().is_empty()))
}

/// The words a session that reported none of its own leaves for the next: the room it stood in, what the turn
/// asked and what the session answered, carried on from what the road before it kept, cut to the tail.
fn transcript_of(agent: &str, previous: Option<&Route>, text: &str) -> String {
  let lines = &HANDOVER.transcript;
  let said = [
    fill_marks(&lines.room, &[("chat", agent)]),
    fill_marks(&lines.answer, &[("agent", agent), ("text", text)]),
  ]
  .join("\n");
  let joined: Vec<String> = [previous.and_then(|held| held.transcript.clone()).unwrap_or_default(), said]
    .into_iter()
    .filter(|part| !part.is_empty())
    .collect();
  let units: Vec<u16> = joined.join("\n").encode_utf16().collect();

  String::from_utf16_lossy(&units[units.len().saturating_sub(HANDOVER.transcript_tail_chars)..])
}

/// What a turn leaves of its session: the road it went out on, the composition it was sent with, the session it
/// resumed where it resumed one, what the run reported of the cache, and the words it answered with.
#[derive(Debug, Clone)]
pub struct Leaving {
  pub route: Route,
  pub sent: String,
  pub resumed: Option<String>,
  pub cache_reads: Option<i64>,
  pub input: Option<i64>,
  pub text: String,
}

/// What a turn leaves for the next: the prefix its session now stands on, whether it read the cache it should
/// have, and the road the room is on.
pub async fn remember(db: &DatabaseConnection, agent: &str, leaving: &Leaving) -> Result<(), Fault> {
  if let Some(session) = &leaving.route.session {
    let key = state_key(agent, session);
    let missed = settings::get(db, &key)
      .await?
      .and_then(|held| serde_json::from_str::<CacheState>(&held).ok())
      .is_some_and(|state| state.missed);
    let state = CacheState {
      prefix: prefix_of(&leaving.sent),
      at: now_ms(),
      missed: missed
        || (leaving.resumed.is_some()
          && leaving.input.unwrap_or(0) > 0
          && leaving.cache_reads.unwrap_or(0) == 0),
    };

    settings::set(db, &key, &serde_json::to_string(&state).unwrap_or_default()).await?;
  }

  let previous = active_route(db, agent).await?;
  let transcript = leaving
    .route
    .session
    .is_none()
    .then(|| transcript_of(agent, previous.as_ref(), &leaving.text));
  let mut route = leaving.route.clone();

  route.transcript = transcript;

  set_active_route(db, agent, &route).await
}

/// An agent's one session handed over by the note it wrote of itself: the note held for the fresh session its
/// next turn opens, and the road let go.
pub async fn hand_over(db: &DatabaseConnection, agent: &str, note: &str) -> Result<(), Fault> {
  settings::set(db, &pending_key(agent), note).await?;
  forget_setting(db, &active_key(agent)).await
}

//! The road the room's turns stand on and what each session was opened against (docs/unified-bmo.md, 「Replacing
//! the session」): kept in the receiver's own settings because the road outlives the session standing on it. A
//! turn whose road moved, whose login is another, whose head moved or outlived its cache, or that the owner
//! asked to start afresh goes out on a fresh session with the old one's note ahead of its words; what a turn
//! leaves is the road its session now stands on and the head it stood on.

use std::sync::LazyLock;

use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use receiver_codes::SessionReason;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::fill_marks;
use receiver_policy::handover::HANDOVER;
use receiver_policy::providers::PROVIDERS;
use receiver_ports::agency::SessionState;

use super::sentinels::{is_skip, is_wait};
use crate::store::{Fault, settings};
use crate::{now_ms, run};

/// Where the road an agent's one session stands on is kept, the owner's word to start it afresh, and what a
/// session's cache stood on.
pub fn active_key(agent: &str) -> String {
  format!("{agent}_active_route")
}

pub fn force_key(agent: &str) -> String {
  format!("{agent}_force_reset")
}

pub fn state_key(agent: &str, session: &str) -> String {
  format!("{agent}_cache:{session}")
}

/// The road an agent's one session stands on: the provider and model it answers on, the login it was opened
/// under, the session itself and the tail of what its sessions answered in each room, the transcript a replacement
/// falls back on where the closing session wrote no handoff.
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

/// What the receiver knows of one session's cache between turns: the fingerprint of the head its last turn stood
/// on, the reason the agency gave then for a changed prefix, the moment it was last seen, and whether a turn
/// resumed it and read nothing from the cache. A record kept before the head was fingerprinted holds none, and the
/// next turn's head is taken as the one it stands on.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CacheState {
  #[serde(default)]
  pub head: String,
  #[serde(default)]
  pub moved: Option<String>,
  pub at: i64,
  pub missed: bool,
}

/// The receiver's own measure of a session's head, where the agency names no reason for a changed prefix: the
/// fingerprint of the fixed lines every round's turn is written in. A deployment that rewrites them moves it;
/// nothing a room or a turn fills in does, so no turn's own composition is read for it.
pub fn composed_head() -> &'static str {
  &HEAD
}

static HEAD: LazyLock<String> = LazyLock::new(|| {
  let lines = format!(
    "{}\n{:?}\n{:?}\n{:?}",
    CHAT_ROUND.scope,
    (
      &CHAT_ROUND.header,
      &CHAT_ROUND.kinds,
      &CHAT_ROUND.blocks,
      &CHAT_ROUND.closing,
      &CHAT_ROUND.queue_line,
      &CHAT_ROUND.push,
      &CHAT_ROUND.relay,
      &CHAT_ROUND.handed,
      &CHAT_ROUND.console,
      &CHAT_ROUND.typed,
      &CHAT_ROUND.voice,
    ),
    // `Instructions` carries clocks as well as lines; only the lines that stand in the room's block move the
    // head.
    CHAT_ROUND.instructions.restarted_marker,
    CHAT_ROUND.instructions.restarted_rule,
  );

  format!("{:x}", Sha256::digest(lines.as_bytes()))
});

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

/// Why the session this turn would resume may not be resumed: the owner's word, a road moved, a session opened
/// before with no record of its head, a head moved, or a head its cache has let go of. `standing` is the agency's
/// state of the session, read before the turn: whether it was ever opened, and why its prefix changed where the
/// agency names it. Nothing of the turn's own composition is read (docs/unified-bmo.md, 「Replacing the session」).
pub async fn reset_reason(
  db: &DatabaseConnection,
  agent: &str,
  asked: &Route,
  standing: &SessionState,
) -> Result<Option<SessionReason>, Fault> {
  if let Some(forced) = settings::get(db, &force_key(agent)).await?.filter(|forced| !forced.is_empty()) {
    let reason =
      serde_json::from_value(serde_json::Value::String(forced)).unwrap_or(SessionReason::DowngradeForced);

    return Ok(Some(reason));
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
    let opened = standing.generation.as_i64().is_some_and(|generation| generation > 0);

    return Ok(opened.then_some(SessionReason::NoPrefix));
  };

  if head_moved(&state, standing.prefix_change_reason.as_deref()) {
    return Ok(Some(SessionReason::Prefix));
  }

  let minutes = PROVIDERS.provider(&asked.provider).and_then(|found| found.lifetime_minutes());

  Ok(
    minutes
      .filter(|lifetime| now_ms() - state.at >= *lifetime as i64 * 60_000)
      .map(|_| SessionReason::Expired),
  )
}

/// Whether the agent's one session has outlived its cache (docs/bmo-window.md, 「The touch」): its last turn older
/// than the lifetime its provider declares. A session with no record, or a provider declaring none, has not.
pub async fn expired(db: &DatabaseConnection, agent: &str) -> bool {
  let Some(route) = active_route(db, agent).await.ok().flatten() else {
    return false;
  };
  let Some(session) = route.session.as_deref() else {
    return false;
  };
  let Some(state) = settings::get(db, &state_key(agent, session))
    .await
    .ok()
    .flatten()
    .and_then(|held| serde_json::from_str::<CacheState>(&held).ok())
  else {
    return false;
  };

  PROVIDERS
    .provider(&route.provider)
    .and_then(|found| found.lifetime_minutes())
    .is_some_and(|lifetime| now_ms() - state.at >= lifetime as i64 * 60_000)
}

/// Whether the head a session's last turn stood on has moved: the agency naming a reason for a changed prefix
/// other than the one that turn kept, or, where it names none, the receiver's own head no longer the one kept. A
/// record that kept no head has nothing to compare.
pub fn head_moved(state: &CacheState, moved: Option<&str>) -> bool {
  match moved {
    Some(why) => state.moved.as_deref() != Some(why),
    None => !state.head.is_empty() && state.head != composed_head(),
  }
}

/// The owner's word to start an agent's one session afresh, with the reason it was asked for.
pub async fn force_reset(db: &DatabaseConnection, agent: &str, reason: SessionReason) -> Result<(), Fault> {
  settings::set(db, &force_key(agent), &reason.to_string()).await
}

/// The owner's word to start afresh spent by the replacement it asked for.
pub async fn spend_force(db: &DatabaseConnection, agent: &str) -> Result<(), Fault> {
  forget_setting(db, &force_key(agent)).await
}

/// What the receiver knew of a session's cache let go, as the session is replaced: the next turn starts its record.
pub async fn forget_state(db: &DatabaseConnection, agent: &str, session: &str) -> Result<(), Fault> {
  forget_setting(db, &state_key(agent, session)).await
}

/// The transcript the road keeps of what the agent's sessions answered, where it holds any: the note a replaced
/// session's successor falls back on.
pub async fn transcript(db: &DatabaseConnection, agent: &str) -> Result<Option<String>, Fault> {
  Ok(
    active_route(db, agent)
      .await?
      .and_then(|route| route.transcript)
      .filter(|kept| !kept.trim().is_empty()),
  )
}

/// The transcript carried on with one more answer of the agent's in a room, cut to the tail: an answer that
/// says nothing (empty, a skip or a wait) adds nothing.
fn transcript_of(agent: &str, chat_id: i64, previous: Option<&Route>, text: &str) -> Option<String> {
  let kept = previous.and_then(|held| held.transcript.clone()).unwrap_or_default();
  let text = text.trim();

  if text.is_empty() || is_skip(text) || is_wait(text) {
    return (!kept.is_empty()).then_some(kept);
  }

  let said = fill_marks(
    &HANDOVER.transcript.answer,
    &[("agent", agent), ("chat", &chat_id.to_string()), ("text", text)],
  );
  let joined: Vec<String> = [kept, said].into_iter().filter(|part| !part.is_empty()).collect();
  let units: Vec<u16> = joined.join("\n").encode_utf16().collect();

  Some(String::from_utf16_lossy(
    &units[units.len().saturating_sub(HANDOVER.transcript_tail_chars)..],
  ))
}

/// What a turn leaves of its session: the room it answered, the road it went out on, the agency's reason for a
/// changed prefix the session's state named after it, the session it resumed where it resumed one, what the run
/// reported of the cache, and the words it answered with.
#[derive(Debug, Clone)]
pub struct Leaving {
  pub chat_id: i64,
  pub route: Route,
  pub moved: Option<String>,
  pub resumed: Option<String>,
  pub cache_reads: Option<i64>,
  pub input: Option<i64>,
  pub text: String,
}

/// What a turn leaves for the next: the head its session now stands on, whether it read the cache it should
/// have, and the road the room is on.
pub async fn remember(db: &DatabaseConnection, agent: &str, leaving: &Leaving) -> Result<(), Fault> {
  if let Some(session) = &leaving.route.session {
    let key = state_key(agent, session);
    let missed = settings::get(db, &key)
      .await?
      .and_then(|held| serde_json::from_str::<CacheState>(&held).ok())
      .is_some_and(|state| state.missed);
    let state = CacheState {
      head: composed_head().to_owned(),
      moved: leaving.moved.clone(),
      at: now_ms(),
      missed: missed
        || (leaving.resumed.is_some()
          && leaving.input.unwrap_or(0) > 0
          && leaving.cache_reads.unwrap_or(0) == 0),
    };

    settings::set(db, &key, &serde_json::to_string(&state).unwrap_or_default()).await?;
  }

  let previous = active_route(db, agent).await?;
  let transcript = transcript_of(agent, leaving.chat_id, previous.as_ref(), &leaving.text);
  let mut route = leaving.route.clone();

  route.transcript = transcript;

  set_active_route(db, agent, &route).await
}

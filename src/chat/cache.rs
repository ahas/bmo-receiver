//! The road the room's turns stand on and what each session was opened against (docs/unified-bmo.md, 「Replacing
//! the session」): kept in the receiver's own settings because the road outlives the session standing on it. A
//! turn whose road moved or whose login is another goes out on a fresh session with the old one's note ahead of
//! its words; a turn whose head moved, or that the owner asked to start afresh, goes on in the session as it
//! stands and leaves the replacement owed, to be made once the agent is quiet; what a turn leaves is the road its
//! session now stands on and the head it stood on.

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

/// Where the replacement an agent's one session owes is kept, with the reason it was first owed for, until it is
/// made (docs/unified-bmo.md, 「Replacing the session」).
pub fn owed_key(agent: &str) -> String {
  format!("{agent}_replacement_owed")
}

pub fn state_key(agent: &str, session: &str) -> String {
  format!("{agent}_cache:{session}")
}

/// The road an agent's one session stands on: the provider and model it answers on, the session itself and the
/// tail of what its sessions answered in each room, the transcript a replacement falls back on where the closing
/// session wrote no handoff. The effort and the login are the agency's to judge, whose state names neither
/// (docs/unified-bmo.md, 「Replacing the session」).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Route {
  pub provider: String,
  pub model: String,
  #[serde(default)]
  pub session: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub transcript: Option<String>,
}

/// What the receiver knows of one session's cache between turns: the fingerprint of the head its last turn stood
/// on, the reason the agency gave then for a changed prefix, the moment it was last seen, and whether a turn
/// resumed it and read nothing from the cache. A record kept before the head was fingerprinted holds none, and the
/// next turn's head is taken as the one it stands on. `process` is the fingerprint of the block's rules for reading
/// a turn as that turn carried them; a record that kept none has nothing to compare.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CacheState {
  #[serde(default)]
  pub head: String,
  #[serde(default)]
  pub process: String,
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
  let mut sections = vec![
    format!("{:?}", CHAT_ROUND.header),
    format!("{:?}", CHAT_ROUND.kinds),
    format!("{:?}", CHAT_ROUND.blocks),
    format!("{:?}", CHAT_ROUND.closing),
    format!("{:?}", CHAT_ROUND.queue_line),
    format!("{:?}", CHAT_ROUND.push),
    format!("{:?}", CHAT_ROUND.relay),
    format!("{:?}", CHAT_ROUND.handed),
    format!("{:?}", CHAT_ROUND.console),
    format!("{:?}", CHAT_ROUND.typed),
    format!("{:?}", CHAT_ROUND.voice),
    format!("{:?}", CHAT_ROUND.supervisor),
    format!("{:?}", CHAT_ROUND.notice),
    format!("{:?}", HANDOVER),
  ];

  // `Instructions` carries clocks as well as lines; only the lines that stand in the room's block move the
  // head.
  sections.push(CHAT_ROUND.instructions.restarted_marker.clone());
  sections.push(CHAT_ROUND.instructions.restarted_rule.clone());

  let lines = format!("{}\n{}", CHAT_ROUND.scope, sections.join("\n"));

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

/// Why the road the room stands on is not one this turn may go on: another provider or model. A provider or model
/// the agency's state does not name — a state it could not answer while it restarts, or a session opened afresh
/// and not yet through its first turn — says nothing of the road, so it moves nothing. A move of the effort or the
/// login is the agency's to find, which asks the session's handoff itself before it opens a fresh one
/// (docs/unified-bmo.md, 「Replacing the session」).
pub fn route_reason(held: &Route, provider: &str, model: &str) -> Option<SessionReason> {
  if provider.is_empty() || model.is_empty() {
    return None;
  }

  (held.provider != provider || held.model != model).then_some(SessionReason::Route)
}

/// Why the session this turn would resume may not be resumed as it stands: the owner's word, a road moved, a
/// session opened before with no record of its head, a head moved, or the block's rules for reading a turn no
/// longer the ones its last turn carried. A cache lifetime passed is no reason: the turn resumes the session and the
/// provider reads its prefix again. `standing` is the agency's
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

  if let Some(moved) = active_route(db, agent)
    .await?
    .and_then(|held| route_reason(&held, &asked.provider, &asked.model))
  {
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

  if !state.process.is_empty() && state.process != super::room::process_mark() {
    return Ok(Some(SessionReason::Process));
  }

  Ok(None)
}

/// Whether a replacement for this reason waits for the agent to be quiet rather than standing between a question
/// and its answer (docs/unified-bmo.md, 「Replacing the session」). A session whose road moved, whose window is
/// full or whose model was replaced on its own cannot take the turn, so it is replaced ahead of it; every other
/// reason leaves the turn to go on in the session as it stands.
pub fn waits_for_quiet(reason: SessionReason) -> bool {
  matches!(
    reason,
    SessionReason::Prefix | SessionReason::NoPrefix | SessionReason::Process | SessionReason::Asked
  )
}

/// The replacement written down as owed for a reason: true where it was not owed before, false where it already
/// was, the first reason kept.
pub async fn owe_replacement(
  db: &DatabaseConnection,
  agent: &str,
  reason: SessionReason,
) -> Result<bool, Fault> {
  if settings::get(db, &owed_key(agent)).await?.is_some_and(|held| !held.is_empty()) {
    return Ok(false);
  }

  settings::set(db, &owed_key(agent), &reason.to_string()).await?;

  Ok(true)
}

/// The replacement the agent's one session owes, if any, by the reason it was first owed for.
pub async fn owed_replacement(
  db: &DatabaseConnection,
  agent: &str,
) -> Result<Option<SessionReason>, Fault> {
  Ok(
    settings::get(db, &owed_key(agent))
      .await?
      .filter(|held| !held.is_empty())
      .and_then(|held| serde_json::from_value(serde_json::Value::String(held)).ok()),
  )
}

/// The owed replacement settled: made, or no longer due.
pub async fn settle_replacement(db: &DatabaseConnection, agent: &str) -> Result<(), Fault> {
  forget_setting(db, &owed_key(agent)).await
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

/// The settings key the tail of what an agent answered in one room is kept under.
pub fn room_transcript_key(agent: &str, chat_id: i64) -> String {
  format!("{agent}_transcript:{chat_id}")
}

/// The tail of what an agent answered in one room, where it answered there.
pub async fn room_transcript(
  db: &DatabaseConnection,
  agent: &str,
  chat_id: i64,
) -> Result<Option<String>, Fault> {
  Ok(
    settings::get(db, &room_transcript_key(agent, chat_id))
      .await?
      .filter(|kept| !kept.trim().is_empty()),
  )
}

/// The transcript carried on with one more answer of the agent's in a room, cut to the tail: an answer that
/// says nothing (empty, a skip or a wait) adds nothing.
fn transcript_of(agent: &str, chat_id: i64, previous: Option<String>, text: &str) -> Option<String> {
  let kept = previous.unwrap_or_default();
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
/// changed prefix the session's state named after it, the fingerprint of the block's rules for reading a turn it
/// carried, whether the turn judged a replacement before it went in, the session it resumed where it resumed one,
/// what the run reported of the cache, and the words it answered with.
#[derive(Debug, Clone)]
pub struct Leaving {
  pub chat_id: i64,
  pub route: Route,
  pub moved: Option<String>,
  pub process: String,
  pub judged: bool,
  pub resumed: Option<String>,
  pub cache_reads: Option<i64>,
  pub input: Option<i64>,
  pub text: String,
}

/// What a turn leaves for the next: the head its session now stands on, whether it read the cache it should
/// have, and the road the room is on. A turn that judged no replacement leaves the fingerprints as the last judged
/// turn left them, so the next judged turn still finds what a deployment changed (docs/unified-bmo.md, 「Replacing
/// the session」).
pub async fn remember(db: &DatabaseConnection, agent: &str, leaving: &Leaving) -> Result<(), Fault> {
  if let Some(session) = &leaving.route.session {
    let key = state_key(agent, session);
    let held = settings::get(db, &key)
      .await?
      .and_then(|held| serde_json::from_str::<CacheState>(&held).ok());
    let missed = held.as_ref().is_some_and(|state| state.missed);
    let kept = held.filter(|_| !leaving.judged);
    let state = CacheState {
      head: kept.as_ref().map_or_else(|| composed_head().to_owned(), |state| state.head.clone()),
      process: kept.as_ref().map_or_else(|| leaving.process.clone(), |state| state.process.clone()),
      moved: kept.as_ref().map_or_else(|| leaving.moved.clone(), |state| state.moved.clone()),
      at: now_ms(),
      missed: missed
        || (leaving.resumed.is_some()
          && leaving.input.unwrap_or(0) > 0
          && leaving.cache_reads.unwrap_or(0) == 0),
    };

    settings::set(db, &key, &serde_json::to_string(&state).unwrap_or_default()).await?;
  }

  let previous = active_route(db, agent).await?;
  let transcript =
    transcript_of(agent, leaving.chat_id, previous.and_then(|held| held.transcript), &leaving.text);
  let room_key = room_transcript_key(agent, leaving.chat_id);
  let room_kept = settings::get(db, &room_key).await?;

  // The room's own tail is kept beside the one across rooms, so a room's own session opened without its handoff
  // reads that room's answers alone.
  if let Some(room) = transcript_of(agent, leaving.chat_id, room_kept, &leaving.text) {
    settings::set(db, &room_key, &room).await?;
  }

  let mut route = leaving.route.clone();

  route.transcript = transcript;

  set_active_route(db, agent, &route).await
}

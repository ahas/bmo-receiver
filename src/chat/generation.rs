//! The generation of an agent's session the receiver last answered on (docs/unified-bmo.md, 「Replacing the
//! session」): the agency opens a new generation as it hands a session over, and a turn that finds a generation
//! past the one the receiver answered on releases what the ended generation held.

use sea_orm::ConnectionTrait;

use receiver_ports::agency::SessionState;
use receiver_ports::http::PortFailure;

use super::cache::forget_state;
use crate::receiver::Receiver;
use crate::store::{Fault, settings};

/// What a session that ended or was handed over no longer holds: the room blocks it read, the note versions it
/// was handed, the cache record of its head, and the round log's count of what it read, cut where it ended
/// (docs/unified-bmo.md, 「Replacing the session」; docs/instruction.md, 「The round log」). The agency
/// releases the loads as it opens the generation, so a record already emptied is not deleted again; a turn the
/// receiver composed against the ended generation may have written its loads back after that, and those are the
/// receiver's to clear.
pub async fn forget(receiver: &Receiver, agent: &str, session: &str) {
  let db = receiver.db();
  let loads = super::context::loaded_in(receiver, Some(session)).await;

  if loads.map(|loads| !loads.is_empty()).unwrap_or(true)
    && let Err(why) = super::context::forget_documents(receiver, Some(session)).await
  {
    eprintln!("{agent}: session {session}'s loads were not forgotten: {}", why.detail);
  }

  if let Err(why) = super::context::forget_room(db, Some(session)).await {
    eprintln!("{agent}: session {session}'s room block was not forgotten: {}", why.detail);
  }

  if let Err(why) = super::situation::forget(db, Some(session)).await {
    eprintln!("{agent}: session {session}'s note versions were not forgotten: {}", why.detail);
  }

  if let Err(why) = forget_state(db, agent, session).await {
    eprintln!("{agent}: session {session}'s cache record was not forgotten: {}", why.detail);
  }

  if let Err(why) = super::stages::log::cut(db, session).await {
    eprintln!("{agent}: session {session}'s count of what it read was not cut: {}", why.detail);
  }
}

/// A generation the agency opened under a session released: what the ended generation held forgotten, and the
/// fold reported to the brain so the marks it handed that generation clear (docs/boundary-api.md, 「The brain」).
pub async fn release(receiver: &Receiver, agent: &str, session: &str) {
  super::consolidation::report_fold(receiver, agent, session).await;
  forget(receiver, agent, session).await;

  if let Err(why) = settle(receiver.db(), agent, session).await {
    eprintln!("{agent}: session {session}'s coming generation was not settled: {}", why.detail);
  }
}

/// Where the generation a session last answered on is kept, under the agent and the session's id.
pub fn key(agent: &str, session: &str) -> String {
  format!("{agent}_generation:{session}")
}

/// The generation the receiver last answered on a session, none where no turn of it was answered.
pub async fn answered_on(
  db: &impl ConnectionTrait,
  agent: &str,
  session: &str,
) -> Result<Option<u64>, Fault> {
  Ok(settings::get(db, &key(agent, session)).await?.and_then(|held| held.parse().ok()))
}

/// The generation a session last answered on written down; nothing written where none is given.
pub async fn keep(
  db: &impl ConnectionTrait,
  agent: &str,
  session: &str,
  generation: Option<u64>,
) -> Result<(), Fault> {
  let Some(generation) = generation else {
    return Ok(());
  };

  settings::set(db, &key(agent, session), &generation.to_string()).await
}

/// Where a generation the agency said it was opening under a session is kept until the turn after it.
pub fn pending_key(agent: &str, session: &str) -> String {
  format!("{agent}_generation_pending:{session}")
}

/// Whether the agency said it was opening a generation under the session and no turn has released it yet.
pub async fn pending(db: &impl ConnectionTrait, agent: &str, session: &str) -> Result<bool, Fault> {
  Ok(settings::get(db, &pending_key(agent, session)).await?.is_some())
}

/// The agency's word that a generation opened under the session kept for the next turn, which releases it. The
/// session's state names a higher generation where the agency keeps one per room; the one session across rooms
/// names none, so its handover is heard on the agency's event stream alone.
pub async fn expect(db: &impl ConnectionTrait, agent: &str, session: &str) -> Result<(), Fault> {
  settings::set(db, &pending_key(agent, session), "1").await
}

/// The kept word let go, by the turn that released the generation it named.
pub async fn settle(db: &impl ConnectionTrait, agent: &str, session: &str) -> Result<(), Fault> {
  settings::forget(db, &pending_key(agent, session)).await
}

/// What a read of a session's state says of the generation its turn goes on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
  /// The state names the generation the session stands in; 0 is a session never opened.
  At(u64),
  /// The state names no session the agency holds: the harness session is gone.
  Gone,
  /// The state was not read, or answered a fault that says nothing of the session.
  Unread,
}

impl Standing {
  /// What a read of the session's state answered.
  pub fn of(state: &Result<SessionState, PortFailure>) -> Self {
    match state {
      Ok(state) => Standing::At(state.generation.as_u64().unwrap_or_default()),
      // The agency answers an id it holds no session for by this code, which tells a session gone from a call
      // that could not be made.
      Err(why) if why.code() == Some("unknown-session") => Standing::Gone,
      Err(_) => Standing::Unread,
    }
  }

  /// The generation the state named, where it named one.
  pub fn at(self) -> Option<u64> {
    match self {
      Standing::At(generation) => Some(generation),
      Standing::Gone | Standing::Unread => None,
    }
  }

  /// The generation to keep once the release is made: the one the turn found, and 0 where the session is gone, so
  /// a turn that finds it gone again releases nothing twice.
  pub fn released(self) -> Option<u64> {
    match self {
      Standing::At(generation) => Some(generation),
      Standing::Gone => Some(0),
      Standing::Unread => None,
    }
  }
}

/// Whether a turn opens a generation past the one the session last answered on: a higher generation, or a session
/// gone where one was answered on before (docs/unified-bmo.md, 「Replacing the session」). A session never opened,
/// one no turn answered on before, and a state that named no generation open nothing.
pub fn changed(answered: Option<u64>, standing: Standing) -> bool {
  let Some(answered) = answered.filter(|answered| *answered > 0) else {
    return false;
  };

  match standing {
    Standing::At(generation) => generation > answered || generation == 0,
    Standing::Gone => true,
    Standing::Unread => false,
  }
}

/// The generation to keep after a turn: the one the turn opened where a session was never opened or came back, and
/// the one the turn was composed against otherwise, so a generation the agency opened while the turn ran is
/// released by the next one. A session the receiver replaced starts its record at the generation it ended on.
pub fn after(answered: Option<u64>, standing: Standing, ended: Option<u64>, replaced: bool) -> Option<u64> {
  if replaced {
    return Some(ended.unwrap_or_default());
  }

  match standing.at() {
    Some(before) if before > 0 => match ended {
      // A generation opened or a session lost while the turn ran is the next turn's to release.
      Some(ended) if ended > before || ended == 0 => Some(before),
      Some(ended) => Some(ended),
      None => Some(before),
    },
    _ => ended.or(answered),
  }
}

//! The handoff a replaced session leaves its successor (docs/unified-bmo.md, 「Replacing the session」): before the
//! agent's one session is ended it is asked one more turn to write its handoff file under the data root, and the
//! new session's first turn opens on that file, on the receiver's transcript of the old session where it wrote
//! nothing, and on the word that nothing was recovered only where neither stands, the owner told so once.

use std::path::{Path, PathBuf};
use std::time::Duration;

use receiver_codes::{HealthFact, SessionReason};
use receiver_policy::fill_marks;
use receiver_policy::handover::HANDOVER;
use receiver_ports::agency::Turn;

use super::notices::{self, Voiced};
use crate::receiver::Receiver;
use crate::{now_ms, rounds};

/// A handoff file a closing session wrote: where it stands and its whole text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Handoff {
  pub path: PathBuf,
  pub text: String,
}

/// What a new session's first turn opens on: its predecessor's handoff, the receiver's transcript of it, or
/// nothing recovered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Note {
  Handed(Handoff),
  Carried(String),
  Lost,
}

/// The folder an agent's handoffs are kept in, under the data root.
pub fn folder(root: &Path, agent: &str) -> PathBuf {
  root.join(fill_marks(&HANDOVER.folder, &[("agent", agent)]))
}

/// The instruction a closing session writes its handoff to `path` by.
pub fn instruction(path: &Path) -> String {
  let path = path.display().to_string();

  HANDOVER
    .chat
    .ask
    .iter()
    .map(|line| fill_marks(line, &[("path", &path)]))
    .collect::<Vec<_>>()
    .join("\n")
}

/// The handoff written at `path`, where it stands and is not empty, copied to the newest beside it.
pub fn read(path: &Path) -> Option<Handoff> {
  let text = std::fs::read_to_string(path).ok()?;

  if text.trim().is_empty() {
    return None;
  }

  if let Some(folder) = path.parent()
    && let Err(why) = std::fs::copy(path, folder.join(&HANDOVER.latest))
  {
    eprintln!("the newest handoff was not copied beside {}: {why}", path.display());
  }

  Some(Handoff {
    path: path.to_owned(),
    text,
  })
}

/// The closing session asked one more turn, bounded by the policy, to write its handoff file; the file it wrote,
/// where it stands and is not empty. A run that fails or outlasts the bound leaves whatever the file holds.
pub async fn ask(receiver: &Receiver, agent: &str, session: &str) -> Option<Handoff> {
  let folder = folder(receiver.data_root().path(), agent);

  if let Err(why) = std::fs::create_dir_all(&folder) {
    eprintln!("{agent}: the handoffs' folder {} could not be made: {why}", folder.display());

    return None;
  }

  let path = folder.join(format!("{}.md", now_ms()));
  let turn = Turn {
    session: session.to_owned(),
    prompt: instruction(&path),
    room: None,
    via: None,
    privileged: None,
    answers: vec![],
    began: Some(now_ms()),
    round: rounds::current(),
  };
  let agency = receiver.units().agency.clone();
  let bound = Duration::from_secs(HANDOVER.ask_seconds);

  match tokio::time::timeout(bound, agency.turn(&turn)).await {
    Ok(Ok(_)) => {}
    Ok(Err(why)) => eprintln!("{agent}: the handoff turn of session {session} failed: {why}"),
    Err(_) => eprintln!("{agent}: the handoff turn of session {session} outlasted {}s", bound.as_secs()),
  }

  let handed = read(&path);

  if handed.is_none() {
    eprintln!("{agent}: session {session} wrote no handoff at {}", path.display());
  }

  handed
}

/// The note a new session's first turn opens with, in the handover policy's lines.
pub fn carried_note(note: &Note) -> String {
  let lines = &HANDOVER.chat;
  let filled = |lines: &[String], marks: &[(&str, &str)]| {
    lines.iter().map(|line| fill_marks(line, marks)).collect::<Vec<_>>().join("\n")
  };

  match note {
    Note::Handed(handoff) => {
      let path = handoff.path.display().to_string();

      filled(&lines.handed, &[("path", &path), ("note", handoff.text.trim_end())])
    }
    Note::Carried(transcript) => filled(&lines.carried, &[("note", transcript.trim_end())]),
    Note::Lost => lines.lost.join("\n"),
  }
}

/// The owner told, once in the private chat, that nothing of a replaced session could be carried over.
pub async fn tell_lost(receiver: &Receiver, agent: &str, reason: SessionReason) {
  let said = &HANDOVER.lost_notice;
  let notice = Voiced {
    fact: HealthFact::HandoffLost {
      agent: agent.to_owned(),
      reason: reason.to_string(),
    }
    .to_string(),
    intent: said.intent.clone(),
    follow_up: None,
    optional: false,
    kind: Some(said.kind.clone()),
    ttl_ms: Some(said.ttl_minutes * 60_000),
    events: vec![],
    rides_ms: None,
  };

  if let Err(why) = notices::queue_notice(receiver, receiver.notices_room(), &notice).await {
    eprintln!("{agent}: the owner was not told the handoff was lost: {why}");
  }
}

/// The settings key a handoff written ahead of a fresh session the agency opens is kept under, until that session's
/// first turn carries it (docs/unified-bmo.md, 「Replacing the session」).
pub fn pending_key(agent: &str) -> String {
  format!("{agent}_handoff_pending")
}

// The agents whose handoff is being asked for now.
static UNDER_WAY: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<String>>> =
  std::sync::LazyLock::new(Default::default);

/// An agent's handoff being asked for, held so a second asking for the same agent is told it is under way; none
/// where one already is. Dropped, it lets the agent go.
pub struct Asking(String);

impl Asking {
  pub fn begin(agent: &str) -> Option<Self> {
    let mut under_way = UNDER_WAY.lock().unwrap_or_else(std::sync::PoisonError::into_inner);

    under_way.insert(agent.to_owned()).then(|| Asking(agent.to_owned()))
  }
}

impl Drop for Asking {
  fn drop(&mut self) {
    UNDER_WAY.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(&self.0);
  }
}

/// The handoff kept for an agent's next fresh session taken: its note where the session the turn goes on is a fresh
/// one and the file still stands, none otherwise. Taken either way, so a session the agency resumed lets it go.
pub async fn take_pending(receiver: &Receiver, agent: &str, fresh: bool) -> Option<Note> {
  let path = crate::store::settings::take(receiver.db(), &pending_key(agent)).await.ok().flatten()?;

  if !fresh {
    return None;
  }

  let text = std::fs::read_to_string(&path).ok().filter(|text| !text.trim().is_empty())?;

  Some(Note::Handed(Handoff {
    path: PathBuf::from(path),
    text,
  }))
}

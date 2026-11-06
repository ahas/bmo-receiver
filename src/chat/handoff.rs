//! The handoff a replaced session leaves its successor (docs/unified-bmo.md, 「Replacing the session」): before the
//! agent's one session is ended it is asked one more turn to write its handoff file under the data root, and the
//! new session's first turn opens on that file, on the receiver's transcript of the old session where it wrote
//! nothing, and on the word that nothing was recovered only where neither stands, the owner told so once.

use std::path::{Path, PathBuf};

use receiver_codes::{HealthFact, SessionReason};
use receiver_policy::fill_marks;
use receiver_policy::handover::HANDOVER;
use receiver_ports::agency::{Turn, TurnRoom};

use super::notices::{self, Voiced};
use super::room::room_context;
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
  let chat = &HANDOVER.chat;

  format!("{}\n{}", chat.ask_marker, fill_marks(&chat.ask, &[("path", &path)]))
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

/// The closing session asked one more turn to write its handoff file, waited on until the turn ends however long it
/// takes; the file it wrote, where it stands and is not empty. A run that fails leaves whatever the file holds.
/// The turn answers `chat_id`, carrying that room's block as every turn of a session across rooms does.
pub async fn ask(receiver: &Receiver, agent: &str, session: &str, chat_id: i64) -> Option<Handoff> {
  let folder = folder(receiver.data_root().path(), agent);

  if let Err(why) = std::fs::create_dir_all(&folder) {
    eprintln!("{agent}: the handoffs' folder {} could not be made: {why}", folder.display());

    return None;
  }

  let room = match room_context(receiver, chat_id, None).await {
    Ok(room) => room,
    Err(why) => {
      eprintln!("{agent}: the handoff turn's room {chat_id} could not be composed: {}", why.detail);

      return None;
    }
  };
  let path = folder.join(format!("{}.md", now_ms()));
  let turn = Turn {
    session: session.to_owned(),
    prompt: instruction(&path),
    room: Some(TurnRoom {
      chat_id,
      text: room.text,
      mark: room.mark,
    }),
    via: None,
    privileged: None,
    answers: vec![],
    began: Some(now_ms()),
    round: rounds::current(),
  };
  let agency = receiver.units().agency.clone();

  if let Err(why) = agency.turn(&turn).await {
    eprintln!("{agent}: the handoff turn of session {session} failed: {why}");
  }

  let handed = read(&path);

  if handed.is_none() {
    eprintln!("{agent}: session {session} wrote no handoff at {}", path.display());
  }

  handed
}

/// The note a new session's first turn opens with, in the handover policy's lines.
pub fn carried_note(note: &Note) -> String {
  let chat = &HANDOVER.chat;

  match note {
    Note::Handed(handoff) => {
      let path = handoff.path.display().to_string();
      let block = fill_marks(&chat.handed, &[("path", &path), ("note", handoff.text.trim_end())]);

      format!("{}\n{block}", chat.handed_marker)
    }
    Note::Carried(transcript) => {
      let block = fill_marks(&chat.carried, &[("note", transcript.trim_end())]);

      format!("{}\n{block}", chat.carried_marker)
    }
    Note::Lost => format!("{}\n{}", chat.lost_marker, chat.lost),
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
type Turns = std::collections::HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>;

static UNDER_WAY: std::sync::LazyLock<std::sync::Mutex<Turns>> = std::sync::LazyLock::new(Default::default);

/// An agent's handoff being asked for, held so a second asking for the same agent waits in line behind it, however
/// long it takes. Dropped, it lets the next asking go.
pub struct Asking {
  _held: tokio::sync::OwnedMutexGuard<()>,
}

impl Asking {
  /// The agent's turn to ask, once every asking for it ahead of this one has ended.
  pub async fn begin(agent: &str) -> Self {
    let line = UNDER_WAY
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
      .entry(agent.to_owned())
      .or_default()
      .clone();

    Asking {
      _held: line.lock_owned().await,
    }
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

/// The last asking of one kind for an agent: when it was made, and how many have come in a row since within the
/// handover policy's span (docs/unified-bmo.md, 「Replacing the session」).
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct Last {
  at: i64,
  times: u32,
}

fn last_key(agent: &str, kind: &str) -> String {
  format!("{agent}_last_{kind}")
}

/// How many askings of `kind` for an agent have come in a row within the span of the last one made, this one
/// counted; none where this one comes outside it, which is then written down as the last made. `moves` has an
/// asking within the span count as made too, so the span runs from it.
pub async fn in_a_row(receiver: &Receiver, agent: &str, kind: &str, moves: bool) -> Option<u32> {
  let db = receiver.db();
  let key = last_key(agent, kind);
  let now = now_ms();
  let span = HANDOVER.again_minutes * 60_000;
  let last = crate::store::settings::get(db, &key)
    .await
    .ok()
    .flatten()
    .and_then(|held| serde_json::from_str::<Last>(&held).ok());
  let (next, counted) = match last {
    Some(last) if now - last.at < span => {
      let times = last.times + 1;

      (
        Last {
          at: if moves { now } else { last.at },
          times,
        },
        Some(times),
      )
    }
    _ => (Last { at: now, times: 1 }, None),
  };

  if let Err(why) =
    crate::store::settings::set(db, &key, &serde_json::to_string(&next).unwrap_or_default()).await
  {
    eprintln!("{agent}: the last {kind} was not written down: {}", why.detail);
  }

  counted
}

/// The handoff kept for an agent's next fresh session, where its file still stands, without taking it.
pub async fn kept(receiver: &Receiver, agent: &str) -> Option<Handoff> {
  let path = crate::store::settings::get(receiver.db(), &pending_key(agent)).await.ok().flatten()?;
  let text = std::fs::read_to_string(&path).ok().filter(|text| !text.trim().is_empty())?;

  Some(Handoff {
    path: PathBuf::from(path),
    text,
  })
}

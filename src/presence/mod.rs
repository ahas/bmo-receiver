//! Who is writing in a room now and who is reachable now (docs/receiver.md, 「Typing and presence」): the typing
//! marks that stand, by room and by who writes, and the presence each source told, by its source, held in memory
//! alone and told on the event stream per room. Nothing here is filed, wakes anybody or reaches a turn.
//!
//! This module holds and tells; what is taken from a chat service, the console and a round, and the rooms a person
//! stands in, are read by `taken`.

pub mod taken;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use bmo_chat_client::ChatPresence;
use serde_json::{Value, json};

use crate::kept::Kept;
use crate::receiver::News;

/// Who writes or is reachable, as the stream names them.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Who {
  /// A console account, and a chat service's person the web unit links to it.
  Account(i64),
  /// A chat service's person linked to no account, by the service's key and the person's number.
  Person { via: String, person: i64 },
  /// The room's answerer, by the agent's registration name; the stream names it by the turn's speaker.
  Answerer(String),
}

impl Who {
  /// How the stream names it.
  pub fn shown(&self) -> Value {
    match self {
      Self::Account(account) => json!({ "kind": "account", "account": account }),
      Self::Person { via, person } => json!({ "kind": "person", "via": via, "personId": person }),
      Self::Answerer(_) => json!({ "kind": "answerer" }),
    }
  }

  // The part of a standing event's key that names who it is of.
  fn key(&self) -> String {
    match self {
      Self::Account(account) => format!("account:{account}"),
      Self::Person { via, person } => format!("person:{via}:{person}"),
      Self::Answerer(agent) => format!("answerer:{agent}"),
    }
  }

  /// Whether a message kept under `sender`, read as `linked`, is this one's own.
  pub fn spoke(&self, sender: i64, linked: &Who) -> bool {
    match self {
      Self::Person { person, .. } => *person == sender,
      Self::Account(_) => self == linked,
      Self::Answerer(_) => false,
    }
  }
}

/// Where a presence was told from: a chat service about one of its people, or the console about an account.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
  Provider { via: String, person: i64 },
  Console(i64),
}

/// One typing mark as it is told: the room, who writes, the chat service that told it where one did, whether it
/// is on, how long it stands unless told again (none for one that stands until told off), and the speaker the
/// stream names the room's turns under, for the answerer's.
#[derive(Clone, Debug)]
pub struct Mark {
  pub room: i64,
  pub who: Who,
  pub via: Option<String>,
  pub on: bool,
  pub lasts: Option<Duration>,
  pub speaker: Option<Value>,
}

// A mark that stands: the service that told it, its speaker, and the count that tells it from a later telling.
struct Standing {
  via: Option<String>,
  speaker: Option<Value>,
  told: u64,
}

// What a presence was last told as, and in which rooms.
struct Told {
  presence: ChatPresence,
  rooms: Vec<i64>,
}

#[derive(Default)]
struct State {
  marks: BTreeMap<(i64, Who), Standing>,
  tellings: u64,
  sources: BTreeMap<Source, (Who, ChatPresence)>,
  told: BTreeMap<Who, Told>,
  // The chat services whose readiness is being looked at.
  watched: BTreeSet<String>,
}

/// The present, held for the whole process. Cloned freely: every clone is the one present.
#[derive(Clone)]
pub struct Present {
  state: Arc<Mutex<State>>,
  // The account each chat service's person is linked to, or none, kept for the policy's span.
  links: Kept<i64, Option<i64>>,
  // How often a chat service whose marks or presence stand is asked whether it is ready.
  look: Duration,
}

impl Present {
  /// Nothing standing; a person's link kept for `link_span`, and a chat service's readiness looked at every
  /// `look`.
  pub fn new(link_span: Duration, look: Duration) -> Self {
    Self {
      state: Arc::default(),
      links: Kept::new(link_span),
      look: look.max(Duration::from_millis(1)),
    }
  }

  fn state(&self) -> MutexGuard<'_, State> {
    self.state.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// How often a chat service whose marks or presence stand is asked whether it is ready.
  pub fn look(&self) -> Duration {
    self.look
  }

  /// The account a person was last read as linked to, while its span lasts.
  pub fn link(&self, person: i64) -> Option<Option<i64>> {
    self.links.get(&person)
  }

  /// The account a person is linked to, or none, kept for the span.
  pub fn keep_link(&self, person: i64, account: Option<i64>) {
    self.links.keep(person, account);
  }

  /// A mark told on, standing for its life from now, or told off, ended where it stood. A mark with a life ends by
  /// itself once the life runs out without its being told again.
  pub fn mark(&self, news: Arc<dyn News>, mark: Mark) {
    let key = (mark.room, mark.who.clone());

    if !mark.on {
      self.end_mark(news.as_ref(), &key, None);

      return;
    }

    let told = {
      let mut state = self.state();

      state.tellings += 1;

      let told = state.tellings;

      state.marks.insert(
        key.clone(),
        Standing {
          via: mark.via.clone(),
          speaker: mark.speaker.clone(),
          told,
        },
      );

      told
    };

    news.tell_present(&mark_key(&key), &typing_event(&key, mark.speaker.as_ref(), true, mark.lasts));

    // The life counts from the telling, not from when the task that ends it first runs.
    if let Some(lasts) = mark.lasts {
      let present = self.clone();
      let ends = tokio::time::Instant::now() + lasts;

      tokio::spawn(async move {
        tokio::time::sleep_until(ends).await;
        present.end_mark(news.as_ref(), &key, Some(told));
      });
    }
  }

  // A standing mark ended and told off once; where `told` names a telling, only while that telling stands, so a
  // renewal outlives the life of the telling before it.
  fn end_mark(&self, news: &dyn News, key: &(i64, Who), told: Option<u64>) {
    let ended = {
      let mut state = self.state();
      let current = state
        .marks
        .get(key)
        .is_some_and(|standing| told.is_none_or(|told| standing.told == told));

      if current { state.marks.remove(key) } else { None }
    };

    if let Some(standing) = ended {
      news.end_present(&mark_key(key), &typing_event(key, standing.speaker.as_ref(), false, None));
    }
  }

  /// Who has a mark standing in a room.
  pub fn writing_in(&self, room: i64) -> Vec<Who> {
    self
      .state()
      .marks
      .keys()
      .filter(|(at, _)| *at == room)
      .map(|(_, who)| who.clone())
      .collect()
  }

  /// The mark of `who` in a room ended, as a message of theirs kept there ends it.
  pub fn spoke(&self, news: &dyn News, room: i64, who: &Who) {
    self.end_mark(news, &(room, who.clone()), None);
  }

  /// A source's word on a presence: `presence` held as what `source` says of `who`, or none taking its word back.
  /// The presence gathered of `who` from every source is told in `rooms`, the rooms it stands in, where it moved;
  /// where `rooms` is none, in the rooms it was told in last.
  pub fn told(
    &self,
    news: &dyn News,
    source: Source,
    who: Who,
    presence: Option<ChatPresence>,
    rooms: Option<Vec<i64>>,
  ) {
    let mut state = self.state();
    let before = match presence {
      Some(presence) => state.sources.insert(source, (who.clone(), presence)),
      None => state.sources.remove(&source),
    };

    // A source read as another subject than before leaves the one it was read as.
    if let Some((was, _)) = before
      && was != who
    {
      gather(&mut state, news, &was, None);
    }

    gather(&mut state, news, &who, rooms);
  }

  /// The people a chat service told the presence of, each with who they were read as.
  pub fn told_by(&self, via: &str) -> Vec<(i64, Who)> {
    self
      .state()
      .sources
      .iter()
      .filter_map(|(source, (who, _))| match source {
        Source::Provider { via: by, person } if by == via => Some((*person, who.clone())),
        _ => None,
      })
      .collect()
  }

  /// Whether a look at the chat service's readiness is to start: true once, until the look stops.
  pub fn watch(&self, via: &str) -> bool {
    self.state().watched.insert(via.to_owned())
  }

  /// Whether the look at a chat service goes on: while anything it told stands. A look that ends lets the service
  /// go, under the same lock, so the next thing it tells starts another.
  pub fn still_watched(&self, via: &str) -> bool {
    let mut state = self.state();
    let stands = state.marks.values().any(|standing| standing.via.as_deref() == Some(via))
      || state
        .sources
        .keys()
        .any(|source| matches!(source, Source::Provider { via: by, .. } if by == via));

    if !stands {
      state.watched.remove(via);
    }

    stands
  }

  /// A chat service that stopped being ready: every mark it told ended, every presence it told taken back, each
  /// told in its rooms, and the look at it let go.
  pub fn not_ready(&self, news: &dyn News, via: &str) {
    let marks: Vec<(i64, Who)> = self
      .state()
      .marks
      .iter()
      .filter(|(_, standing)| standing.via.as_deref() == Some(via))
      .map(|(key, _)| key.clone())
      .collect();

    for key in marks {
      self.end_mark(news, &key, None);
    }

    let mut state = self.state();
    let sources: Vec<Source> = state
      .sources
      .keys()
      .filter(|source| matches!(source, Source::Provider { via: by, .. } if by == via))
      .cloned()
      .collect();

    for source in sources {
      if let Some((who, _)) = state.sources.remove(&source) {
        gather(&mut state, news, &who, None);
      }
    }

    state.watched.remove(via);
  }
}

// The presence of `who` gathered from every source and told in its rooms where it moved: the rooms given, or the
// rooms it was told in last; a presence that became unknown told as null where it was told.
fn gather(state: &mut State, news: &dyn News, who: &Who, rooms: Option<Vec<i64>>) {
  let gathered = state
    .sources
    .values()
    .filter(|(subject, _)| subject == who)
    .map(|(_, presence)| *presence)
    // Online over away over offline, as the states are ordered, and the latest moment any source gives.
    .reduce(|one, other| ChatPresence {
      state: one.state.min(other.state),
      last_seen_at: one.last_seen_at.max(other.last_seen_at),
    });
  let was = state.told.remove(who);
  let mut rooms = rooms.unwrap_or_else(|| was.as_ref().map(|told| told.rooms.clone()).unwrap_or_default());

  rooms.sort_unstable();
  rooms.dedup();

  let Some(presence) = gathered.filter(|_| !rooms.is_empty()) else {
    for room in was.map(|told| told.rooms).unwrap_or_default() {
      news.end_present(&presence_key(room, who), &presence_event(room, who, None));
    }

    return;
  };

  if let Some(was) = &was {
    for room in was.rooms.iter().filter(|room| !rooms.contains(room)) {
      news.end_present(&presence_key(*room, who), &presence_event(*room, who, None));
    }
  }

  let moved = was.as_ref().is_none_or(|was| was.presence != presence);

  for room in &rooms {
    if moved || was.as_ref().is_some_and(|was| !was.rooms.contains(room)) {
      news.tell_present(&presence_key(*room, who), &presence_event(*room, who, Some(presence)));
    }
  }

  state.told.insert(who.clone(), Told { presence, rooms });
}

fn mark_key((room, who): &(i64, Who)) -> String {
  format!("typing:{room}:{}", who.key())
}

fn presence_key(room: i64, who: &Who) -> String {
  format!("presence:{room}:{}", who.key())
}

/// A typing mark as the stream tells it: its room, who writes, whether on, and its life in milliseconds, null for
/// a mark that stands until told off; the answerer's under the speaker the room's turns are told under.
pub fn typing_event(
  (room, who): &(i64, Who),
  speaker: Option<&Value>,
  on: bool,
  lasts: Option<Duration>,
) -> Value {
  let mut event = json!({
    "type": "typing",
    "chatId": room,
    "messageId": null,
    "who": who.shown(),
    "on": on,
    "lastsMs": lasts.map(|lasts| u64::try_from(lasts.as_millis()).unwrap_or(u64::MAX)),
  });

  if let Some(speaker) = speaker {
    event["speaker"] = speaker.clone();
  }

  event
}

/// A presence as the stream tells it in one room: who, the state, null where unknown, and the last moment seen.
pub fn presence_event(room: i64, who: &Who, presence: Option<ChatPresence>) -> Value {
  json!({
    "type": "presence",
    "chatId": room,
    "messageId": null,
    "who": who.shown(),
    "state": presence.map(|presence| presence.state),
    "lastSeenAt": presence.and_then(|presence| presence.last_seen_at),
  })
}

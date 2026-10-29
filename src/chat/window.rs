//! The one window every turn of the chat agent's session passes (docs/bmo-window.md): what a turn brought
//! selected over for its core — the speakers' sheet heads, the room's due memories and the rows its events wake,
//! with a user's own thread in their private room — each entry wrapped to go to a session in full once per version,
//! and beside it the brain's index with its hint. The window hands the gate the
//! turn's envelope and reads back what attaches.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;

use crate::js::{self, bun_hash};
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::fill_marks;
use regex::Regex;
use serde::Serialize;

use receiver_ports::gate::{Entry, Envelope, RoomKind, Situation, Source, TurnEvent};

use super::thread::{ThreadMark, thread_entry};
use crate::Fault;
use crate::now_ms;
use crate::receiver::{Inbound, Receiver, Room, rows};

// The wrapping a document rides in until the session it goes to is settled; it opens with a private-use
// character no document is written in, and never leaves this process.
pub const BLOCK_OPEN: &str = "\u{e000}REFERENCE-ONCE ";
pub const BLOCK_CLOSE: &str = "\u{e000}END REFERENCE-ONCE";

/// What a turn brought, which its reference is selected over.
pub struct Material<'a> {
  /// The agent whose turn it is, whose gates' switches and memories the selection reads.
  pub agent: &'a str,
  pub chat_id: i64,
  /// What the room's road declares of it: whether it is the owner's chat, and who BMO is there.
  pub room: Room,
  pub messages: &'a [Inbound],
  /// The quiet instructions, as the session reads them.
  pub instructions: &'a [String],
  /// Anything else the turn brought in words.
  pub said: &'a [String],
  pub events: &'a [TurnEvent],
  /// The room's recent messages in the room shape.
  pub recent: &'a [String],
  /// What the round already holds, by path, where the words are pushed into its working turn.
  pub holds: Option<&'a HashSet<String>>,
  pub round: Option<String>,
  /// The session the turn goes on, whose loads count read entries out of the index.
  pub session: Option<&'a str>,
}

/// A turn's selection: the entries it carries whole, what the gate answered and a person's thread; the block they
/// go in as; the index block and its counts; and what the turn's log is told of them.
#[derive(Clone, Debug, Default)]
pub struct Selection {
  pub entries: Vec<Entry>,
  pub index: String,
  pub counts: Vec<(Group, usize)>,
  pub block: String,
  pub context: Vec<(String, String)>,
  /// Pieces that stand as held marks alone, by path and what they hold: a person's thread once its session has
  /// every line of it.
  pub held: Vec<(String, Option<String>)>,
  /// The thread line this turn hands, kept under the session once the turn is delivered.
  pub thread_mark: Option<ThreadMark>,
}

/// A group of the brain's index, as a turn names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Group {
  SystemDocuments,
  RoomRules,
  Knowledge,
  People,
  RoomMemories,
}

impl Group {
  /// Every group, in the order a turn names them.
  pub const ALL: [Group; 5] = [
    Self::SystemDocuments,
    Self::RoomRules,
    Self::Knowledge,
    Self::People,
    Self::RoomMemories,
  ];

  /// The group an entry the gate answers stands in, by where it was read from.
  pub fn of(source: Source) -> Self {
    match source {
      Source::Document => Self::SystemDocuments,
      Source::Rule | Source::Situation => Self::RoomRules,
      Source::Knowledge => Self::Knowledge,
      Source::Person => Self::People,
      Source::Memory | Source::Room | Source::Archive => Self::RoomMemories,
    }
  }

  /// The group the gate's index names by its word, or none for a word it does not hold.
  pub fn named(word: &str) -> Option<Self> {
    match word {
      "system-documents" => Some(Self::SystemDocuments),
      "room-rules" => Some(Self::RoomRules),
      "knowledge" => Some(Self::Knowledge),
      "people" => Some(Self::People),
      "room-memories" => Some(Self::RoomMemories),
      _ => None,
    }
  }

  /// What the index calls it.
  pub fn word(self) -> &'static str {
    let said = &CHAT_ROUND.index;

    match self {
      Self::SystemDocuments => &said.system_documents,
      Self::RoomRules => &said.room_rules,
      Self::Knowledge => &said.knowledge,
      Self::People => &said.people,
      Self::RoomMemories => &said.room_memories,
    }
  }
}

/// How many of the entries a turn was offered, by group, the session has not read at the version offered: an entry
/// whose path the session's loads hold at the same version is read.
pub fn unread(entries: &[Entry], loaded: &HashMap<String, String>) -> Vec<(Group, usize)> {
  Group::ALL
    .iter()
    .map(|group| {
      let count = entries
        .iter()
        .filter(|entry| Group::of(entry.source) == *group)
        .filter(|entry| loaded.get(&entry.path) != Some(&bun_hash(&entry.text).to_string()))
        .count();

      (*group, count)
    })
    .collect()
}

/// The index line's words for each group, one a group with its unread count, each with what changed lately where
/// the gate gives a line of it; the words join into one line under the head.
pub fn group_words(counts: &[(Group, usize)], hints: &HashMap<Group, String>) -> Vec<String> {
  let said = &CHAT_ROUND.index;

  counts
    .iter()
    .map(|(group, count)| match hints.get(group).filter(|hint| !hint.trim().is_empty()) {
      Some(hint) => fill_marks(
        &said.group_hint,
        &[
          ("group", group.word()),
          ("count", &count.to_string()),
          ("hint", hint.trim().trim_end_matches('.')),
        ],
      ),
      None => fill_marks(&said.group, &[("group", group.word()), ("count", &count.to_string())]),
    })
    .collect()
}

/// The gate's index of a room for a session read into counts and hints, a group at a time in the turn's order; none
/// where the gate serves no index or does not answer.
pub async fn gate_index(
  receiver: &Receiver,
  chat_id: i64,
  session: Option<&str>,
  agent: &str,
) -> Option<(Vec<(Group, usize)>, HashMap<Group, String>)> {
  let groups = match receiver.units().gate.index(chat_id, session, Some(agent)).await {
    Ok(Some(groups)) => groups,
    Ok(None) => return None,
    Err(why) => {
      eprintln!("bmo-receiver: chat {chat_id} the brain's index could not be read: {why}");
      return None;
    }
  };
  let mut counts = vec![];
  let mut hints = HashMap::new();

  for group in Group::ALL {
    let Some(read) = groups.iter().find(|one| Group::named(&one.group) == Some(group)) else {
      continue;
    };

    counts.push((group, read.unread as usize));

    if let Some(hint) = read.hint.as_ref().filter(|hint| !hint.trim().is_empty()) {
      hints.insert(group, hint.clone());
    }
  }

  Some((counts, hints))
}

/// The index a turn carries: one line of the head, a group's words with what changed lately where the gate says,
/// and the hint; the hint alone where there is no group to count.
pub fn index_block(counts: &[(Group, usize)], hints: &HashMap<Group, String>) -> String {
  let said = &CHAT_ROUND.index;

  if counts.is_empty() {
    return said.hint.clone();
  }

  format!("{} {}. {}", said.head, group_words(counts, hints).join(" · "), said.hint)
}

#[derive(Serialize)]
struct BlockHead<'a> {
  path: &'a str,
  chat: i64,
  person: Option<i64>,
  version: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  holds: Option<&'a str>,
}

static PERSON_BOOK: LazyLock<[Regex; 3]> = LazyLock::new(|| {
  [
    Regex::new(r"^people/(\d+)/(?:sheet\.md|chunks/c\d+-[a-z0-9-]+\.md)$").expect("a person book sheet"),
    Regex::new(r"^rooms/-?\d+/people/(\d+)\.md$").expect("a room's sheet of a person"),
    Regex::new(r"^rooms/-?\d+/people/chunks/p(\d+)-[a-z0-9-]+\.md$").expect("a room's piece of a person"),
  ]
});

/// The user a document of the person book is about, read off its path into the brain; nobody for any other.
pub fn person_book_owner(path: &str) -> Option<i64> {
  let path = Path::new(path);

  if path.is_absolute() {
    return None;
  }

  let relative = path.to_string_lossy().replace('\\', "/");

  PERSON_BOOK
    .iter()
    .find_map(|pattern| pattern.captures(&relative))
    .and_then(|found| found[1].parse().ok())
}

/// A document wrapped to go to a session once per version: its head names it, its room, whose it is, which
/// version of it this is and, where it is named, what it holds.
pub fn once_block(chat: i64, path: &str, text: &str, holds: Option<&str>) -> String {
  let head = BlockHead {
    path,
    chat,
    person: person_book_owner(path),
    version: bun_hash(text).to_string(),
    holds,
  };

  format!(
    "{BLOCK_OPEN}{}\n[{path}]\n{text}\n{BLOCK_CLOSE}",
    serde_json::to_string(&head).unwrap_or_default()
  )
}

/// Whether anybody's words came into a turn rather than the system's own lines alone.
pub fn people_spoke(room: Room, messages: &[Inbound], said: &[String]) -> bool {
  messages.iter().any(|message| message.sender_id != room.identity) || !said.is_empty()
}

/// The line a turn's message is handed to the pickers as.
fn message_line(sender_name: &str, sender_id: Option<i64>, text: &str) -> String {
  format!(
    "{sender_name} ({}): {text}",
    sender_id.map_or_else(|| "unknown".to_owned(), |id| id.to_string()),
  )
}

/// A quiet instruction as one of the turn's lines.
fn instruction_line(text: &str) -> String {
  format!("quiet instruction from the system: {text}")
}

// The message a turn's messages follow on from: one of them replies to it, or it is the message the room's line
// puts just before the turn's first.
async fn followed_on(
  receiver: &Receiver,
  chat_id: i64,
  message_ids: &[i64],
) -> Result<Option<Inbound>, Fault> {
  for message_id in message_ids {
    let Some(row) = rows::message(receiver.db(), chat_id, *message_id, "").await? else {
      continue;
    };
    let Some(target) = row.reply_to_message_id.filter(|target| !message_ids.contains(target)) else {
      continue;
    };

    if let Some(found) = rows::message(receiver.db(), chat_id, target, "").await? {
      return Ok(Some(found));
    }
  }

  let Some(first) = message_ids.first() else { return Ok(None) };
  let recent = rows::recent_messages(receiver.db(), chat_id, CHAT_ROUND.recent_messages, "").await?;
  let Some(at) = recent.iter().position(|one| one.message_id == *first) else {
    return Ok(None);
  };

  Ok(at.checked_sub(1).and_then(|before| recent.get(before)).cloned())
}

/// The line of the message a turn's messages follow on from, its closing words alone where it is long, or none
/// where it follows on from nothing the archive holds.
async fn answered_line(
  receiver: &Receiver,
  chat_id: i64,
  message_ids: &[i64],
) -> Result<Option<String>, Fault> {
  let Some(said) = followed_on(receiver, chat_id, message_ids).await? else {
    return Ok(None);
  };
  let keep = CHAT_ROUND.reference.answered_chars;
  let characters = js::chars(&said.text);
  let text = if characters.len() > keep {
    format!("\u{2026}{}", js::text(&characters[characters.len() - keep..]))
  } else {
    said.text
  };

  Ok(Some(message_line(&said.sender_name, said.sender_id, &text)))
}

/// What this turn is handed. A selection that fails costs the turn its reference and nothing else.
pub async fn select(receiver: &Receiver, material: &Material<'_>) -> Result<Selection, Fault> {
  let chat_id = material.chat_id;
  let speakers: Vec<i64> = material
    .messages
    .iter()
    .filter_map(|message| message.sender_id)
    .filter(|id| Some(*id) != material.room.identity)
    .collect();
  let instruction_lines: Vec<String> =
    material.instructions.iter().map(|text| instruction_line(text)).collect();
  let mut lines: Vec<String> = material
    .messages
    .iter()
    .map(|message| message_line(&message.sender_name, message.sender_id, &message.text))
    .collect();

  lines.extend(instruction_lines.iter().cloned());
  lines.extend(material.said.iter().cloned());

  let message_id = material.messages.last().map(|message| message.message_id);
  let spoke = people_spoke(material.room, material.messages, material.said);
  let answered = if spoke && !material.messages.is_empty() {
    let ids: Vec<i64> = material.messages.iter().map(|message| message.message_id).collect();

    answered_line(receiver, chat_id, &ids).await.ok().flatten()
  } else {
    None
  };
  let mut situation_lines: Vec<String> = material.recent.to_vec();

  situation_lines.extend(instruction_lines);
  situation_lines.extend(material.said.iter().cloned());

  let envelope = Envelope {
    room_id: chat_id,
    message_id,
    lines,
    speaker_ids: speakers,
    situation: Some(Situation {
      agent_name: material.agent.to_owned(),
      room_kind: if material.room.owner_room { RoomKind::Owner } else { RoomKind::Group },
      events: material.events.to_vec(),
      now_at: now_ms(),
    }),
    holds: material.holds.map(|holds| holds.iter().map(String::clone).collect()),
    round: material.round.clone(),
    people_spoke: spoke,
    answered,
    records: true,
  };
  let entries = match receiver.units().gate.reference(&envelope).await {
    Ok(entries) => entries,
    Err(why) => {
      eprintln!("bmo-receiver: the reference selection failed: {why}");
      vec![]
    }
  };
  // The entries are what attaches without a pick, the turn's core, and go in whole with a person's thread; the agent
  // reads anything more through the index. The counts are the gate's index for the session, a piece it already
  // loaded counted out; where the gate serves none, they are counted over the selection's entries, with no hint.
  let (counts, hints) = match gate_index(receiver, chat_id, material.session, material.agent).await {
    Some(read) => read,
    None => {
      let loaded = super::context::loaded_in(receiver, material.session).await?;

      (unread(&entries, &loaded), HashMap::new())
    }
  };
  let mut core = entries;
  let mut held = Vec::new();
  let mut thread_mark = None;

  match thread_entry(receiver, chat_id).await {
    Ok(Some(thread)) => {
      if let Some(entry) = thread.entry {
        core.push(entry);
      } else {
        held.push((thread.path, Some(thread.holds)));
      }

      thread_mark = thread.mark;
    }
    Ok(None) => {}
    Err(why) => eprintln!("bmo-receiver: the private-room thread could not be read: {why}"),
  }

  let mut selection = made(chat_id, core);

  selection.index = index_block(&counts, &hints);
  selection.counts = counts;
  selection.held = held;
  selection.thread_mark = thread_mark;

  Ok(selection)
}

// The turn's core, each entry wrapped to go to the session once per version.
fn made(chat_id: i64, entries: Vec<Entry>) -> Selection {
  if entries.is_empty() {
    return Selection::default();
  }

  let blocks = &CHAT_ROUND.blocks;
  let mut parts = vec![fill_marks(&blocks.reference, &[("count", &entries.len().to_string())])];

  for entry in &entries {
    parts.push(once_block(chat_id, &entry.path, &entry.text, core_holds(entry).as_deref()));
  }

  parts.push(blocks.reference_end.clone());

  Selection {
    context: entries.iter().map(|entry| (source_name(entry), entry.path.clone())).collect(),
    entries,
    block: parts.join("\n"),
    ..Selection::default()
  }
}

/// What a piece of the turn's core holds: the name its maker gives it, else the policy's word for its sort
/// (docs/drafting.md, 「The round log」).
pub fn core_holds(entry: &Entry) -> Option<String> {
  entry
    .holds
    .clone()
    .or_else(|| CHAT_ROUND.reference.holds.get(&source_name(entry)).cloned())
}

/// What the turn's log calls where an entry came from.
pub fn source_name(entry: &Entry) -> String {
  serde_json::to_value(entry.source)
    .ok()
    .and_then(|value| value.as_str().map(str::to_owned))
    .unwrap_or_default()
}

/// A turn as the window composes it: the receiver's own block and the index, then what the turn is for.
pub fn composed(selection: &Selection, body: &str) -> String {
  [selection.block.as_str(), selection.index.as_str(), body]
    .iter()
    .filter(|part| !part.is_empty())
    .copied()
    .collect::<Vec<&str>>()
    .join("\n\n")
}

/// The index a new session's first turn walks once (docs/unified-bmo.md, 「Replacing the session」): every room the
/// agent's one session has answered in, the turn's own among them, each by its number and title with its groups'
/// unread counts where they are known — the turn's own room's read off the turn's own index line — and the words
/// for a room whose counts the gate does not give yet; then the index's hint.
pub async fn walk(
  receiver: &Receiver,
  own: i64,
  own_index: Option<&str>,
  session: Option<&str>,
  agent: &str,
) -> Result<String, Fault> {
  let said = &CHAT_ROUND.index;
  let mut rooms: Vec<i64> =
    crate::all(receiver.db(), r#"SELECT "chat_id" FROM "bmo_rooms" ORDER BY "chat_id""#, vec![])
      .await?
      .iter()
      .filter_map(|row| row.try_get::<i64>("", "chat_id").ok())
      .collect();

  if !rooms.contains(&own) {
    rooms.insert(0, own);
  }

  let own_words: Option<String> = own_index.and_then(|text| {
    let rest = text.split_once(&said.head)?.1;
    let words = rest.split_once(&said.hint).map_or(rest, |(words, _)| words).trim();
    let words = words.trim_end_matches('.');

    (!words.is_empty()).then(|| words.to_owned())
  });
  let mut lines = vec![said.walk_head.clone()];

  for chat in rooms {
    let title = rows::chat_title(receiver.db(), chat).await?.unwrap_or_default();
    let room = match title.is_empty() {
      true => fill_marks(&said.walk_untitled, &[("chat", &chat.to_string())]),
      false => fill_marks(&said.walk_room, &[("chat", &chat.to_string()), ("title", &title)]),
    };
    let words = match gate_index(receiver, chat, session, agent).await {
      Some((counts, hints)) if !counts.is_empty() => Some(group_words(&counts, &hints).join(" · ")),
      _ if chat == own => own_words.clone(),
      _ => None,
    };

    lines.push(match words {
      Some(words) => format!("{room} {words}"),
      None => format!("{room} {}", said.walk_unknown),
    });
  }

  lines.push(said.hint.clone());

  Ok(lines.join("\n"))
}

/// A first turn's prompt with the walk in place of the turn's own index block, where it carries one, and after
/// everything else otherwise: the brain comes after the room's words (docs/unified-bmo.md, 「Replacing the
/// session」).
pub async fn walked(
  receiver: &Receiver,
  own: i64,
  prompt: &str,
  session: Option<&str>,
  agent: &str,
) -> Result<String, Fault> {
  let said = &CHAT_ROUND.index;
  let span = prompt.find(&said.head).map(|start| {
    let end = prompt[start..].find('\n').map_or(prompt.len(), |at| start + at);

    (start, end)
  });
  let own_index = span.map(|(start, end)| &prompt[start..end]);
  let walk = walk(receiver, own, own_index, session, agent).await?;

  Ok(match span {
    Some((start, end)) => format!("{}{walk}{}", &prompt[..start], &prompt[end..]),
    None => format!("{prompt}\n\n{walk}"),
  })
}

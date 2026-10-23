//! The one window every turn of the chat agent's session passes (docs/bmo-window.md): what a turn brought
//! selected over for its reference — the room's memories, the matcher's rules, the rows its events wake and the
//! gates' picks, with a user's own thread in their private room — and each document wrapped to go to a session in
//! full once per version, passing the sanitiser on its way in. The window hands the gate the turn's envelope and
//! reads back the pieces it chooses.

use std::collections::HashSet;
use std::path::Path;
use std::sync::LazyLock;

use crate::js::{self, bun_hash};
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::fill_marks;
use regex::Regex;
use serde::Serialize;

use receiver_ports::agency::Agent;
use receiver_ports::gate::{Entry, Envelope, RoomKind, Sanitise, Situation, TurnEvent};

use super::thread::thread_entry;
use crate::Fault;
use crate::now_ms;
use crate::receiver::{Inbound, Receiver, Room, rows};

// The gates that pick a turn's reference, each under the agent's own switch.
const REFERENCE_GATES: [&str; 5] = [
  "system-documents",
  "room-rules",
  "library-knowledge",
  "people-search",
  "situation-rules",
];

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
}

/// A turn's selection: the entries, the block they go in as, and what the turn's log is told of them.
#[derive(Clone, Debug, Default)]
pub struct Selection {
  pub entries: Vec<Entry>,
  pub block: String,
  pub context: Vec<(String, String)>,
}

#[derive(Serialize)]
struct BlockHead<'a> {
  path: &'a str,
  chat: i64,
  person: Option<i64>,
  version: String,
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

/// A document wrapped to go to a session once per version: its head names it, its room, whose it is and which
/// version of it this is.
pub fn once_block(chat: i64, path: &str, original: &str, text: &str) -> String {
  let head = BlockHead {
    path,
    chat,
    person: person_book_owner(path),
    version: bun_hash(original).to_string(),
  };

  format!(
    "{BLOCK_OPEN}{}\n[{path}]\n{text}\n{BLOCK_CLOSE}",
    serde_json::to_string(&head).unwrap_or_default()
  )
}

/// Whether the agent's own registration leaves one of its gates switched off.
fn switched_off(agent: &Agent, gate: &str) -> bool {
  match agent.row.get("gates_off") {
    Some(off) => off.as_array().is_some_and(|off| off.iter().any(|one| one.as_str() == Some(gate))),
    None => false,
  }
}

/// Whether a turn asks one of its agent's gates, by the agent's own switch.
pub async fn gate_on(receiver: &Receiver, agent: &str, gate: &str) -> bool {
  let listed = receiver.agents().await.ok();

  match listed.as_ref().and_then(|listed| listed.agents.iter().find(|one| one.name == agent)) {
    Some(agent) => !switched_off(agent, gate),
    None => true,
  }
}

/// A text on its way into the session's context, with any order to attack a real target defused, where the
/// agent's own switch leaves the sanitiser on.
pub async fn clean(
  receiver: &Receiver,
  agent: &str,
  text: String,
  label: String,
  chat_id: i64,
  message_id: Option<i64>,
) -> Result<String, Fault> {
  if !gate_on(receiver, agent, "sanitizer").await {
    return Ok(text);
  }

  let asked = Sanitise {
    text: text.to_owned(),
    label: label.to_owned(),
    room_id: Some(chat_id),
    message_id,
  };
  let cleaned = receiver
    .units()
    .gate
    .sanitise(&asked)
    .await
    .map_err(|why| Fault::new(format!("the sanitiser: {why}")))?;

  if cleaned.changed {
    println!("context chat {chat_id}: security-intrusion material was defused in {label}");
  }

  Ok(cleaned.text)
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

  let mut off_gates = Vec::new();

  for gate in REFERENCE_GATES {
    if !gate_on(receiver, material.agent, gate).await {
      off_gates.push(gate.to_owned());
    }
  }

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
    off_gates,
    records: true,
  };
  let mut entries = match receiver.units().gate.reference(&envelope).await {
    Ok(entries) => entries,
    Err(why) => {
      eprintln!("bmo-receiver: the reference selection failed: {why}");
      vec![]
    }
  };

  match thread_entry(receiver, chat_id).await {
    Ok(Some(thread)) => entries.push(thread),
    Ok(None) => {}
    Err(why) => eprintln!("bmo-receiver: the private-room thread could not be read: {why}"),
  }

  made(receiver, material.agent, chat_id, message_id, entries).await
}

// The documents go in wrapped, each passing the sanitiser first, since the room's own documents are written
// out of what was said in it.
async fn made(
  receiver: &Receiver,
  agent: &str,
  chat_id: i64,
  message_id: Option<i64>,
  entries: Vec<Entry>,
) -> Result<Selection, Fault> {
  if entries.is_empty() {
    return Ok(Selection::default());
  }

  let blocks = &CHAT_ROUND.blocks;
  let mut parts = vec![fill_marks(&blocks.reference, &[("count", &entries.len().to_string())])];

  for entry in &entries {
    let label = fill_marks(&CHAT_ROUND.labels.reference, &[("path", &entry.path)]);
    let text = clean(receiver, agent, entry.text.clone(), label, chat_id, message_id).await?;

    parts.push(once_block(chat_id, &entry.path, &entry.text, &text));
  }

  parts.push(blocks.reference_end.clone());

  Ok(Selection {
    context: entries.iter().map(|entry| (source_name(entry), entry.path.clone())).collect(),
    entries,
    block: parts.join("\n"),
  })
}

/// What the turn's log calls where an entry came from.
pub fn source_name(entry: &Entry) -> String {
  serde_json::to_value(entry.source)
    .ok()
    .and_then(|value| value.as_str().map(str::to_owned))
    .unwrap_or_default()
}

/// A turn as the window composes it: the selection, then what the turn is for.
pub fn composed(selection: &Selection, body: &str) -> String {
  if selection.block.is_empty() {
    body.to_owned()
  } else {
    format!("{}\n\n{body}", selection.block)
  }
}

/// The words a push puts into a working turn, with what they wake that the round does not already hold, whole
/// rather than wrapped.
pub async fn pushed(
  receiver: &Receiver,
  agent: &str,
  selection: &Selection,
  holds: &HashSet<String>,
  paragraph: &str,
  chat_id: i64,
  message_id: Option<i64>,
) -> Result<String, Fault> {
  let fresh: Vec<&Entry> = selection.entries.iter().filter(|entry| !holds.contains(&entry.path)).collect();

  if fresh.is_empty() {
    return Ok(paragraph.to_owned());
  }

  let blocks = &CHAT_ROUND.blocks;
  let mut parts = vec![fill_marks(&blocks.reference_pushed, &[("count", &fresh.len().to_string())])];

  for entry in fresh {
    let label = fill_marks(&CHAT_ROUND.labels.reference, &[("path", &entry.path)]);

    parts.push(format!("[{}]", entry.path));
    parts.push(clean(receiver, agent, entry.text.clone(), label, chat_id, message_id).await?);
  }

  parts.push(blocks.reference_end.clone());
  parts.push(String::new());
  parts.push(paragraph.to_owned());

  Ok(parts.join("\n"))
}

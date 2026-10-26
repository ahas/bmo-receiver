//! Every session concerned told to read the documents named again (docs/bmo-window.md, 「Document reload」): the
//! agency publishes `documents-moved` when a reload names documents, and this tells the sessions the receiver
//! concerns by a riding instruction for each room, folded into the one still waiting there. A chat told within
//! the last hour gets the documents with its next reload; one whose reload says `now` is told at once, and which
//! rooms were told when is the receiver's record (its reload tables).
//!
//! A document in the brain is runtime data: it keeps its absolute path and is told from wherever this runs. A
//! document that ships with the code is named relative to its tree.

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use futures_util::future::BoxFuture;
use receiver_ports::agency::{Event, Scope};
use serde::Deserialize;

use sea_orm::DatabaseConnection;

use crate::chat::conversation::unified_session;
use crate::chat::instructions::{Queued, queue};
use crate::hearing::Hearer;
use crate::receiver::Receiver;
use crate::{Fault, all, now_ms, one, run};

/// What `bmo reload` takes, as its refusal of a call naming no document says.
pub const RELOAD_USAGE: &str = "usage: bmo reload --path <file> [--path <file>...] [--now]";

// The tree's document directories a session relies on, relative to its root.
const DOCUMENT_DIRS: [&str; 4] = ["docs/lore", "prompts/persona", "prompts/rules", "prompts/agents"];

// The rulebooks and the chat agent's own documents a chat session is given as its system prompt.
const CHAT_DOCUMENTS: [&str; 5] = [
  "rules/answer-content.md",
  "rules/answer-shape.md",
  "rules/plain-writing.md",
  "rules/agent-words.md",
  "rules/self-preservation.md",
];
const CHAT_SYSTEM_DOCUMENTS: [&str; 2] = ["agents/chat/context.md", "agents/chat/manual.md"];

// The brain's own document directories, relative to its root.
const ROOMS_ROOT: &str = "rooms";
const KNOWLEDGE_ROOT: &str = "knowledge";
const PEOPLE_ROOT: &str = "people";

const INLINE_FILE_CAP: usize = 6_000;
const INLINE_TOTAL_CAP: usize = 20_000;

/// How often a chat is told about changed documents at most, in seconds; what changes meanwhile gathers.
pub const RELOAD_COOLDOWN_SECONDS: i64 = 3_600;

const RELOAD_HEAD: &str = "Documents you rely on were updated";

// A reload notice's kind, beside the agent it is for: the one row a room holds of it for each agent at a time.
const RELOAD_KIND: &str = "reload";

// The family a reload row is listed under.
const RELOAD_SORT: &str = "document_reload";

/// A path made whole without the file system: `.` dropped and `..` taking its parent away, as Node resolves.
pub fn lexical(path: &Path) -> PathBuf {
  let mut whole = PathBuf::new();

  for part in path.components() {
    match part {
      Component::CurDir => {}
      Component::ParentDir => {
        whole.pop();
      }
      other => {
        whole.push(other.as_os_str());
      }
    }
  }

  whole
}

/// Where a reload reads shipped documents out of, where the deployment names that tree, and where the brain's
/// documents are.
#[derive(Clone, Debug)]
pub struct Places {
  pub tree: Option<PathBuf>,
  pub brain: PathBuf,
}

impl Places {
  // The path relative to the brain root, for a whole path that lies inside the brain and is not the root.
  fn brain_relative(&self, path: &str) -> Option<String> {
    let path = Path::new(path);

    if !path.is_absolute() {
      return None;
    }

    let inside = lexical(path).strip_prefix(lexical(&self.brain)).ok()?.to_path_buf();

    (!inside.as_os_str().is_empty()).then(|| inside.to_string_lossy().into_owned())
  }

  /// The path a document is named by: the brain's whole, a tree's relative to its root. A whole path into a
  /// checkout names the same document the tree holds under that name.
  pub fn named(&self, value: &str) -> String {
    let whole = lexical(Path::new(value));
    let whole_text = whole.to_string_lossy().into_owned();

    if self.brain_relative(&whole_text).is_some() {
      return whole_text;
    }

    if !Path::new(value).is_absolute() {
      return lexical(Path::new(value)).to_string_lossy().into_owned();
    }

    let Some(tree) = self.tree.as_deref().map(lexical) else {
      return whole_text;
    };

    if let Ok(inside) = whole.strip_prefix(&tree) {
      return inside.to_string_lossy().into_owned();
    }

    pathdiff(&whole, &tree)
  }

  /// The file a named document is read from: a whole path as it stands, a tree's under the tree where one is
  /// named; none for a tree's document where none is.
  pub fn file(&self, path: &str) -> Option<PathBuf> {
    if Path::new(path).is_absolute() {
      return Some(PathBuf::from(path));
    }

    self.tree.as_ref().map(|tree| tree.join(path))
  }
}

// One path relative to another, both whole, as Node's `relative` words it.
fn pathdiff(path: &Path, base: &Path) -> String {
  let ours: Vec<_> = path.components().collect();
  let theirs: Vec<_> = base.components().collect();
  let shared = ours.iter().zip(&theirs).take_while(|(one, other)| one == other).count();
  let mut out = PathBuf::new();

  for _ in shared..theirs.len() {
    out.push("..");
  }

  for part in &ours[shared..] {
    out.push(part.as_os_str());
  }

  out.to_string_lossy().into_owned()
}

/// Of the repository-relative paths named, the documents a session relies on that ship with the code: these go
/// live with the deployment and nowhere earlier.
pub fn tree_documents(paths: &[String]) -> Vec<String> {
  paths
    .iter()
    .filter(|path| {
      path.ends_with(".md") && DOCUMENT_DIRS.iter().any(|dir| path.starts_with(&format!("{dir}/")))
    })
    .cloned()
    .collect()
}

// What a tree document is classified by: the container every prompt document stands in dropped first.
fn document_parts(path: &str) -> Vec<&str> {
  let parts: Vec<&str> = path.split('/').collect();

  if parts.first() == Some(&"prompts") { parts[1..].to_vec() } else { parts }
}

fn stem(file: &str) -> &str {
  file.strip_suffix(".md").unwrap_or(file)
}

/// Whether a document is a rulebook or one of the agents' own documents.
pub fn is_agent_document(path: &str) -> bool {
  matches!(document_parts(path).first(), Some(&"rules" | &"agents"))
}

/// Whether a document is part of a chat session's turn rather than its system prompt or something read with a
/// tool: the session carries its new text on its next turn without being told.
pub fn is_chat_turn_document(path: &str) -> bool {
  let joined = document_parts(path).join("/");

  joined.starts_with("persona/")
    || joined == "agents/chat/owner.md"
    || joined == "agents/chat/rules.md"
    || joined == "agents/chat/capabilities.md"
}

/// Whether a document is part of a chat session's own system prompt.
pub fn is_chat_prompt_document(path: &str) -> bool {
  if is_chat_turn_document(path) {
    return false;
  }

  let parts = document_parts(path);
  let joined = parts.join("/");

  CHAT_SYSTEM_DOCUMENTS.contains(&joined.as_str()) || CHAT_DOCUMENTS.contains(&joined.as_str())
}

/// Whether a document is a reference document, attached in full with the next turn that needs it once per
/// session (docs/bmo-window.md, 「Loaded once per session」).
pub fn loads_once(places: &Places, path: &str) -> bool {
  if let Some(inside) = places.brain_relative(path) {
    let parts: Vec<&str> = inside.split('/').collect();
    let markdown = inside.ends_with(".md");

    return markdown
      && match parts.as_slice() {
        [KNOWLEDGE_ROOT, _] => true,
        [ROOMS_ROOT, chat, kind, _, ..] => {
          let digits = chat.strip_prefix('-').unwrap_or(chat);

          !digits.is_empty()
            && digits.bytes().all(|byte| byte.is_ascii_digit())
            && matches!(*kind, "rules" | "people")
        }
        [PEOPLE_ROOT, person, _, ..] => {
          !person.is_empty() && person.bytes().all(|byte| byte.is_ascii_digit())
        }
        _ => false,
      };
  }

  let path = path.replace('\\', "/");
  let parts: Vec<&str> = path.split('/').collect();

  path == "docs/lore/index.md"
    || path == "docs/reference-index.md"
    || matches!(parts.as_slice(), ["docs", "lore", "chunks", file] if file.ends_with(".md"))
}

/// The instruction a session gets: the changed files, each with its current text, so the session takes them as
/// given without opening them. It rides a round opened for something else, so it asks for no answer of its own.
pub fn reload_instruction(places: &Places, paths: &[String]) -> String {
  let mut lines = vec![format!("{RELOAD_HEAD} ({}):", paths.len())];

  lines.extend(paths.iter().map(|path| format!("- {path}")));
  lines.extend(
    [
      "",
      "Whatever you remember of these documents is stale. The text below is their current version: it outranks your memory of them, and where the two disagree the text below wins. Take it as given from here on, and where a file is only named, open it with the Read tool when you next need it.",
      "Nothing about this needs saying in the room: answer whatever else this turn carries as you would without it, and where it carries nothing else, output exactly <<SKIP>>.",
      "",
      "DOCUMENTS:",
    ]
      .map(str::to_owned),
  );

  let mut budget = INLINE_TOTAL_CAP;

  for path in paths {
    if loads_once(places, path) {
      lines.push(
        format!(
          "[{path}] (a reference document: its current version is attached in full with the next turn that needs it)"
        ),
      );
      continue;
    }

    let Some(file) = places.file(path) else {
      lines.push(format!(
        "[{path}] (its current text is in a tree this service does not read; open it when you next need it)"
      ));
      continue;
    };
    let text = std::fs::read_to_string(&file).map(|whole| whole.trim().to_owned()).unwrap_or_default();

    if text.is_empty() {
      lines.push(format!("[{path}] (removed or empty)"));
      continue;
    }

    let length = text.chars().count();

    if length > INLINE_FILE_CAP || length > budget {
      let at = if Path::new(path).is_absolute() {
        String::new()
      } else {
        format!(" at {}", lexical(&file).display())
      };

      lines.push(format!("[{path}] (too long to include here; read the file{at})"));
      continue;
    }

    budget -= length;
    lines.push(format!("[{path}]"));
    lines.push(text.to_owned());
  }

  lines.push("END OF DOCUMENTS".to_owned());
  lines.join("\n")
}

/// The files a reload notice names, read back out of its text.
pub fn reload_paths_of(instruction: &str) -> Vec<String> {
  let mut lines = instruction.split('\n');

  if !lines.next().is_some_and(|head| head.starts_with(RELOAD_HEAD)) {
    return vec![];
  }

  lines
    .take_while(|line| !line.is_empty())
    .filter_map(|line| line.strip_prefix("- ").map(str::to_owned))
    .collect()
}

/// The rooms holding a session, which alone have a memory of a document to go stale.
pub async fn session_rooms(receiver: &Receiver, agent: &str) -> Vec<i64> {
  if unified_session(receiver, agent).await {
    let rows = all(receiver.db(), r#"SELECT "chat_id" FROM "bmo_rooms" ORDER BY "chat_id""#, vec![])
      .await
      .unwrap_or_default();

    return rows.iter().map(|row| row.try_get::<i64>("", "chat_id").unwrap_or_default()).collect();
  }

  receiver
    .units()
    .agency
    .sessions()
    .await
    .unwrap_or_default()
    .iter()
    .filter(|session| session.agent == agent)
    .filter_map(|session| match session.scope {
      Scope::Room { room } => Some(room),
      _ => None,
    })
    .collect()
}

// The rooms a person's words stand in, which are the rooms whose session reads their book.
async fn sender_rooms(receiver: &Receiver, sender: i64) -> Vec<i64> {
  all(
    receiver.db(),
    r#"SELECT DISTINCT "chat_id" FROM "messages" WHERE "sender_id" = $1"#,
    vec![sender.into()],
  )
  .await
  .unwrap_or_default()
  .iter()
  .map(|row| row.try_get::<i64>("", "chat_id").unwrap_or_default())
  .collect()
}

/// Which rooms a changed document concerns. Only rooms holding a session are named.
pub async fn chats_concerned(receiver: &Receiver, places: &Places, path: &str) -> Vec<i64> {
  let sessions = session_rooms(receiver, &receiver.deployment().agent).await;
  let only =
    |chats: Vec<i64>| -> Vec<i64> { chats.into_iter().filter(|chat| sessions.contains(chat)).collect() };

  if let Some(inside) = places.brain_relative(path) {
    let parts: Vec<&str> = inside.split('/').collect();
    let named = stem(parts.last().copied().unwrap_or_default());

    return match parts.as_slice() {
      [ROOMS_ROOT, chat, ..] => only(chat.parse::<i64>().into_iter().collect()),
      [KNOWLEDGE_ROOT, ..] if named == "index" || parts.len() != 2 => vec![],
      [KNOWLEDGE_ROOT, _] => sessions.clone(),
      [PEOPLE_ROOT, person, ..] => {
        let digits = !person.is_empty() && person.bytes().all(|byte| byte.is_ascii_digit());

        if !digits || parts.len() < 3 || (parts.len() == 3 && named == "index") {
          vec![]
        } else {
          only(sender_rooms(receiver, person.parse().unwrap_or_default()).await)
        }
      }
      _ => vec![],
    };
  }

  let parts = document_parts(path);
  let named = stem(parts.last().copied().unwrap_or_default());

  match parts.as_slice() {
    ["docs", "lore", ..] => sessions.clone(),
    ["persona", "rooms", _, ..] => only(named.parse::<i64>().into_iter().collect()),
    ["persona", ..] => sessions.clone(),
    ["agents", "chat", ..] if named == "rules" => {
      sessions.iter().filter(|chat| **chat != receiver.owner_room()).copied().collect()
    }
    ["agents", "chat", ..] if named == "owner" => only(vec![receiver.owner_room()]),
    ["agents", "chat", ..] => sessions.clone(),
    _ if CHAT_DOCUMENTS.contains(&parts.join("/").as_str()) => sessions.clone(),
    _ => vec![],
  }
}

/// The files a reload notice names, folded into the one still waiting in the room for the agent, and the row
/// written for the agent's session there: it rides the next round of any room its session answers, for good,
/// rather than waking one. Where the room's gathering is kept, the moment it was told is written.
async fn tell_chat(
  receiver: &Receiver,
  places: &Places,
  agent: &str,
  chat_id: i64,
  paths: &[String],
  now: i64,
  gathered: bool,
) -> Result<(), Fault> {
  let db = receiver.db();
  let kind = format!("{RELOAD_KIND}:{agent}");
  let waiting = one(
    db,
    r#"SELECT "text" FROM "instructions" WHERE "chat_id" = $1 AND "kind" = $2 AND "delivered_at" IS NULL AND "handed_at" IS NULL ORDER BY "id" DESC LIMIT 1"#,
    vec![chat_id.into(), kind.clone().into()],
  )
    .await
    .unwrap_or(None);
  let every: Vec<String> = match waiting {
    Some(row) => reload_paths_of(&row.try_get::<String>("", "text").unwrap_or_default()),
    None => vec![],
  };
  let mut every = every.into_iter().chain(paths.iter().cloned()).collect::<Vec<String>>();

  every = distinct_paths(&every);

  let text = reload_instruction(places, &every);
  let queued = Queued {
    kind: Some(&kind),
    sort: Some(RELOAD_SORT),
    rides_session: true,
    ..Queued::default()
  };

  queue(db, agent, chat_id, &text, &queued).await.map(drop)?;

  if !gathered {
    return Ok(());
  }

  run(
    db,
    r#"INSERT INTO "reload_sent" ("chat_id", "at") VALUES ($1, $2) ON CONFLICT ("chat_id") DO UPDATE SET "at" = excluded."at""#,
    vec![chat_id.into(), now.into()],
  )
    .await
    .map(drop)
}

fn distinct_paths(paths: &[String]) -> Vec<String> {
  let mut seen: HashSet<String> = HashSet::new();
  let mut kept: Vec<String> = vec![];

  for path in paths {
    if seen.insert(path.clone()) {
      kept.push(path.clone());
    }
  }

  kept
}

/// The files a room has waiting on its next reload, given up by the room that is told.
async fn take_pending(db: &DatabaseConnection, chat_id: i64) -> Vec<String> {
  let rows = all(
    db,
    r#"SELECT "path" FROM "reload_pending" WHERE "chat_id" = $1 ORDER BY "path""#,
    vec![chat_id.into()],
  )
  .await
  .unwrap_or_default();

  let _ = run(db, r#"DELETE FROM "reload_pending" WHERE "chat_id" = $1"#, vec![chat_id.into()]).await;

  rows.iter().map(|row| row.try_get::<String>("", "path").unwrap_or_default()).collect()
}

async fn last_told(db: &DatabaseConnection, chat_id: i64) -> Option<i64> {
  one(db, r#"SELECT "at" FROM "reload_sent" WHERE "chat_id" = $1"#, vec![chat_id.into()])
    .await
    .unwrap_or(None)
    .map(|row| row.try_get::<i64>("", "at").unwrap_or_default())
}

/// Every room whose gathered changes have waited out the cooldown told them, each session once for a path two
/// of its rooms both had waiting; the rooms told are answered. `now` is in epoch milliseconds.
pub async fn flush_due(receiver: &Receiver, places: &Places, now: i64) -> Vec<i64> {
  let db = receiver.db();
  let waiting: Vec<i64> =
    all(db, r#"SELECT DISTINCT "chat_id" FROM "reload_pending" ORDER BY "chat_id""#, vec![])
      .await
      .unwrap_or_default()
      .iter()
      .map(|row| row.try_get::<i64>("", "chat_id").unwrap_or_default())
      .collect();
  let mut told_session: HashMap<String, HashSet<String>> = HashMap::new();
  let mut told: Vec<i64> = vec![];

  for chat_id in waiting {
    let last = last_told(db, chat_id).await;

    if last.is_some_and(|last| now - last < RELOAD_COOLDOWN_SECONDS * 1_000) {
      continue;
    }

    let already = told_session.entry(session_key(receiver, chat_id).await).or_default();
    let paths: Vec<String> = take_pending(db, chat_id)
      .await
      .into_iter()
      .filter(|path| already.insert(path.clone()))
      .collect();

    if !paths.is_empty() {
      let _ = tell_chat(receiver, places, &receiver.deployment().agent, chat_id, &paths, now, true).await;
      told.push(chat_id);
    }
  }

  told
}

// The session a room's turns go to: the agent's own where one session is shared across rooms, the room's else.
async fn session_key(receiver: &Receiver, chat_id: i64) -> String {
  if unified_session(receiver, &receiver.deployment().agent).await {
    receiver.deployment().agent.clone()
  } else {
    format!("chat:{chat_id}")
  }
}

/// A reload as the receiver runs it: the documents named told to every session of the chat agent's they concern,
/// and what the reload came to said. `given` are as the call gave them; `now` is in epoch milliseconds.
pub async fn reload(
  receiver: &Receiver,
  places: &Places,
  given: &[String],
  now: i64,
  immediate: bool,
) -> Vec<String> {
  let paths: Vec<String> = given.iter().map(|value| places.named(value)).collect();

  if paths.is_empty() {
    return vec![RELOAD_USAGE.to_owned()];
  }

  let mut said: Vec<String> = vec![];
  let mut by_room: Vec<(i64, Vec<String>)> = vec![];

  for path in &paths {
    if is_chat_turn_document(path) {
      continue;
    }

    if is_chat_prompt_document(path) {
      said.push(format!(
        "reload: {path} is part of a session's own system prompt; each reads it on its own next turn"
      ));
      continue;
    }

    for chat_id in chats_concerned(receiver, places, path).await {
      gather(&mut by_room, chat_id, std::slice::from_ref(path));
    }
  }

  said.extend(tell_rooms(receiver, places, &receiver.deployment().agent, by_room, now, immediate).await);
  said
}

// The paths added to what a room has gathered, each once.
fn gather(by_room: &mut Vec<(i64, Vec<String>)>, chat_id: i64, paths: &[String]) {
  let at = match by_room.iter().position(|(chat, _)| *chat == chat_id) {
    Some(at) => at,
    None => {
      by_room.push((chat_id, vec![]));
      by_room.len() - 1
    }
  };
  let list = &mut by_room[at].1;

  for path in paths {
    if !list.contains(path) {
      list.push(path.clone());
    }
  }
}

/// What each room gathered told to the agent's session there, or kept for its next reload where the room was told
/// within the hour and the reload is not due at once; what came of it, said.
async fn tell_rooms(
  receiver: &Receiver,
  places: &Places,
  agent: &str,
  by_room: Vec<(i64, Vec<String>)>,
  now: i64,
  immediate: bool,
) -> Vec<String> {
  let mut said: Vec<String> = vec![];
  let mut told: Vec<i64> = vec![];
  // The reload tables keep a room's gathering for the chat agent's session alone: another agent's is told at once.
  let gathered = agent == receiver.deployment().agent;

  for (chat_id, list) in by_room {
    let last = last_told(receiver.db(), chat_id).await;

    if gathered && !immediate && last.is_some_and(|last| now - last < RELOAD_COOLDOWN_SECONDS * 1_000) {
      for path in &list {
        let _ = run(
          receiver.db(),
          r#"INSERT INTO "reload_pending" ("chat_id", "path") VALUES ($1, $2) ON CONFLICT DO NOTHING"#,
          vec![chat_id.into(), path.clone().into()],
        )
        .await;
      }

      said.push(format!(
        "reload: room {chat_id} was told recently; {} will follow with the next reload",
        list.join(", "),
      ));
      continue;
    }

    let mut every = if gathered { take_pending(receiver.db(), chat_id).await } else { vec![] };

    for path in &list {
      if !every.contains(path) {
        every.push(path.clone());
      }
    }

    every = distinct_paths(&every);

    let _ = tell_chat(receiver, places, agent, chat_id, &every, now, gathered).await;
    told.push(chat_id);
    said.push(format!(
      "reload: room {chat_id} told to re-read {}, with its next round",
      list.join(", ")
    ));
  }

  let waiting: Vec<i64> = all(
    receiver.db(),
    r#"SELECT DISTINCT "chat_id" FROM "reload_pending" ORDER BY "chat_id""#,
    vec![],
  )
  .await
  .unwrap_or_default()
  .iter()
  .map(|row| row.try_get::<i64>("", "chat_id").unwrap_or_default())
  .collect();

  if !told.is_empty() {
    said.insert(0, format!("reload: queued for {} room(s)", told.len()));
  }

  if !waiting.is_empty() {
    said.push(format!(
      "reload: {} room(s) were told within the hour and will get these with their next reload: {}",
      waiting.len(),
      waiting.iter().map(i64::to_string).collect::<Vec<_>>().join(", "),
    ));
  }

  if told.is_empty() && waiting.is_empty() {
    said.push("reload: no session holds these documents; nothing queued".to_owned());
  }

  said
}

/// The kind a reload naming documents is published under.
pub const DOCUMENTS_MOVED: &str = "document-move.created";

/// A reload as the agency publishes it: the paths, whether it is due at once, and where the caller named them, whose
/// documents they are and the rooms they concern.
#[derive(Debug, Deserialize)]
pub struct Moved {
  #[serde(default)]
  pub paths: Vec<String>,
  #[serde(default)]
  pub now: bool,
  #[serde(default)]
  pub agent: Option<String>,
  #[serde(default)]
  pub rooms: Vec<i64>,
}

/// The documents moved told to the sessions concerned, as the agency's `documents-moved` names them: where it names
/// rooms, the named agent's sessions in those rooms, the chat agent's where it names no agent, each room holding a
/// session of that agent told every path; where it names none, the chat agent's sessions each document concerns. A
/// reload naming an agent and rooms is the brain's, so a relative path it names stands under the brain root.
pub async fn documents_moved(receiver: &Receiver, places: &Places, moved: &Moved) -> Vec<String> {
  let now = now_ms();

  if moved.rooms.is_empty() {
    return reload(receiver, places, &moved.paths, now, moved.now).await;
  }

  let agent = moved.agent.clone().unwrap_or_else(|| receiver.deployment().agent.clone());
  let brains = moved.agent.is_some();
  let paths: Vec<String> = moved
    .paths
    .iter()
    .map(|path| {
      if brains && !Path::new(path).is_absolute() {
        places.named(&places.brain.join(path).to_string_lossy())
      } else {
        places.named(path)
      }
    })
    .collect();

  if paths.is_empty() {
    return vec![RELOAD_USAGE.to_owned()];
  }

  let sessions = session_rooms(receiver, &agent).await;
  let mut by_room: Vec<(i64, Vec<String>)> = vec![];

  for chat_id in moved.rooms.iter().filter(|room| sessions.contains(room)) {
    gather(&mut by_room, *chat_id, &paths);
  }

  tell_rooms(receiver, places, &agent, by_room, now, moved.now).await
}

/// The reloads as the service hears them on the agency's stream.
pub struct Reloads {
  receiver: Receiver,
  places: Places,
}

impl Reloads {
  pub fn new(receiver: Receiver, places: Places) -> Self {
    Reloads { receiver, places }
  }
}

impl Hearer for Reloads {
  fn kinds(&self) -> &'static [&'static str] {
    &[DOCUMENTS_MOVED]
  }

  fn hear<'a>(&'a self, event: &'a Event) -> BoxFuture<'a, ()> {
    Box::pin(async move {
      let moved = match serde_json::from_value::<Moved>(event.data.clone()) {
        Ok(moved) => moved,
        Err(why) => {
          eprintln!("bmo-receiver: a reload that does not read: {why}");
          return;
        }
      };

      for line in documents_moved(&self.receiver, &self.places, &moved).await {
        println!("{line}");
      }
    })
  }
}

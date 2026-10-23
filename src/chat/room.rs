//! The room's block a turn of BMO's chat carries ahead of its prompt (docs/unified-bmo.md, 「The system prompt
//! and the room switch」): which room this is and what it is called, the chat's document for its kind with the
//! roster in the owner's own chat, what the road the turn answers on does with marks, shows and sends, the
//! room's persona and what is in force in it — and the one line that restates it to a session holding this
//! version from earlier. Variable room instructions belong to a turn, never to the cached prefix, so the
//! prefix every room shares stays one.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use receiver_policy::chat_line::CHAT_LINE;
use receiver_policy::fill_marks;
use regex::Regex;
use serde_json::Value;

use crate::Fault;
use crate::js;
use crate::receiver::{Receiver, rows};
use crate::roads::{Facts, Road};
use crate::{all, now_ms};
use receiver_ports::agency::Agent;
use receiver_ports::web::GrantRow;

// The folder a room's documents are kept in, under the data root the configuration names.
pub struct Trees {
  pub data: PathBuf,
}

impl Trees {
  /// The trees of the receiver's own deployment.
  pub fn of(receiver: &Receiver) -> Self {
    Trees {
      data: receiver.data_root().path().to_path_buf(),
    }
  }

  /// The folder of one agent's own documents.
  pub fn agent_dir(&self, agent: &str) -> PathBuf {
    self.data.join("prompts").join(agent)
  }
}

/// A document the room's block cannot go out without, read whole and trimmed.
pub fn read_required(path: &Path) -> Result<String, Fault> {
  Ok(
    std::fs::read_to_string(path)
      .map_err(|why| Fault::new(format!("{}: {why}", path.display())))?
      .trim()
      .to_owned(),
  )
}

/// The room's block and the line that restates it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomContext {
  pub chat_id: i64,
  pub text: String,
  pub mark: String,
}

// A heading of the capability document naming the verb it is about and what that verb is called.
static CAPABILITY: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^CAPABILITY ([A-Za-z]+): (.+)$").expect("a capability heading"));

// The tone a room's persona file may open with.
static TONE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^tone:[ \t]*(\S+)[ \t]*(?:\n|$)").expect("a tone line"));

/// What the room is: the owner's own chat, or a group.
pub async fn room_kind(receiver: &Receiver, _owner_id: i64, chat_id: i64) -> Result<&'static str, Fault> {
  Ok(if receiver.roads().is_owner_room(chat_id).await { "owner" } else { "group" })
}

/// What BMO's own server says of itself for the chat line: the digest of what it shows, the verb each sending
/// tool needs and the record each recording tool leaves, as the tool host keeps them; none where the host does
/// not say.
pub struct ServerFacts {
  pub digest: Option<String>,
  pub sending: Vec<(String, String)>,
  pub records: Vec<(String, String)>,
}

/// What the receiver's own server says of itself for the chat line: the digest of what it shows, the verb each
/// sending tool needs and the record each recording tool leaves. The tools bound to this repository state them
/// beside themselves; where none stands, the room is told nothing.
pub async fn server_facts(receiver: &Receiver) -> ServerFacts {
  let _receiver = receiver;

  ServerFacts {
    digest: None,
    sending: vec![],
    records: vec![],
  }
}

// One section of the capability document: its verb, what the verb is called, and its text.
struct Section {
  verb: String,
  name: String,
  text: String,
}

// The capability document cut into its opening, which every room reads, and its sections.
fn capability_document(trees: &Trees) -> Result<(String, Vec<Section>), Fault> {
  let document = read_required(&trees.agent_dir("chat").join(&CHAT_LINE.capabilities.document))?;
  let mut opening = String::new();
  let mut sections: Vec<Section> = vec![];

  for line in document.split('\n') {
    match CAPABILITY.captures(line) {
      Some(mark) => sections.push(Section {
        verb: mark[1].to_owned(),
        name: mark[2].to_owned(),
        text: line.to_owned(),
      }),
      None => match sections.last_mut() {
        Some(open) => {
          open.text.push('\n');
          open.text.push_str(line);
        }
        None => {
          opening.push('\n');
          opening.push_str(line);
        }
      },
    }
  }

  for section in &mut sections {
    section.text = js::trim(&section.text).to_owned();
  }

  Ok((js::trim(&opening).to_owned(), sections))
}

fn joined(parts: Vec<String>, by: &str) -> String {
  parts.into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join(by)
}

// The sending tools of BMO's own server the road carries, and those the room's own road carries instead.
fn sending_tools(server: &ServerFacts, road: &Facts, home: Option<&Facts>) -> String {
  let said = &CHAT_LINE.capabilities;
  let carried: Vec<&str> = server
    .sending
    .iter()
    .filter(|(_, verb)| performs(road, verb))
    .map(|(name, _)| name.as_str())
    .collect();
  let handed: Vec<&str> = match home {
    Some(home) => server
      .sending
      .iter()
      .filter(|(_, verb)| !performs(road, verb) && performs(home, verb))
      .map(|(name, _)| name.as_str())
      .collect(),
    None => vec![],
  };
  let tools = if carried.is_empty() { said.sending_none.clone() } else { carried.join(", ") };

  joined(
    vec![
      fill_marks(&said.sending, &[("tools", &tools)]),
      match home {
        Some(home) if !handed.is_empty() => {
          fill_marks(&said.sending_handed, &[("key", &home.key), ("tools", &handed.join(", "))])
        }
        _ => String::new(),
      },
    ],
    " ",
  )
}

// Whether a road performs a chat verb, by the protocol's name for it.
fn performs(facts: &Facts, verb: &str) -> bool {
  facts.can.iter().any(|one| one == verb)
}

/// What a turn is told about what it can send: the sections the road performs, the rest named in one line,
/// and where the turn answers on another road than the room's own, what that road sends instead.
pub fn capability_context(
  trees: &Trees,
  server: &ServerFacts,
  road: &Facts,
  home: Option<&Facts>,
) -> Result<String, Fault> {
  let said = &CHAT_LINE.capabilities;
  let (opening, sections) = capability_document(trees)?;
  let named =
    |list: &[&Section]| list.iter().map(|section| section.name.as_str()).collect::<Vec<_>>().join("; ");
  let given: Vec<&Section> = sections.iter().filter(|section| performs(road, &section.verb)).collect();
  let missing: Vec<&Section> = sections.iter().filter(|section| !performs(road, &section.verb)).collect();
  let handed: Vec<&Section> = match home {
    Some(home) => missing.iter().copied().filter(|section| performs(home, &section.verb)).collect(),
    None => vec![],
  };
  let withheld = if missing.is_empty() {
    String::new()
  } else {
    joined(
      vec![
        fill_marks(&said.withheld, &[("names", &named(&missing))]),
        match home {
          Some(home) if !handed.is_empty() => {
            fill_marks(&said.handed, &[("key", &home.key), ("names", &named(&handed))])
          }
          _ => String::new(),
        },
      ],
      " ",
    )
  };
  let mut parts = vec![opening, sending_tools(server, road, home)];

  parts.extend(given.iter().map(|section| section.text.clone()));
  parts.push(withheld);

  Ok(joined(parts, "\n\n"))
}

// A persona file, trimmed, or nothing where there is none.
fn persona_file(path: &Path) -> String {
  std::fs::read_to_string(path).map(|text| js::trim(&text).to_owned()).unwrap_or_default()
}

fn persona_dir(trees: &Trees) -> PathBuf {
  trees.data.join("prompts").join("persona")
}

/// The room's tone layer and its own persona, each under its header: the tone its file opens with, `none` for
/// none, or the default.
pub fn room_persona(trees: &Trees, chat_id: i64) -> String {
  let said = &CHAT_LINE.persona;
  let raw = persona_file(&persona_dir(trees).join("rooms").join(format!("{chat_id}.md")));
  let (tone, text) = match TONE.captures(&raw) {
    Some(found) => {
      let tone = (&found[1] != "none").then(|| found[1].to_owned());

      (tone, js::trim(&raw[found[0].len()..]).to_owned())
    }
    None => (None, raw.clone()),
  };
  let tone = tone
    .map(|name| persona_file(&persona_dir(trees).join(format!("{name}.md"))))
    .unwrap_or_default();

  joined(
    vec![
      if tone.is_empty() { String::new() } else { format!("{}\n{tone}", said.tone_header) },
      if text.is_empty() { String::new() } else { format!("{}\n{text}", said.room_header) },
    ],
    "\n\n",
  )
}

#[derive(serde::Serialize, serde::Deserialize)]
struct RoomRule {
  id: String,
  subject: i64,
  text: String,
}

// The room rules document, off the account model: every rule standing in the room.
fn room_rules_document(data_root: &Path, chat_id: i64) -> Result<String, Fault> {
  let path = data_root.join("room-rules").join(format!("{chat_id}.json"));
  let rows: Vec<RoomRule> = match std::fs::read_to_string(&path) {
    Ok(text) => serde_json::from_str(&text)
      .map_err(|why| Fault::new(format!("{}: invalid room rules document: {why}", path.display())))?,
    Err(_) => vec![],
  };
  let rows = serde_json::to_string(&rows).map_err(|why| Fault::new(why.to_string()))?;

  Ok(fill_marks(&CHAT_LINE.rules.room_rules, &[("rows", &rows)]))
}

// The grants standing in the room, on the account model.
async fn grants_block(receiver: &Receiver, chat_id: i64) -> Result<String, Fault> {
  let said = &CHAT_LINE.rules;
  let rows: Vec<GrantRow> = receiver.units().web.room_grants(chat_id).await.unwrap_or_default();
  let now = now_ms();
  let mut lines = vec![];

  for row in rows {
    let relation: String = row.relation.clone();
    let subject: String = row.subject_key.clone();
    let expires: Option<i64> = row.expires_at;
    let words: Option<String> = row.words;

    if expires.is_some_and(|at| at <= now) {
      continue;
    }

    let user = subject
      .strip_prefix("user:")
      .filter(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()));
    let holder = match user {
      None => said.members.clone(),
      Some(user) => {
        let senders: Vec<String> = receiver
          .units()
          .web
          .senders_of_user(user)
          .await
          .unwrap_or_default()
          .iter()
          .map(|sender| sender.to_string())
          .collect();
        let senders = if senders.is_empty() { said.no_senders.clone() } else { senders.join(", ") };

        fill_marks(&said.user, &[("user", user), ("senders", &senders)])
      }
    };
    let until = match expires {
      Some(at) => {
        let at = chrono::DateTime::from_timestamp_millis(at)
          .unwrap_or_default()
          .format("%Y-%m-%dT%H:%M:%S%.3fZ");

        fill_marks(&said.until, &[("at", &at.to_string())])
      }
      None => String::new(),
    };
    let words = match words {
      Some(words) => {
        fill_marks(&said.words, &[("words", &serde_json::to_string(&words).unwrap_or_default())])
      }
      None => String::new(),
    };

    lines.push(fill_marks(
      &said.grant,
      &[("relation", &relation), ("holder", &holder), ("until", &until), ("words", &words)],
    ));
  }

  if lines.is_empty() {
    lines.push(said.nothing.clone());
  }

  let mut block = vec![said.grants_head.clone()];

  block.extend(lines);
  block.push(said.grants_end.clone());

  Ok(block.join("\n"))
}

/// What is in force in a room: the room rules document off the account model, the room's grants on it.
/// What is in force in a room: the room's grants on the account model where it stands, the room's rules document
/// where it does not.
pub async fn room_rules(receiver: &Receiver, chat_id: i64) -> Result<String, Fault> {
  if receiver.units().web.model_in_force().await.unwrap_or(false) {
    return grants_block(receiver, chat_id).await;
  }

  room_rules_document(receiver.data_root().path(), chat_id)
}

// Every name one agent answers to, the owner's own word for it first.
pub fn names_of(name: &str, label: Option<&str>, aliases: Option<&str>) -> String {
  let aliases: Vec<&str> = aliases
    .unwrap_or_default()
    .split(',')
    .map(str::trim)
    .filter(|one| !one.is_empty())
    .collect();
  let label = label.unwrap_or_default();
  let every: Vec<&str> = if label.is_empty() {
    aliases
  } else {
    std::iter::once(label).chain(aliases.into_iter().filter(|one| *one != label)).collect()
  };

  if every.is_empty() { name.to_owned() } else { every.join(", ") }
}

// The order names are compared in: case set aside first, the lower case ahead where only case differs.
pub fn name_order(left: &str, right: &str) -> std::cmp::Ordering {
  left.to_lowercase().cmp(&right.to_lowercase()).then_with(|| right.cmp(left))
}

/// The roster the owner's own chat is given: the one shared chat agent with the rooms it is connected to, then
/// each kind of agent with every agent of it under the names the owner calls it by.
pub async fn roster(receiver: &Receiver, owner_id: i64) -> Result<String, Fault> {
  let db = receiver.db();
  let words = &CHAT_LINE.room;
  let rooms: Vec<i64> = all(db, r#"SELECT "chat_id" FROM "bmo_rooms" ORDER BY "chat_id""#, vec![])
    .await?
    .iter()
    .filter_map(|row| row.try_get::<i64>("", "chat_id").ok())
    .collect();
  let mut named = vec![words.roster_owner_room.clone()];

  for room in rooms {
    if room == owner_id {
      continue;
    }

    if let Some(title) = rows::chat_title(db, room).await?
      && !title.is_empty()
    {
      named.push(title);
    }
  }

  let listed = receiver.units().agency.agents().await.ok();
  let agents: Vec<Agent> = listed.as_ref().map(|listed| listed.agents.clone()).unwrap_or_default();
  let mut lines = vec![format!(
    "- {}",
    fill_marks(&words.roster, &[("agent", &receiver.deployment().agent), ("rooms", &named.join(", "))],)
  )];

  let kinds: [(&str, String); 5] = [
    ("companion", words.kinds.companion.clone()),
    ("gate", words.kinds.gate.clone()),
    ("runner", words.kinds.runner.clone()),
    ("recollector", words.kinds.recollector.clone()),
    ("worker", words.kinds.worker.clone()),
  ];

  for (kind, line) in &kinds {
    let mut members: Vec<(bool, String, String)> = agents
      .iter()
      .filter(|one| {
        one.row.get("kind").and_then(Value::as_str).map(str::to_owned).unwrap_or_default() == *kind
      })
      .map(|one| {
        let aliases = one.row.get("aliases").and_then(Value::as_str).unwrap_or_default();
        let builtin = one.row.get("builtin").and_then(Value::as_i64).unwrap_or(0) == 1;
        let names = names_of(&one.name, one.label.as_deref(), Some(aliases));

        (builtin, one.name.clone(), names)
      })
      .collect();

    if members.is_empty() {
      continue;
    }

    members.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| name_order(&left.1, &right.1)));
    lines.push(format!("- {line}"));
    lines.extend(members.iter().map(|(_, name, names)| format!("  - {name}: {names}.")));
  }

  Ok(lines.join("\n"))
}

/// The room's block a turn in a room carries, answering on the road it was asked on where that road reaches
/// the room, and the line that restates it.
pub async fn room_context(
  receiver: &Receiver,
  owner_id: i64,
  chat_id: i64,
  via: Option<&str>,
) -> Result<RoomContext, Fault> {
  let trees = Trees::of(receiver);
  let said = &CHAT_LINE.room;
  let kind = room_kind(receiver, owner_id, chat_id).await?;
  let home_road = receiver.roads().room_road(chat_id).await.map(Road::Service).unwrap_or(Road::Record);
  let home = receiver.roads().facts(&home_road).await;
  let answering = receiver.roads().answering(chat_id, via).await.unwrap_or(home_road);
  let road = receiver.roads().facts(&answering).await;
  let elsewhere = (road.key != home.key).then_some(&home);
  let title = serde_json::to_string(&rows::chat_title(receiver.db(), chat_id).await?).unwrap_or_default();
  let server = server_facts(receiver).await;
  let mut parts = vec![
    fill_marks(&said.head, &[("chat", &chat_id.to_string()), ("kind", kind)]),
    fill_marks(&said.name, &[("title", &title)]),
    said.replaces.clone(),
  ];

  if kind == "group" {
    parts.push(read_required(&trees.agent_dir("chat").join(&said.group_document))?);
  } else {
    parts.push(read_required(&trees.agent_dir("chat").join(&said.owner_document))?);
    parts.push(roster(receiver, owner_id).await?);
  }

  parts.push(fill_marks(&said.writing, &[("writing", &road.writing)]));
  parts.push(
    road
      .shows
      .as_ref()
      .map(|shows| fill_marks(&said.shows, &[("shows", shows)]))
      .unwrap_or_default(),
  );
  parts.push(capability_context(&trees, &server, &road, elsewhere)?);
  parts.push(fill_marks(&said.road, &[("key", &road.key)]));
  parts.push(room_persona(&trees, chat_id));
  parts.push(room_rules(receiver, chat_id).await?);
  parts.push(said.end.clone());

  Ok(RoomContext {
    chat_id,
    text: joined(parts, "\n\n"),
    mark: fill_marks(&said.mark, &[("chat", &chat_id.to_string()), ("kind", kind), ("title", &title)]),
  })
}

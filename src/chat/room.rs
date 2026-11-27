//! The room's block a turn of BMO's chat carries ahead of its prompt (docs/unified-bmo.md, 「The system prompt
//! and the room switch」): which room this is and what it is called, the chat's document for its kind with the
//! roster in the owner's own chat, what the road the turn answers on does with marks, shows and sends, and
//! what is in force in it — and the one line that restates it to a session holding this
//! version from earlier. Variable room instructions belong to a turn, never to the cached prefix, so the
//! prefix every room shares stays one.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use receiver_policy::chat_line::CHAT_LINE;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::fill_marks;
use receiver_policy::handover::HANDOVER;
use receiver_policy::instruction::INSTRUCTION;
use receiver_policy::tool_host::TOOL_HOST;
use regex::Regex;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::Fault;
use crate::all;
use crate::js;
use crate::receiver::{Receiver, rows};
use crate::roads::{Facts, Road};
use receiver_ports::agency::Agent;

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

/// The room's block, the line that restates it, and the fingerprint of the block's rules for reading a turn,
/// which a session is replaced on where it changes (docs/unified-bmo.md, 「Replacing the session」).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomContext {
  pub chat_id: i64,
  pub text: String,
  pub mark: String,
  pub process: String,
}

// A heading of the capability document naming the verb it is about and what that verb is called.
static CAPABILITY: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^CAPABILITY ([A-Za-z]+): (.+)$").expect("a capability heading"));

/// What the room is: the owner's own chat, or a group.
pub async fn room_kind(receiver: &Receiver, chat_id: i64) -> Result<&'static str, Fault> {
  Ok(if receiver.roads().is_owner_room(chat_id).await { "owner" } else { "group" })
}

/// What BMO's own server says of itself for the chat line: the digest of what it shows and the record each
/// recording tool leaves, as the tool host keeps them; none where the host does not say.
pub struct ServerFacts {
  pub digest: Option<String>,
  pub records: Vec<(String, String)>,
}

/// What the receiver's own server says of itself for the chat line: the digest of what it shows and the record
/// each recording tool leaves. The tools bound to this repository state them beside themselves; where none
/// stands, the room is told nothing.
pub async fn server_facts(receiver: &Receiver) -> ServerFacts {
  let _receiver = receiver;

  ServerFacts {
    digest: None,
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

// Whether a road performs a chat verb, by the protocol's name for it.
fn performs(facts: &Facts, verb: &str) -> bool {
  facts.can.iter().any(|one| one == verb)
}

// The name a capability section stands under where it is about the fold rather than a verb: the protocol's name
// for a dialect that folds.
const FOLDABLE: &str = "foldable";

// Whether a road offers what a capability section is about: the verb it names, or the fold where the road's
// dialect folds.
fn offers(facts: &Facts, name: &str) -> bool {
  performs(facts, name) || (name == FOLDABLE && facts.foldable)
}

/// What a turn is told about what it can send: the sections the road performs, the rest named in one line,
/// and where the turn answers on another road than the room's own, what that road sends instead. The sending
/// entries are not named: the road judges every send itself, whichever entries the tool shows.
pub fn capability_context(trees: &Trees, road: &Facts, home: Option<&Facts>) -> Result<String, Fault> {
  let said = &CHAT_LINE.capabilities;
  let (opening, sections) = capability_document(trees)?;
  let named =
    |list: &[&Section]| list.iter().map(|section| section.name.as_str()).collect::<Vec<_>>().join("; ");
  let given: Vec<&Section> = sections.iter().filter(|section| offers(road, &section.verb)).collect();
  let missing: Vec<&Section> = sections.iter().filter(|section| !offers(road, &section.verb)).collect();
  let handed: Vec<&Section> = match home {
    // The fold is no verb another road carries for this one, so only a verb is handed.
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
  let mut parts = vec![opening];

  parts.extend(given.iter().map(|section| section.text.clone()));
  parts.push(withheld);

  Ok(joined(parts, "\n\n"))
}

/// What is in force in a room: the owner's room rules, every live `rule` memory of the agent's brain about the room,
/// one JSON row each (docs/unified-bmo.md, 「Delegations」). The block is complete, so an agent or a deployment
/// keeping no brain carries it empty, and a brain that does not answer carries the line saying nothing is
/// delegated until it does.
pub async fn room_rules(receiver: &Receiver, agent: &str, chat_id: i64) -> Result<String, Fault> {
  let said = &CHAT_LINE.rules;
  let read = match receiver.units().brain.as_ref() {
    Some(brain) => brain.room_rules(agent, chat_id).await,
    None => Ok(None),
  };
  let Ok(rules) = read else {
    return Ok(said.rules_down.clone());
  };
  let rows =
    serde_json::to_string(&rules.unwrap_or_default()).map_err(|why| Fault::new(why.to_string()))?;

  Ok(fill_marks(&said.room_rules, &[("rows", &rows)]))
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

/// The roster the owner's own chat is given: the default answerer with the rooms it is connected to, then the
/// other supervisors under the names the owner calls them by, and the roles work is handed off as.
pub async fn roster(receiver: &Receiver) -> Result<String, Fault> {
  let db = receiver.db();
  let owners = receiver.owner_room();
  let words = &CHAT_LINE.room;
  let rooms: Vec<i64> = all(db, r#"SELECT "chat_id" FROM "bmo_rooms" ORDER BY "chat_id""#, vec![])
    .await?
    .iter()
    .filter_map(|row| row.try_get::<i64>("", "chat_id").ok())
    .collect();
  let mut named = vec![words.roster_owner_room.clone()];

  for room in rooms {
    if room == owners {
      continue;
    }

    if let Some(title) = rows::chat_title(db, room).await?
      && !title.is_empty()
    {
      named.push(title);
    }
  }

  let listed = receiver.agents().await.ok();
  let agents: Vec<Agent> = listed.as_ref().map(|listed| listed.agents.clone()).unwrap_or_default();
  let mut lines = vec![format!(
    "- {}",
    fill_marks(&words.roster, &[("agent", &receiver.default_answerer()), ("rooms", &named.join(", "))],)
  )];

  let mut supervisors: Vec<(String, String)> = agents
    .iter()
    .filter(|one| one.name != receiver.default_answerer())
    .map(|one| {
      let aliases = one.row.get("aliases").and_then(Value::as_str).unwrap_or_default();

      (one.name.clone(), names_of(&one.name, one.label.as_deref(), Some(aliases)))
    })
    .collect();

  if !supervisors.is_empty() {
    supervisors.sort_by(|left, right| name_order(&left.0, &right.0));
    lines.push(format!("- {}", words.kinds.supervisor));
    lines.extend(supervisors.iter().map(|(name, names)| format!("  - {name}: {names}.")));
  }

  let roles = receiver.units().agency.roles().await.unwrap_or_default();

  if !roles.is_empty() {
    lines.push(format!("- {}", words.kinds.role));
    lines.extend(roles.iter().map(|role| {
      let label = names_of(&role.name, role.label.as_deref(), None);

      match role.description.as_deref().map(str::trim).filter(|said| !said.is_empty()) {
        Some(said) => format!("  - {}: {label} — {said}", role.name),
        None => format!("  - {}: {label}.", role.name),
      }
    }));
  }

  Ok(lines.join("\n"))
}

// How a message and an instruction are read and how each is answered, as the room's block carries them once
// (docs/instruction.md, 「What reaches an agent」): an instruction's own words ride its turn, so nothing here changes
// with a supervisor's chain or its stages' words.
fn reading_rules() -> String {
  [
    INSTRUCTION.message_rule.clone(),
    INSTRUCTION.instruction_rule.clone(),
    INSTRUCTION.answer_rule.clone(),
    fill_marks(&INSTRUCTION.arrived, &[("voice", &INSTRUCTION.voice)]),
  ]
  .join("\n")
}

// The situation-note instruction, as the room's block carries it (docs/instruction.md, 「The situation note」): the
// policy's text with its character ceiling.
fn situation_instruction() -> String {
  let said = &CHAT_ROUND.situation;

  fill_marks(&said.instruction, &[("ceiling", &said.ceiling_chars.to_string())])
}

/// The part of the room's block that says how a turn is read and answered, the same for every supervisor in every
/// room (docs/instruction.md, 「What reaches an agent」): the rules for a room's words, the rules for reading and
/// answering a message and an instruction, and the agent's first judgement, in the order the block carries them.
pub fn process_part() -> Vec<String> {
  vec![
    CHAT_LINE.room.process.clone(),
    reading_rules(),
    CHAT_ROUND.supervisor.structure.clone(),
    fill_marks(
      &CHAT_ROUND.supervisor.rule,
      &[("marker", &CHAT_ROUND.supervisor.marker), ("question", &CHAT_ROUND.supervisor.question)],
    ),
  ]
}

/// The fingerprint of the process's part of the block as its parts stand.
pub fn process_mark_of(parts: &[String]) -> String {
  format!("{:x}", Sha256::digest(parts.join("\n\n").as_bytes()))
}

/// The fingerprint of the process's part of the block as the policy sets it now.
pub fn process_mark() -> String {
  process_mark_of(&process_part())
}

/// The room's block a turn in a room carries, answering on the road it was asked on where that road reaches
/// the room, and the line that restates it.
pub async fn room_context(
  receiver: &Receiver,
  agent: &str,
  chat_id: i64,
  via: Option<&str>,
) -> Result<RoomContext, Fault> {
  let trees = Trees::of(receiver);
  let said = &CHAT_LINE.room;
  let kind = room_kind(receiver, chat_id).await?;
  let home_road = receiver.roads().room_road(chat_id).await.map(Road::Service).unwrap_or(Road::Record);
  let home = receiver.roads().facts(&home_road).await;
  let answering = receiver.roads().answering(chat_id, via).await.unwrap_or(home_road);
  let road = receiver.roads().facts(&answering).await;
  let elsewhere = (road.key != home.key).then_some(&home);
  let title = serde_json::to_string(&rows::chat_title(receiver.db(), chat_id).await?).unwrap_or_default();
  let process = process_part();
  let process_mark = process_mark_of(&process);
  let mut parts = vec![
    fill_marks(&said.head, &[("chat", &chat_id.to_string()), ("kind", kind)]),
    fill_marks(&said.name, &[("title", &title)]),
    said.replaces.clone(),
  ];

  parts.extend(process);
  parts.extend([situation_instruction(), CHAT_ROUND.closing.rule.clone()]);

  // The rules about the brain's index stand only where the brain reaches the supervisor's turns, as the
  // index itself does.
  if super::window::brain_reaches(receiver, agent).await {
    parts.extend([
      CHAT_ROUND.index.rule.clone(),
      fill_marks(&CHAT_ROUND.index.memories, &[("chat", &chat_id.to_string())]),
    ]);
  }

  parts.extend([
    CHAT_ROUND.queue_line.rule.clone(),
    fill_marks(&CHAT_ROUND.notice.rule, &[("marker", &CHAT_ROUND.notice.marker)]),
    fill_marks(&CHAT_ROUND.notice.optional_rule, &[("marker", &CHAT_ROUND.notice.optional_marker)]),
    fill_marks(&TOOL_HOST.rule, &[("marker", &TOOL_HOST.marker)]),
    fill_marks(&HANDOVER.chat.ask_rule, &[("marker", &HANDOVER.chat.ask_marker)]),
    fill_marks(&HANDOVER.chat.consolidate_rule, &[("marker", &HANDOVER.chat.consolidate_marker)]),
    fill_marks(
      &HANDOVER.chat.clean_rule,
      &[("marker", &HANDOVER.chat.clean_marker), ("handoff", &HANDOVER.chat.ask_marker)],
    ),
    fill_marks(&HANDOVER.chat.handed_rule, &[("marker", &HANDOVER.chat.handed_marker)]),
    fill_marks(&HANDOVER.chat.carried_rule, &[("marker", &HANDOVER.chat.carried_marker)]),
    fill_marks(&HANDOVER.chat.lost_rule, &[("marker", &HANDOVER.chat.lost_marker)]),
    fill_marks(&CHAT_ROUND.push.rule, &[("marker", &CHAT_ROUND.push.marker)]),
    fill_marks(&CHAT_ROUND.relay.rule, &[("marker", &CHAT_ROUND.relay.marker)]),
    fill_marks(&CHAT_ROUND.handed.rule, &[("marker", &CHAT_ROUND.handed.marker)]),
    fill_marks(&CHAT_ROUND.console.rule, &[("marker", &CHAT_ROUND.console.marker)]),
    fill_marks(&CHAT_ROUND.typed.rule, &[("marker", &CHAT_ROUND.typed.marker)]),
    CHAT_ROUND.voice.summary_rule_stated(),
    CHAT_ROUND.voice.reading_rule_stated(),
    fill_marks(
      &CHAT_ROUND.instructions.restarted_rule,
      &[("marker", &CHAT_ROUND.instructions.restarted_marker)],
    ),
    said.voice.clone(),
  ]);

  if kind == "group" {
    parts.push(read_required(&trees.agent_dir("chat").join(&said.group_document))?);
  } else {
    parts.push(read_required(&trees.agent_dir("chat").join(&said.owner_document))?);
    parts.push(roster(receiver).await?);
  }

  parts.push(fill_marks(&said.writing, &[("writing", &road.writing)]));
  parts.push(
    road
      .shows
      .as_ref()
      .map(|shows| fill_marks(&said.shows, &[("shows", shows)]))
      .unwrap_or_default(),
  );
  parts.push(capability_context(&trees, &road, elsewhere)?);
  parts.push(fill_marks(&said.road, &[("key", &road.key)]));
  parts.push(room_rules(receiver, agent, chat_id).await?);
  parts.push(said.end.clone());

  Ok(RoomContext {
    chat_id,
    text: joined(parts, "\n\n"),
    mark: fill_marks(&said.mark, &[("chat", &chat_id.to_string()), ("kind", kind), ("title", &title)]),
    process: process_mark,
  })
}

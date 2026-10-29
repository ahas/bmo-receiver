//! What a round of BMO's chat session is handed (docs/bmo-window.md, 「What every turn carries」): its scope and
//! header lines, the reference the window selects, the stretch of the room archived since the last reply, the
//! words a wait still owes and the new words, its quiet instructions cleaned, the agents' windows and the other
//! agents' work, and the lines that close it.

use std::collections::HashSet;

use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::fill_marks;
use serde::Serialize;

use super::conversation::unified_session;
use super::shape::Reading;
use super::window::{self, Material, Selection};
use crate::chat::consult::{consult_block, room_windows_block, tells_of_queue};
use crate::receiver::{Inbound, Receiver, Room, rows};
use crate::roads::Roads;
use crate::{Fault, now_ms, one};

use receiver_ports::gate::TurnEvent;

/// What a round is composed for: its room and what the road declares of it, the words it reads, its quiet
/// instructions with the rows and events they carry, and what the conversation knows of the round.
pub struct Round<'a> {
  /// The agent whose turn it is.
  pub agent: &'a str,
  pub chat_id: i64,
  pub room: Room,
  pub owner: i64,
  pub batch: &'a [Inbound],
  pub instructions: &'a [String],
  pub instruction_ids: &'a [i64],
  pub events: &'a [TurnEvent],
  /// The waiting window is over: the round must answer.
  pub force: bool,
  pub tools_allowed: bool,
  /// The messages of the batch a wait held over from an earlier round.
  pub waited_on: &'a HashSet<i64>,
  /// Whether this is the conversation's first asking, which carries what the session missed.
  pub first_call: bool,
  pub all_from_owner: bool,
  /// The session the round goes on in, which says whether a unified session has seen this room.
  pub session: Option<&'a str>,
  /// The room's words not yet taken, which the prompt does not show.
  pub pending: &'a HashSet<i64>,
  /// What waits in BMO's queue behind this turn.
  pub queue_line: &'a str,
  /// Why the session's memory was just replaced, where it was.
  pub woken: Option<&'a str>,
  /// The owner's reader, where a reply's picture may be fetched on demand.
  pub reader: Option<&'a Roads>,
  pub round: Option<String>,
  /// The turn carries the draft process's first question below its closing lines, and that question is not the
  /// process's last: the closing lines ask for its answer rather than the message (docs/drafting.md).
  pub questioned: bool,
}

/// The scope a round's turn is about.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Scope {
  pub chat_id: i64,
  pub message_ids: Vec<i64>,
  pub reference_message_ids: Vec<i64>,
  #[serde(skip)]
  pub instructions: String,
}

/// A round's prompt, with what it read and what it was handed.
pub struct Composed {
  pub prompt: String,
  pub recent: Vec<Inbound>,
  pub scope: Scope,
  pub context: Vec<(String, String)>,
  pub selection: Selection,
}

/// The scope as the session reads it, on one line; the instructions stand in their own block.
pub fn scope_line(scope: &Scope) -> String {
  format!("{}{}", CHAT_ROUND.scope, serde_json::to_string(scope).unwrap_or_default())
}

/// The room's recent messages in the room shape, as a turn that reads no words of its own is handed them: a
/// fold's line before it and the keepalive's touch.
pub async fn recent_lines(receiver: &Receiver, owner: i64, chat_id: i64) -> Result<Vec<String>, Fault> {
  let recent = rows::recent_messages(receiver.db(), chat_id, CHAT_ROUND.recent_messages, "").await?;
  let reading = Reading {
    receiver,
    owner,
    fetch_on_demand: false,
    shown: &recent.iter().map(|message| message.message_id).collect(),
  };

  reading.lines(&recent).await
}

/// What a turn that reads no words of its own knows of its room: whether it is the owner's own chat. Who spoke
/// and who BMO is there are its messages' to say, and it has none.
pub async fn quiet_room(receiver: &Receiver, chat_id: i64) -> Result<Room, Fault> {
  Ok(Room {
    owner_room: receiver.roads().is_owner_room(chat_id).await,
    identity: None,
    private_to: None,
  })
}

// What kind of room the turn is in: the owner's chat, a user's own private chat, or a group.
async fn chat_kind(receiver: &Receiver, room: Room) -> Result<&'static str, Fault> {
  let kinds = &CHAT_ROUND.kinds;

  if room.owner_room {
    return Ok(&kinds.owner);
  }

  if let Some(person) = room.private_to
    && receiver.units().web.user_of_sender(person).await.ok().flatten().is_some()
  {
    return Ok(&kinds.user);
  }

  Ok(&kinds.group)
}

// What the session missed since it last answered, which only the first call of a round carries; the batch's own
// messages are shown a few lines further on.
async fn archived(receiver: &Receiver, round: &Round<'_>) -> Result<Vec<Inbound>, Fault> {
  if !round.first_call {
    return Ok(vec![]);
  }

  let db = receiver.db();
  let last_read = rows::last_read(db, round.chat_id).await?;
  let unified = unified_session(receiver, round.agent).await;
  let seen = match round.session {
    Some(session) if unified => one(
      db,
      r#"SELECT 1 AS "one" FROM "bmo_session_rooms" WHERE "session_id" = $1 AND "chat_id" = $2"#,
      vec![session.into(), round.chat_id.into()],
    )
    .await?
    .is_some(),
    _ => false,
  };
  let context = match last_read {
    Some(last) if !(unified && !seen) => rows::messages_after(db, round.chat_id, last, "").await?,
    _ => rows::recent_messages(db, round.chat_id, CHAT_ROUND.recent_messages, "").await?,
  };
  let batch: HashSet<i64> = round.batch.iter().map(|message| message.message_id).collect();

  Ok(context.into_iter().filter(|message| !batch.contains(&message.message_id)).collect())
}

/// The unread messages an archived block carries of those it is handed, oldest first: the newest `cap` of them in
/// their order, with how many older ones it leaves out.
pub fn carried_unread(mut unread: Vec<Inbound>, cap: usize) -> (Vec<Inbound>, usize) {
  let left_out = unread.len().saturating_sub(cap);

  (unread.split_off(left_out), left_out)
}

/// The archived block's opening: its head counting the messages it carries, and where it left older ones out,
/// the line counting them.
pub fn archived_head(carried: usize, left_out: usize) -> Vec<String> {
  let blocks = &CHAT_ROUND.blocks;
  let mut head = vec![fill_marks(&blocks.archived, &[("count", &carried.to_string())])];

  if left_out > 0 {
    head.push(fill_marks(&blocks.archived_left_out, &[("count", &left_out.to_string())]));
  }

  head
}

// The stretch of a room a turn went back and read, as one line.
fn archive_range(chat_id: i64, messages: &[Inbound]) -> String {
  let reads = &CHAT_ROUND.reads;
  let first = messages.iter().map(|message| message.message_id).min().unwrap_or_default();
  let last = messages.iter().map(|message| message.message_id).max().unwrap_or_default();
  let span = if first == last {
    fill_marks(&reads.one, &[("first", &first.to_string())])
  } else {
    fill_marks(&reads.span, &[("first", &first.to_string()), ("last", &last.to_string())])
  };

  fill_marks(
    &reads.range,
    &[("chat", &chat_id.to_string()), ("span", &span), ("count", &messages.len().to_string())],
  )
}

// The messages a reply reached back for, which stand on their own rather than inside the stretch.
fn quoted_range(chat_id: i64, ids: &[i64]) -> String {
  let mut numbers: Vec<i64> = ids.to_vec();

  numbers.sort_unstable();
  numbers.dedup();

  let named: Vec<String> = numbers.iter().map(|number| format!("#{number}")).collect();

  fill_marks(&CHAT_ROUND.reads.quoted, &[("chat", &chat_id.to_string()), ("ids", &named.join(", "))])
}

/// The round's prompt.
pub async fn compose(receiver: &Receiver, round: &Round<'_>) -> Result<Composed, Fault> {
  let db = receiver.db();
  let header = &CHAT_ROUND.header;
  let blocks = &CHAT_ROUND.blocks;
  let closing = &CHAT_ROUND.closing;
  let chat_id = round.chat_id;
  let mut parts: Vec<String> = Vec::new();
  let mut context: Vec<(String, String)> = Vec::new();
  let unread: Vec<Inbound> = archived(receiver, round)
    .await?
    .into_iter()
    .filter(|message| !round.pending.contains(&message.message_id))
    .collect();
  let (archived, left_out) = carried_unread(unread, CHAT_ROUND.archived_messages as usize);
  let recent_rows: Vec<Inbound> = rows::recent_messages(db, chat_id, CHAT_ROUND.recent_messages, "")
    .await?
    .into_iter()
    .filter(|message| !round.pending.contains(&message.message_id))
    .collect();
  // The room's recent messages, what it missed and the batch, once each, the first place each stands.
  let mut recent: Vec<Inbound> = Vec::new();

  for message in recent_rows.iter().chain(archived.iter()).chain(round.batch.iter()) {
    match recent.iter().position(|one| one.message_id == message.message_id) {
      Some(at) => recent[at] = message.clone(),
      None => recent.push(message.clone()),
    }
  }

  // The scope line leads the prompt but carries the cleaned instructions, so its place is kept here.
  let scope_at = parts.len();

  parts.push(String::new());
  parts.push(round.queue_line.to_owned());

  let shown: HashSet<i64> =
    archived.iter().chain(round.batch.iter()).map(|message| message.message_id).collect();
  let title = rows::chat_title(db, chat_id).await?;

  parts.push(match &title {
    Some(title) => fill_marks(&header.chat_titled, &[("chat", &chat_id.to_string()), ("title", title)]),
    None => fill_marks(&header.chat, &[("chat", &chat_id.to_string())]),
  });
  parts.push(fill_marks(&header.kind, &[("kind", chat_kind(receiver, round.room).await?)]));
  parts.push(fill_marks(
    &header.privilege,
    &[("privilege", if round.all_from_owner { &header.privileged } else { &header.restricted })],
  ));

  parts.push(fill_marks(&header.owner, &[("owner", &round.owner.to_string())]));

  // In a room more than one agent answers, each one's window there.
  if let Some(windows) = room_windows_block(receiver, chat_id, round.agent, now_ms()).await? {
    parts.push(windows);
  }

  let plain = Reading {
    receiver,
    owner: round.owner,
    fetch_on_demand: false,
    shown: &recent.iter().map(|message| message.message_id).collect(),
  };
  let recent_lines = plain.lines(&recent).await?;
  // The session the turn goes on, which the index counts the read entries out by: the one the round holds, else
  // the one the agency stands for the agent, which a start writes nothing of.
  let standing = match round.session {
    Some(session) => Some(session.to_owned()),
    None => {
      let unified = unified_session(receiver, round.agent).await;
      let scope = super::ask::scope_of(unified, chat_id);

      receiver.units().agency.start(round.agent, scope).await.ok().map(|session| session.id)
    }
  };
  let selection = window::select(
    receiver,
    &Material {
      agent: round.agent,
      chat_id,
      room: round.room,
      messages: round.batch,
      instructions: round.instructions,
      said: &[],
      events: round.events,
      recent: &recent_lines,
      holds: None,
      round: round.round.clone(),
      session: standing.as_deref(),
    },
  )
  .await?;
  let instructions = round.instructions.join("\n\n");
  let scope = Scope {
    chat_id,
    message_ids: round.batch.iter().map(|message| message.message_id).collect(),
    reference_message_ids: archived.iter().map(|message| message.message_id).collect(),
    instructions: instructions.clone(),
  };

  parts[scope_at] = scope_line(&scope);

  let reading = |fetch: bool| Reading {
    receiver,
    owner: round.owner,
    fetch_on_demand: fetch,
    shown: &shown,
  };

  if !archived.is_empty() {
    context.push(("archive".to_owned(), archive_range(chat_id, &archived)));

    let lines = reading(false).lines(&archived).await?;

    parts.push(String::new());
    parts.extend(archived_head(lines.len(), left_out));
    parts.extend(lines);
    parts.push(blocks.archived_end.clone());
  }

  // What an earlier round waited on stands apart, as words the session has not answered.
  let waited: Vec<Inbound> = round
    .batch
    .iter()
    .filter(|message| round.waited_on.contains(&message.message_id))
    .cloned()
    .collect();
  let fresh: Vec<Inbound> = round
    .batch
    .iter()
    .filter(|message| !round.waited_on.contains(&message.message_id))
    .cloned()
    .collect();

  if !waited.is_empty() {
    let lines = reading(round.tools_allowed).lines(&waited).await?;

    parts.push(String::new());
    parts.push(fill_marks(&blocks.waited, &[("count", &lines.len().to_string())]));
    parts.extend(lines);
    parts.push(blocks.waited_end.clone());
  }

  if !fresh.is_empty() {
    let lines = reading(round.tools_allowed).lines(&fresh).await?;

    parts.push(String::new());
    parts.push(fill_marks(&blocks.fresh, &[("count", &lines.len().to_string())]));
    parts.extend(lines);
    parts.push(blocks.fresh_end.clone());
  } else if round.instructions.is_empty() {
    parts.push(String::new());
    parts.push(blocks.nothing.clone());
  }

  if !round.instructions.is_empty() {
    parts.push(String::new());
    parts.push(fill_marks(&blocks.quiet, &[("count", &round.instructions.len().to_string())]));
    parts.push(instructions.clone());
    parts.push(blocks.quiet_end.clone());
  }

  // What the brain holds stands after the turn's words, so the session reads what came in before it decides
  // whether to look anything up; a person's own thread from their group rooms goes in beside it whole
  // (docs/bmo-window.md, 「What every turn carries」).
  for block in [&selection.block, &selection.index] {
    if !block.is_empty() {
      parts.push(String::new());
      parts.push(block.clone());
    }
  }

  // A notice of the queue shows the other agents' work and the consult room beside it.
  if tells_of_queue(round.events)
    && let Some(consult) = consult_block(receiver, round.agent, now_ms()).await?
  {
    parts.push(String::new());
    parts.push(consult);
  }

  // The owner's own words in his own chat are owed an answer whatever else the round carried.
  let owed = if round.room.owner_room {
    round.batch.iter().filter(|message| message.sender_id == Some(round.owner)).count()
  } else {
    0
  };

  if owed > 0 {
    parts.push(String::new());
    let owed_line = if round.questioned { &closing.questioned_owed } else { &closing.owed };

    parts.push(fill_marks(owed_line, &[("count", &owed.to_string())]));
  }

  // The language an answer is written in is the agent's own judgement, from the person and the room as the
  // brain holds them; no line of the turn names one (docs/answer-checks.md).
  if let Some(reason) = round.woken {
    parts.push(String::new());
    parts.push(fill_marks(&closing.woken, &[("reason", reason)]));
  }

  parts.push(String::new());
  parts.push(match (round.questioned, round.force, owed > 0) {
    (true, true, _) => closing.questioned_forced.clone(),
    (true, false, _) => closing.questioned_waiting.clone(),
    (false, true, true) => closing.forced_owed.clone(),
    (false, true, false) => closing.forced.clone(),
    (false, false, _) => closing.waiting.clone(),
  });

  // A reply whose target the prompt does not otherwise show pulls that message out of the archive on its own.
  let quoted: Vec<i64> = archived
    .iter()
    .chain(round.batch.iter())
    .filter_map(|message| message.reply_to_message_id)
    .filter(|id| !shown.contains(id))
    .collect();

  if !quoted.is_empty() {
    context.push(("archive".to_owned(), quoted_range(chat_id, &quoted)));
  }

  // The round that writes the message reads the room's situation note first (docs/drafting.md, 「The situation
  // note」): the update turn wrote it before this round, and the message is written with the room's state in front.
  let front = match round.questioned {
    true => None,
    false => match super::situation::read(db, chat_id).await {
      Ok(Some(note)) => Some(format!("{}\n{note}", CHAT_ROUND.situation.front)),
      Ok(None) => None,
      Err(why) => {
        eprintln!("bmo-receiver: the room's situation note could not be read: {why}");

        None
      }
    },
  };
  let prompt = match front {
    Some(front) => format!("{front}\n\n{}", parts.join("\n")),
    None => parts.join("\n"),
  };

  Ok(Composed {
    prompt,
    recent,
    scope,
    context,
    selection,
  })
}

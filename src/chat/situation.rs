//! The room's situation note (docs/drafting.md, 「The situation note」): one light turn reads the room's note as it
//! stands and the messages that came in since it last read the room's record, writes it afresh and keeps it for the
//! room with the row it read to. The note is the receiver's own row, one a room. It is written when the agent's
//! session leaves a room it answered in, before a handoff and on a keepalive touch, and a session reads a room's
//! note once, in the round that first takes it into the room.

use receiver_policy::chat_round::CHAT_ROUND;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use crate::chat::promises::own_sentences;
use crate::chat::shape::Reading;
use crate::js;
use crate::receiver::{Inbound, Receiver, rows};
use crate::store::settings;
use crate::{Fault, now_ms, one, run};

// The wrapping a note rides in until the session it goes to is settled: a private-use pair no document is written
// in and that never leaves this process.
const NOTE_OPEN: &str = "\u{e000}SITUATION-ONCE ";
const NOTE_CLOSE: &str = "\u{e000}END SITUATION-ONCE";

// The place a round's one reading of the room's note stands at until the session it goes to is settled: the note
// is read then, so one written while the turn waited, as before a handoff, is the one the session reads.
const FIRST_OPEN: &str = "\u{e000}SITUATION-FIRST ";
const FIRST_CLOSE: &str = "\u{e000}";

static NOTE: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"\x{e000}SITUATION-ONCE (\{[^\n]*\})\n((?s:.*?))\n\x{e000}END SITUATION-ONCE")
    .expect("a wrapped situation note")
});

/// The room's note as it stands, none where the room has none yet.
pub async fn read(db: &DatabaseConnection, chat_id: i64) -> Result<Option<String>, Fault> {
  let Some(row) =
    one(db, r#"SELECT "text" FROM "room_situations" WHERE "chat_id" = $1"#, vec![chat_id.into()]).await?
  else {
    return Ok(None);
  };

  let text: String = row.try_get("", "text")?;

  Ok((!text.trim().is_empty() && !first_person_claim(&text, &CHAT_ROUND.situation.claim)).then_some(text))
}

/// The version of a note a session read: the note's text itself, and the room it is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteMark {
  pub chat_id: i64,
  pub fingerprint: String,
}

/// The version of a note, the note's text itself (docs/bmo-window.md, 「What every turn carries」).
pub fn fingerprint(note: &str) -> String {
  js::bun_hash(note).to_string()
}

#[derive(Deserialize, Serialize)]
struct NoteHead {
  chat: i64,
  fingerprint: String,
}

/// The note as an update turn carries it, wrapped to go to the session the turn settles on late (`resolve`): its
/// room and version in the head, its text in the body. It goes whole where that session has not read this
/// version, and as the policy's one-line marker where it has.
pub fn note_block(chat_id: i64, note: &str) -> String {
  let head = NoteHead {
    chat: chat_id,
    fingerprint: fingerprint(note),
  };

  format!("{NOTE_OPEN}{}\n{note}\n{NOTE_CLOSE}", serde_json::to_string(&head).unwrap_or_default())
}

/// The place a round's turn reads the room's note at, filled once the session the turn goes to is settled
/// (`resolve`): the note as it stands then, under the policy's line, where that session holds no note of the
/// room, and nothing where it holds one or the room has none, since a session reads a room's note once.
pub fn first_reading(chat_id: i64) -> String {
  format!("{FIRST_OPEN}{chat_id}{FIRST_CLOSE}")
}

/// Each wrapped note opened against the session the turn settled on. An update turn's: the note whole where the
/// session has not read this version, the policy's one-line marker where it has. A round's one reading: the room's
/// note as it stands now, whole under its line, where the session holds none of the room, and nothing where it
/// holds one or the room has none, the blank lines after its place going with it. With the marks to record once the
/// turn is delivered, which are none for a note that did not go whole.
pub async fn resolve(
  db: &DatabaseConnection,
  session: Option<&str>,
  prompt: &str,
) -> (String, Vec<NoteMark>) {
  let (prompt, mut marks) = first_readings(db, session, prompt).await;

  if !prompt.contains(NOTE_OPEN) {
    return (prompt, marks);
  }

  let mut out = String::new();
  let mut at = 0;

  for found in NOTE.captures_iter(&prompt) {
    let whole = found.get(0).expect("the whole note block");
    let head = serde_json::from_str::<NoteHead>(&found[1]).ok();
    let note = &found[2];

    out.push_str(&prompt[at..whole.start()]);

    match head.filter(|head| head.fingerprint == fingerprint(note)) {
      Some(head) => {
        let (shown, mark) = carried(db, session, head.chat, note).await;

        marks.extend(mark);
        out.push_str(&shown);
      }
      None => out.push_str(note),
    }

    at = whole.end();
  }

  out.push_str(&prompt[at..]);

  (out, marks)
}

// Each place a round reads its room's note at, filled against the session the turn settled on.
async fn first_readings(
  db: &DatabaseConnection,
  session: Option<&str>,
  prompt: &str,
) -> (String, Vec<NoteMark>) {
  let mut out = String::new();
  let mut rest = prompt;
  let mut marks = vec![];

  while let Some(at) = rest.find(FIRST_OPEN) {
    let after = &rest[at + FIRST_OPEN.len()..];
    let Some(end) = after.find(FIRST_CLOSE) else { break };
    let chat = after[..end].parse::<i64>().ok();

    out.push_str(&rest[..at]);
    rest = &after[end + FIRST_CLOSE.len()..];

    let note = match chat {
      Some(chat) if !holds(db, session, chat).await => {
        read(db, chat).await.unwrap_or(None).map(|note| (chat, note))
      }
      _ => None,
    };

    match note {
      Some((chat, note)) => {
        marks.push(NoteMark {
          chat_id: chat,
          fingerprint: fingerprint(&note),
        });
        out.push_str(&format!("{}\n{note}", CHAT_ROUND.situation.front));
      }
      None => rest = rest.trim_start_matches('\n'),
    }
  }

  out.push_str(rest);

  (out, marks)
}

/// The note as a turn carries it: the note whole where the session has not read this version, and the policy's
/// one-line marker where it has; with the mark to record once the turn is delivered, which is none where the note
/// went as its marker. A note no session continues is always whole.
pub async fn carried(
  db: &DatabaseConnection,
  session: Option<&str>,
  chat_id: i64,
  note: &str,
) -> (String, Option<NoteMark>) {
  let mark = || {
    Some(NoteMark {
      chat_id,
      fingerprint: fingerprint(note),
    })
  };
  let Some(session) = session else { return (note.to_owned(), mark()) };
  let held = match one(
    db,
    r#"SELECT "fingerprint" FROM "bmo_session_notes" WHERE "session_id" = $1 AND "chat_id" = $2"#,
    vec![session.into(), chat_id.into()],
  )
  .await
  {
    Ok(row) => row.and_then(|row| row.try_get::<String>("", "fingerprint").ok()),
    Err(why) => {
      eprintln!("chat {chat_id}: the situation note's version could not be read: {}", why.detail);

      return (note.to_owned(), mark());
    }
  };

  if held.as_deref() == Some(fingerprint(note).as_str()) {
    return (CHAT_ROUND.situation.unchanged.clone(), None);
  }

  (note.to_owned(), mark())
}

/// Whether a session holds a note of the room, whichever version: one it read or one it wrote. A turn no session
/// continues holds none, and a record that cannot be read is read as none, so the note goes rather than is lost.
pub async fn holds(db: &DatabaseConnection, session: Option<&str>, chat_id: i64) -> bool {
  let Some(session) = session else { return false };

  match one(
    db,
    r#"SELECT 1 AS "held" FROM "bmo_session_notes" WHERE "session_id" = $1 AND "chat_id" = $2"#,
    vec![session.into(), chat_id.into()],
  )
  .await
  {
    Ok(row) => row.is_some(),
    Err(why) => {
      eprintln!(
        "chat {chat_id}: whether the session holds the situation note could not be read: {}",
        why.detail
      );

      false
    }
  }
}

/// The note a session now holds, recorded under the session and the room once its turn is delivered.
pub async fn keep(db: &DatabaseConnection, session: &str, mark: &NoteMark) -> Result<(), Fault> {
  run(
    db,
    r#"INSERT INTO "bmo_session_notes" ("session_id", "chat_id", "fingerprint")
       VALUES ($1, $2, $3)
       ON CONFLICT ("session_id", "chat_id") DO UPDATE SET "fingerprint" = excluded."fingerprint""#,
    vec![session.into(), mark.chat_id.into(), mark.fingerprint.clone().into()],
  )
  .await?;

  Ok(())
}

/// A session's record of the notes it read let go where the session is replaced or folds.
pub async fn forget(db: &DatabaseConnection, session: Option<&str>) -> Result<(), Fault> {
  if let Some(session) = session {
    run(db, r#"DELETE FROM "bmo_session_notes" WHERE "session_id" = $1"#, vec![session.into()]).await?;
  }

  Ok(())
}

/// The room's note replaced by what the update turn wrote, and the row of the room's record it has read to, taken
/// only where the note changed; the position moves even where the note stands.
pub async fn write(
  db: &DatabaseConnection,
  chat_id: i64,
  text: &str,
  read_through: Option<i64>,
) -> Result<(), Fault> {
  run(
    db,
    r#"INSERT INTO "room_situations" ("chat_id", "text", "updated_at", "read_through")
       VALUES ($1, $2, $3, $4)
       ON CONFLICT ("chat_id") DO UPDATE SET
         "updated_at" = CASE WHEN "room_situations"."text" IS DISTINCT FROM excluded."text"
           THEN excluded."updated_at" ELSE "room_situations"."updated_at" END,
         "text" = excluded."text",
         "read_through" = COALESCE(excluded."read_through", "room_situations"."read_through")"#,
    vec![chat_id.into(), text.into(), now_ms().into(), read_through.into()],
  )
  .await?;

  Ok(())
}

/// The row of the room's record the note has read to, none where the room has no note yet.
pub async fn read_through(db: &DatabaseConnection, chat_id: i64) -> Result<Option<i64>, Fault> {
  let Some(row) = one(
    db,
    r#"SELECT "read_through" FROM "room_situations" WHERE "chat_id" = $1"#,
    vec![chat_id.into()],
  )
  .await?
  else {
    return Ok(None);
  };

  Ok(row.try_get::<Option<i64>>("", "read_through")?)
}

/// The room's note and when it last changed, none where the room has none yet.
pub async fn read_at(db: &DatabaseConnection, chat_id: i64) -> Result<Option<(String, i64)>, Fault> {
  let Some(row) = one(
    db,
    r#"SELECT "text", "updated_at" FROM "room_situations" WHERE "chat_id" = $1"#,
    vec![chat_id.into()],
  )
  .await?
  else {
    return Ok(None);
  };

  let text: String = row.try_get("", "text")?;
  let updated_at: i64 = row.try_get("", "updated_at")?;

  Ok(
    (!text.trim().is_empty() && !first_person_claim(&text, &CHAT_ROUND.situation.claim))
      .then_some((text, updated_at)),
  )
}

/// The note as it stood when a round judged these messages, kept on each message it judged with whether that round
/// changed it, so the console's judgement record says what the room's situation was as the round read it.
pub async fn attach(
  db: &DatabaseConnection,
  chat_id: i64,
  messages: &[i64],
  note: Option<&str>,
  changed: bool,
) -> Result<(), Fault> {
  for message_id in messages {
    run(
      db,
      r#"UPDATE "messages" SET "situation" = $1, "situation_changed" = $2 WHERE "chat_id" = $3 AND "message_id" = $4"#,
      vec![
        note.map(str::to_owned).into(),
        Some(changed).into(),
        chat_id.into(),
        (*message_id).into(),
      ],
    )
    .await?;
  }

  Ok(())
}

/// Whether a gap as long as `standing` messages is read as its newest `gap` alone: a room the note has not read
/// for long is caught up from its latest messages, so one turn is never asked to read a whole backlog; a gap of 0
/// reads every message since the mark.
pub fn catch_up(gap: usize, standing: i64) -> bool {
  gap > 0 && standing > i64::try_from(gap).unwrap_or(i64::MAX)
}

/// The one turn that updates the note: its short mark, the note as it stands and the room's messages that came in
/// since the note last read the room's record. What the turn is asked stands in the room's block.
pub fn update_prompt(chat_id: i64, old: Option<&str>, lines: &[String]) -> String {
  let said = &CHAT_ROUND.situation;
  let mut parts = vec![said.marker.clone(), String::new(), said.previous.clone()];

  parts.push(old.map(|note| note_block(chat_id, note)).unwrap_or_else(|| said.none.clone()));
  parts.push(String::new());
  parts.push(said.since.clone());
  parts.extend(lines.iter().cloned());
  parts.join("\n")
}

/// A first-person claim that the agent replied, sent, attached or fixed something: the note may not hold one,
/// whether it wrote it or copied it from the room.
fn first_person_claim(text: &str, claim: &str) -> bool {
  static ENGLISH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
      r"(?i)\b(i|i'm|i've|i have|we|we're|we've|we have)\b.{0,60}\b(replied|answered|sent|attached|fixed|handled|delivered|corrected|updated|wrote|posted|uploaded)\b",
    )
    .expect("a first-person claim pattern")
  });
  let korean = Regex::new(claim).is_ok_and(|pattern| pattern.is_match(text));

  ENGLISH.is_match(text) || korean
}

// A sentence flattened for a copy comparison.
fn flattened(text: &str) -> String {
  text
    .to_lowercase()
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ")
    .trim_matches(|one: char| !one.is_alphanumeric())
    .to_owned()
}

/// Whether the note repeats a sentence of a line the turn was shown: a message, the agent's own answer, or a
/// quiet instruction.
fn copies(answer: &str, shown: &[String]) -> bool {
  let lines: Vec<String> = shown.iter().map(|line| flattened(line)).collect();

  own_sentences(answer).iter().any(|sentence| {
    let sentence = flattened(sentence);

    sentence.chars().count() >= 12 && lines.iter().any(|line| line.contains(&sentence))
  })
}

/// The note an update turn's answer writes: one line, under the policy's ceiling, none where the turn skipped,
/// waited or said nothing, and none that copies a shown line or makes a first-person claim of a reply, a fix, an
/// attachment or a send.
pub fn note_from_answer(answer: &str, shown: &[String]) -> Option<String> {
  let said = &CHAT_ROUND.situation;
  let text = crate::form::strip_prompt_scaffolding(answer);
  let text = text.trim();

  if text.is_empty() || super::sentinels::is_skip(text) || super::sentinels::is_wait(text) {
    return None;
  }

  if first_person_claim(text, &said.claim) || copies(text, shown) {
    return None;
  }

  let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
  let kept = if js::units(&flat) > said.ceiling_chars {
    let room = said.ceiling_chars.saturating_sub(js::units(&said.cut));
    let units: Vec<u16> = flat.encode_utf16().take(room).collect();

    format!("{}{}", String::from_utf16_lossy(&units), said.cut)
  } else {
    flat
  };

  (!kept.is_empty()).then_some(kept)
}

/// Where the rooms an agent owes a situation note are kept (docs/drafting.md, 「The situation note」).
pub fn owed_key(agent: &str) -> String {
  format!("{agent}_notes_owed")
}

/// The rooms an agent has sent a round's message in since their note was last updated, in the order of the last
/// answer: the last of them is the room it last answered in.
pub async fn owed(db: &DatabaseConnection, agent: &str) -> Vec<i64> {
  match settings::get(db, &owed_key(agent)).await {
    Ok(held) => held.and_then(|held| serde_json::from_str(&held).ok()).unwrap_or_default(),
    Err(why) => {
      eprintln!("{agent}: the rooms owed a situation note could not be read: {}", why.detail);

      vec![]
    }
  }
}

async fn keep_owed(db: &DatabaseConnection, agent: &str, rooms: &[i64]) {
  let kept = settings::set(db, &owed_key(agent), &serde_json::to_string(rooms).unwrap_or_default()).await;

  if let Err(why) = kept {
    eprintln!("{agent}: the rooms owed a situation note were not kept: {}", why.detail);
  }
}

/// A room the agent has just sent a round's message in: owed a note, and the room last answered in.
pub async fn owe(db: &DatabaseConnection, agent: &str, chat_id: i64) {
  let mut rooms = owed(db, agent).await;

  if rooms.last() == Some(&chat_id) {
    return;
  }

  rooms.retain(|room| *room != chat_id);
  rooms.push(chat_id);
  keep_owed(db, agent, &rooms).await;
}

/// A room whose note has been updated, or whose record holds nothing its note has not read: owed none.
pub async fn settle(db: &DatabaseConnection, agent: &str, chat_id: i64) {
  let mut rooms = owed(db, agent).await;

  if !rooms.contains(&chat_id) {
    return;
  }

  rooms.retain(|room| *room != chat_id);
  keep_owed(db, agent, &rooms).await;
}

/// One room's note update, ready to be asked: the turn's prompt, and what its answer is read against.
pub struct Update {
  pub chat_id: i64,
  pub prompt: String,
  lines: Vec<String>,
  old: Option<String>,
  through: Option<i64>,
}

/// The update of a room's note as its turn is asked (docs/drafting.md, 「The situation note」): the note as it
/// stands and what it has not read — the room's record after the row it read to, or, where the room has no note
/// yet, the latest messages the policy names, so the first note has a room to stand on. A gap longer than the
/// policy's bound is read as its newest messages alone. None where the record holds nothing the note has not read.
pub async fn update_of(receiver: &Receiver, owner: i64, chat_id: i64) -> Result<Option<Update>, Fault> {
  let db = receiver.db();
  let gap = CHAT_ROUND.situation.gap_messages;
  let unread = match read_through(db, chat_id).await? {
    Some(after) => {
      let standing = rows::count_since_row(db, chat_id, after).await?;

      if catch_up(gap, standing) {
        rows::recent_rows(db, chat_id, gap as u64, "").await?
      } else {
        rows::messages_since_row(db, chat_id, after, "").await?
      }
    }
    None => rows::recent_rows(db, chat_id, CHAT_ROUND.recent_messages, "").await?,
  };

  if unread.is_empty() {
    return Ok(None);
  }

  let through = unread.iter().map(|(row, _)| *row).max();
  let mut messages: Vec<Inbound> = unread.into_iter().map(|(_, message)| message).collect();

  messages.sort_by_key(|message| (message.sent_at, message.message_id));

  let shown: HashSet<i64> = messages.iter().map(|message| message.message_id).collect();
  let reading = Reading {
    receiver,
    owner,
    fetch_on_demand: false,
    shown: &shown,
  };
  let lines = reading.lines(&messages).await?;
  let old = read(db, chat_id).await?;

  Ok(Some(Update {
    chat_id,
    prompt: update_prompt(chat_id, old.as_deref(), &lines),
    lines,
    old,
    through,
  }))
}

/// What an update turn answered, kept: the note it wrote, or the note as it stood where it wrote none, with the
/// row the turn read to; and the session that answered recorded as holding the note it wrote.
pub async fn written(
  db: &DatabaseConnection,
  update: &Update,
  session: Option<&str>,
  answer: &str,
) -> Result<(), Fault> {
  let Some(note) = note_from_answer(answer, &update.lines).or(update.old.clone()) else {
    return Ok(());
  };

  write(db, update.chat_id, &note, update.through).await?;

  if let Some(session) = session {
    let mark = NoteMark {
      chat_id: update.chat_id,
      fingerprint: fingerprint(&note),
    };

    keep(db, session, &mark).await?;
  }

  Ok(())
}

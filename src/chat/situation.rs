//! The room's situation note (docs/drafting.md, 「The situation note」): one light turn before the supervisor's
//! judgement reads the room's note as it stands and the new lines, writes it afresh and keeps it for the room. The
//! note is the receiver's own row, one a room, and is handed to the judgement and to the round's draft as a piece.

use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::fill_marks;
use sea_orm::DatabaseConnection;

use crate::js;
use crate::receiver::Receiver;
use crate::{Fault, now_ms, one, run};
use receiver_ports::gate::{Entry, Source};

// The path a room's situation note stands at in the round's record, apart from the gate's own situations.
const PREFIX: &str = "room-situations/";

/// The path a room's situation note stands at.
pub fn path(chat_id: i64) -> String {
  format!("{PREFIX}{chat_id}")
}

/// The room's note as it stands, none where the room has none yet.
pub async fn read(db: &DatabaseConnection, chat_id: i64) -> Result<Option<String>, Fault> {
  let Some(row) =
    one(db, r#"SELECT "text" FROM "room_situations" WHERE "chat_id" = $1"#, vec![chat_id.into()]).await?
  else {
    return Ok(None);
  };

  let text: String = row.try_get("", "text")?;

  Ok((!text.trim().is_empty()).then_some(text))
}

/// The room's note replaced by what the update turn wrote, the row taken only where the note changed.
pub async fn write(db: &DatabaseConnection, chat_id: i64, text: &str) -> Result<(), Fault> {
  run(
    db,
    r#"INSERT INTO "room_situations" ("chat_id", "text", "updated_at") VALUES ($1, $2, $3)
       ON CONFLICT ("chat_id") DO UPDATE SET "text" = excluded."text", "updated_at" = excluded."updated_at"
       WHERE "room_situations"."text" IS DISTINCT FROM excluded."text""#,
    vec![chat_id.into(), text.into(), now_ms().into()],
  )
  .await?;

  Ok(())
}

/// The room's note as a piece of a turn's core, none where the room has none.
pub async fn entry(receiver: &Receiver, chat_id: i64) -> Result<Option<Entry>, Fault> {
  let Some(text) = read(receiver.db(), chat_id).await? else {
    return Ok(None);
  };

  Ok(Some(Entry {
    path: path(chat_id),
    text,
    source: Source::Situation,
    fragment: None,
    holds: Some(CHAT_ROUND.situation.holds.clone()),
  }))
}

/// The one turn that updates the note: the note as it stands, the room's recent words and the new lines.
pub fn update_prompt(old: Option<&str>, recent: &[String], judged: &[String]) -> String {
  let said = &CHAT_ROUND.situation;
  let head = fill_marks(&said.head, &[("ceiling", &said.ceiling_chars.to_string())]);
  let mut parts = vec![head, String::new(), said.previous.clone()];

  parts.push(old.map(str::to_owned).unwrap_or_else(|| said.none.clone()));
  parts.push(String::new());
  parts.push(said.recent.clone());
  parts.extend(recent.iter().cloned());
  parts.push(String::new());
  parts.push(said.judged.clone());
  parts.extend(judged.iter().cloned());
  parts.join("\n")
}

/// The note an update turn's answer writes: one line, under the policy's ceiling, none where the turn skipped,
/// waited or said nothing.
pub fn note_from_answer(answer: &str) -> Option<String> {
  let said = &CHAT_ROUND.situation;
  let text = crate::form::strip_prompt_scaffolding(answer);
  let text = text.trim();

  if text.is_empty() || super::sentinels::is_skip(text) || super::sentinels::is_wait(text) {
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

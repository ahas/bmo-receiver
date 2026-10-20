//! What opened a room's turns, as the console stands it with their verdicts (docs/bmo-window.md, 「Quiet
//! instruction」): the quiet instructions each round settled, without their text, and one of them whole, asked
//! by the room it was queued for or the room whose round settled it. The gate keeps what each stage offered of a
//! round, and reads that record there.

use std::collections::BTreeMap;

use axum::{
  extract::{Path, RawQuery, State},
  http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};

use sea_orm::{DatabaseConnection, QueryResult, Value as SeaValue};

use crate::receiver::Receiver;
use crate::routes::{Answered, Served, answered, refused, store};
use crate::store::Fault;
use crate::{all, one};

// How many rounds one asking may name: a page of a room's messages carries far fewer.
const ROUNDS_ASKED: usize = 200;

// How long a summary may run before it is cut: one line on any screen.
pub const SUMMARY_CHARS: usize = 300;

#[derive(Deserialize)]
pub struct Asked {
  room: i64,

  #[serde(default)]
  rounds: String,
}

#[derive(Deserialize)]
pub struct OneAsked {
  room: i64,
}

/// The line a person reads an instruction by: the summary its producer gave where it gave one, and
/// otherwise the first line of its text, cut to one screen line.
pub fn summary_of(summary: Option<&str>, text: &str) -> String {
  let source = summary.filter(|given| !given.trim().is_empty()).unwrap_or(text);
  let line = source.lines().map(str::trim).find(|line| !line.is_empty()).unwrap_or_default();

  if line.chars().count() <= SUMMARY_CHARS {
    return line.to_owned();
  }

  format!("{}…", line.chars().take(SUMMARY_CHARS).collect::<String>())
}

/// The rounds named that are the room's own: a round's name opens on its room.
pub fn rounds_of(chat: i64, asked: &str) -> Vec<String> {
  let head = format!("round:{chat}:");
  let mut rounds: Vec<String> = asked
    .split(',')
    .map(str::trim)
    .filter(|round| round.starts_with(&head))
    .map(str::to_owned)
    .collect();

  rounds.sort();
  rounds.dedup();
  rounds.truncate(ROUNDS_ASKED);
  rounds
}

// `$1, $2, …` for as many values, numbered from `from`.
fn numbered(from: usize, count: usize) -> String {
  (from..from + count).map(|at| format!("${at}")).collect::<Vec<_>>().join(", ")
}

/// What one instruction a round settled is, without its text.
fn opening(row: &QueryResult) -> Value {
  let summary: Option<String> = row.try_get("", "summary").ok().flatten();
  let text: String = row.try_get("", "text").ok().unwrap_or_default();
  let sort: Option<String> = row.try_get::<Option<String>>("", "sort").ok().flatten();

  json!({
    "id": row.try_get::<i64>("", "id").ok().unwrap_or_default(),
    "sort": sort.filter(|sort| !sort.trim().is_empty()),
    "summary": summary_of(summary.as_deref(), &text),
    "createdAt": row.try_get::<i64>("", "created_at").ok().unwrap_or_default() * 1_000,
  })
}

/// The instructions each round named settled, a round that settled none not in it.
async fn read_rounds(db: &DatabaseConnection, rounds: &[String]) -> Result<Vec<Value>, Fault> {
  let mut read: BTreeMap<String, Vec<Value>> = BTreeMap::new();

  if rounds.is_empty() {
    return Ok(vec![]);
  }

  let statement = format!(
    "SELECT id, round, sort, summary, text, created_at FROM instructions WHERE round IN ({}) ORDER BY id",
    numbered(1, rounds.len()),
  );
  let values: Vec<SeaValue> = rounds.iter().map(|round| round.into()).collect();

  for row in all(db, &statement, values).await? {
    let round: String = row.try_get("", "round").unwrap_or_default();

    read.entry(round).or_default().push(opening(&row));
  }

  Ok(
    read
      .into_iter()
      .map(|(round, instructions)| json!({ "round": round, "instructions": instructions }))
      .collect(),
  )
}

/// For a room and the rounds named, what each of them settled.
pub async fn openings(State(served): State<Served>, raw: RawQuery) -> Answered {
  let asked = super::query::<Asked>(raw)?;
  let receiver: Receiver = (*served.receiver).clone();
  let chat = asked.room;
  let rounds = rounds_of(chat, &asked.rounds);

  answered(json!({ "items": read_rounds(receiver.db(), &rounds).await.map_err(store)?, "more": false }))
}

/// One quiet instruction's whole text, asked by the room it was queued for or the room whose round settled it.
pub async fn instruction(State(served): State<Served>, Path(id): Path<i64>, raw: RawQuery) -> Answered {
  let asked = super::query::<OneAsked>(raw)?;
  let receiver: Receiver = (*served.receiver).clone();
  let chat = asked.room;
  let row = one(
    receiver.db(),
    r#"SELECT "text" FROM "instructions" WHERE "id" = $1 AND ("chat_id" = $2 OR "round" LIKE $3)"#,
    vec![id.into(), chat.into(), format!("round:{chat}:%").into()],
  )
  .await
  .map_err(store)?;
  let Some(row) = row else {
    return Err(refused(
      StatusCode::NOT_FOUND,
      "not-found",
      &format!("instruction {id} is not this room's"),
    ));
  };
  let text: String = row.try_get("", "text").unwrap_or_default();

  answered(json!({ "id": id, "text": text }))
}

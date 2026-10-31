//! The rows the store already holds, read for secret-shaped runs and cleaned in place (docs/boundary-api.md, 「The
//! service and its commands」, `redact-kept`): a message's words, what was heard in its voice note and the room's
//! situation note its round read, every string of the payload kept beside it, and each room's own situation note.
//! What it answers is counts and the messages it changed, by room and number, and never a word of a row.

use std::collections::BTreeSet;

use sea_orm::DatabaseConnection;
use serde_json::Value;

use super::{redact, redact_value};
use crate::{Fault, all, run};

/// How many rows a page of a column reads at once.
const PAGE: i64 = 500;

/// One column of the store the shapes are read over.
struct Place {
  table: &'static str,
  column: &'static str,
  /// Whether a row stands under a message, and not under a room alone.
  of_message: bool,
  /// Whether the column holds JSON, whose strings alone are read.
  json: bool,
}

const PLACES: [Place; 5] = [
  Place {
    table: "messages",
    column: "text",
    of_message: true,
    json: false,
  },
  Place {
    table: "messages",
    column: "transcript",
    of_message: true,
    json: false,
  },
  Place {
    table: "messages",
    column: "situation",
    of_message: true,
    json: false,
  },
  Place {
    table: "message_metadata",
    column: "metadata",
    of_message: true,
    json: true,
  },
  Place {
    table: "room_situations",
    column: "text",
    of_message: false,
    json: false,
  },
];

/// What one column lost: how many of its rows changed and how many runs they held.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
  pub table: &'static str,
  pub column: &'static str,
  pub rows: usize,
  pub runs: usize,
}

/// What the store lost: each column's count, the messages changed by room and number, and the rooms whose own
/// situation note changed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cleaned {
  pub columns: Vec<Column>,
  pub messages: BTreeSet<(i64, i64)>,
  pub rooms: BTreeSet<i64>,
}

// A row's value as the shapes leave it, and how many runs it lost: a JSON column's strings alone, or the whole
// value where it reads as no JSON.
fn cleaned(held: &str, json: bool) -> (String, usize) {
  if json && let Ok(mut value) = serde_json::from_str::<Value>(held) {
    let runs = redact_value(&mut value);

    return (if runs > 0 { value.to_string() } else { held.to_owned() }, runs);
  }

  let read = redact(held);

  (read.text, read.runs)
}

/// Every row of every place read for secret-shaped runs, and, where `write` holds, each run replaced in place.
pub async fn redact_kept(db: &DatabaseConnection, write: bool) -> Result<Cleaned, Fault> {
  let mut report = Cleaned::default();

  for place in &PLACES {
    let message = if place.of_message { r#""message_id""# } else { "NULL::bigint" };
    let read = format!(
      r#"SELECT "rowid", "chat_id", {message} AS "message_id", "{column}" AS "held" FROM "{table}"
         WHERE "rowid" > $1 AND "{column}" IS NOT NULL ORDER BY "rowid" LIMIT {PAGE}"#,
      column = place.column,
      table = place.table,
    );
    let written = format!(r#"UPDATE "{}" SET "{}" = $1 WHERE "rowid" = $2"#, place.table, place.column);
    let mut column = Column {
      table: place.table,
      column: place.column,
      rows: 0,
      runs: 0,
    };
    let mut after = i64::MIN;

    loop {
      let rows = all(db, &read, vec![after.into()]).await?;

      for row in &rows {
        let rowid: i64 = row.try_get("", "rowid")?;
        let chat_id: i64 = row.try_get("", "chat_id")?;
        let message_id: Option<i64> = row.try_get("", "message_id")?;
        let held: String = row.try_get("", "held")?;
        let (text, runs) = cleaned(&held, place.json);

        after = rowid;

        if runs == 0 {
          continue;
        }

        if write {
          run(db, &written, vec![text.into(), rowid.into()]).await?;
        }

        column.rows += 1;
        column.runs += runs;

        match message_id {
          Some(message_id) => report.messages.insert((chat_id, message_id)),
          None => report.rooms.insert(chat_id),
        };
      }

      if i64::try_from(rows.len()).unwrap_or(i64::MAX) < PAGE {
        break;
      }
    }

    report.columns.push(column);
  }

  Ok(report)
}

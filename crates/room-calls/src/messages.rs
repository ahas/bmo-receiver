//! A room's messages read out of the receiver's database, newest first, as `bmo archive scan` prints them and the
//! archive-scan tool hands them back (docs/messages.md, 「The archive」): the one way a session reaches what was
//! actually said rather than what somebody remembers being said.

use chrono::DateTime;

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement, Value};

use crate::reading::{MAX_VALUE_CHARS, Said, cap_items, cap_value, count_text};

/// How many messages a scan answers at most, and where it is asked for none.
pub const SCAN_MOST: i64 = 500;
pub const SCAN_DEFAULT: i64 = 100;

/// What a scan of a room's messages asks for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScanAsked {
  pub chat_id: i64,
  /// One message by its own number, which is handed back whole.
  pub id: Option<i64>,
  pub limit: i64,
  pub before_id: Option<i64>,
  pub contains: Option<String>,
  pub sender: Option<i64>,
}

// One message as the database keeps it.
struct Scanned {
  id: i64,
  sender: Option<i64>,
  sender_name: String,
  sent_at: i64,
  text: String,
}

// A moment in epoch milliseconds as an instant, to the millisecond.
fn instant(milliseconds: i64) -> String {
  DateTime::from_timestamp_millis(milliseconds)
    .map(|at| at.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
    .unwrap_or_else(|| milliseconds.to_string())
}

// One value a condition binds.
enum Bound {
  Number(i64),
  Text(String),
}

// One value a condition binds, at the one width each kind is bound at.
fn bound(value: Bound) -> Value {
  match value {
    Bound::Number(number) => number.into(),
    Bound::Text(text) => text.into(),
  }
}

/// A room's messages, newest first. Each message's text is cut at the value ceiling in a scan of a room, since
/// one message can be longer than everything else the run was asked for, but not where one message was asked for
/// by its number, which is the way to read one whole. The scan as a whole stands under the ceiling, cut to whole
/// messages, and the number to carry on from is the last message named, never the last one fetched, so carrying
/// on skips none.
pub async fn scan(db: &DatabaseConnection, asked: &ScanAsked) -> Result<Vec<String>, DbErr> {
  let mut conditions = vec![r#""chat_id" = $1"#.to_owned()];
  let mut values = vec![Bound::Number(asked.chat_id)];
  let mut bind = |condition: &str, value: Bound| {
    values.push(value);
    conditions.push(condition.replace('?', &format!("${}", values.len())));
  };

  if let Some(id) = asked.id {
    bind(r#""message_id" = ?"#, Bound::Number(id));
  }

  if let Some(before) = asked.before_id {
    bind(r#""message_id" < ?"#, Bound::Number(before));
  }

  if let Some(contains) = &asked.contains {
    bind(r#""text" LIKE ?"#, Bound::Text(format!("%{contains}%")));
  }

  if let Some(sender) = asked.sender {
    bind(r#""sender_id" = ?"#, Bound::Number(sender));
  }

  values.push(Bound::Number(asked.limit.clamp(1, SCAN_MOST)));

  let sql = format!(
    r#"SELECT "message_id", "sender_id", "sender_name", "sent_at", "text" FROM "messages" WHERE {}
       ORDER BY "message_id" DESC LIMIT ${}"#,
    conditions.join(" AND "),
    values.len(),
  );
  let statement = Statement::from_sql_and_values(DbBackend::Postgres, sql, values.into_iter().map(bound));
  let rows = db.query_all(statement).await?;

  if rows.is_empty() {
    return Ok(vec!["(no messages)".to_owned()]);
  }

  let scanned = rows
    .iter()
    .map(|row| {
      Ok(Scanned {
        id: row.try_get("", "message_id")?,
        sender: row.try_get("", "sender_id")?,
        sender_name: row.try_get("", "sender_name")?,
        sent_at: row.try_get("", "sent_at")?,
        text: row.try_get("", "text")?,
      })
    })
    .collect::<Result<Vec<_>, DbErr>>()?;
  let whole = asked.id.is_some();
  let line = |one: &Scanned| {
    let text = one.text.replace('\n', "\\n");
    let sender = one.sender.map_or_else(|| "unknown".to_owned(), |sender| sender.to_string());

    format!(
      "{}\t{}\t{sender}\t{}\t{}",
      one.id,
      instant(one.sent_at),
      one.sender_name,
      if whole { text } else { cap_value(&text, MAX_VALUE_CHARS) },
    )
  };
  let capped = cap_items(
    &scanned,
    |kept| kept.iter().map(line).collect::<Vec<_>>().join("\n"),
    Said::new("messages", "Carry on with the --before-id below."),
  );
  let mut lines: Vec<String> = capped.kept.iter().map(line).collect();
  let last = capped.kept.last().map_or_else(String::new, |one| one.id.to_string());

  lines.extend(capped.note);
  lines.push(format!(
    "-- {} messages (newest first); continue with --before-id {last}",
    count_text(capped.kept.len()),
  ));

  Ok(lines)
}

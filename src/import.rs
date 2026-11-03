//! The old archive's move into the receiver's store (docs/store.md, and the migration plan's table map
//! for TODO #1923): one export directory read, this store written, nothing else touched.
//!
//! The export is the one the item's export hand wrote under
//! `workbench/worker/items/1923/exports/receiver/`: `manifest.json` with each source table's exact
//! count and key, and one NDJSON file a table. The import reads only that directory, refuses a store
//! already holding rows of its tables, and runs whole inside one transaction, so a dry run and a
//! failed run leave the store as it stood.
//!
//! Every table keeps its old name but `telegram_messages`, whose rows become `message_metadata`. A
//! time is epoch seconds in the old archive and epoch milliseconds here, so the columns
//! `m20261020_000007_millisecond_times` multiplies are multiplied as they are written.
//!
//! Two path columns are rewritten: the old archive's `messages.image_path` and
//! `collected_items.stored_path` carry paths under the data root of their day, and those files
//! stand under the same relative path at this deployment's data root. The command names both the
//! stale prefix and the live root, since the import reads its arguments and never the deployment's
//! configuration; only that prefix is rewritten, and a relative path is written as it stands.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::Row;
use sqlx::postgres::{PgPool, PgPoolOptions};

use crate::store::migration::migrate;
use crate::store::pool::connect as sea_connect;

/// The two roots the path rewrite needs: the prefix the old archive's paths carry, and the data
/// root those files stand under here. The command names both, since the import reads its arguments
/// and never the deployment's configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Roots {
  /// The prefix an old path carries, as the caller gives it.
  pub stale: String,
  /// This deployment's data root, whose relative layout the old paths keep.
  pub live: PathBuf,
}

impl Roots {
  pub fn new(stale: impl Into<String>, live: impl Into<PathBuf>) -> Self {
    Self {
      stale: stale.into(),
      live: live.into(),
    }
  }

  /// The old root rewritten to the live one, and every other path as it stands. An empty prefix
  /// rewrites nothing.
  pub fn live_path(&self, said: &str) -> String {
    if self.stale.is_empty() {
      return said.to_owned();
    }

    match said.strip_prefix(&self.stale) {
      Some(rest) => format!("{}{rest}", self.live.display()),
      None => said.to_owned(),
    }
  }
}

/// One value written to the store, already in the target's shape.
#[derive(Clone, Debug, PartialEq)]
pub enum Cell {
  Null,
  Int(i64),
  Text(String),
}

/// One mapped row: the target's columns and the cell each takes.
pub type Mapped = Vec<(&'static str, Cell)>;
/// The rows of one table, in the order they are written.
pub type MappedRows = Vec<Mapped>;

/// One export table's entry in the manifest.
#[derive(Clone, Debug)]
pub struct TableEntry {
  pub file: String,
  pub count: u64,
  pub key: Vec<String>,
}

/// The export directory's manifest.
#[derive(Clone, Debug)]
pub struct Manifest {
  pub snapshot: String,
  pub read_at: String,
  pub tables: HashMap<String, TableEntry>,
}

/// One table of the report: what the export held and what was written.
#[derive(Clone, Debug, PartialEq)]
pub struct Moved {
  pub source: String,
  pub target: String,
  pub exported: u64,
  pub written: u64,
}

/// What an import run did.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
  pub dry_run: bool,
  pub snapshot: String,
  pub read_at: String,
  pub tables: Vec<Moved>,
  pub sample: Sample,
}

/// The count-and-sample compare's result.
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
  pub checked: usize,
  pub failures: Vec<String>,
}

/// A source table's target, its target columns, and the columns that change shape on the way.
pub struct TableMap {
  pub source: &'static str,
  pub target: &'static str,
  /// The target's columns, in the order they are written. `rowid` is never among them: the target
  /// generates its own.
  pub columns: &'static [&'static str],
  /// A target column read from a differently named source column.
  pub renamed: &'static [(&'static str, &'static str)],
  /// Source columns held in seconds and written in milliseconds.
  pub times: &'static [&'static str],
  /// Source columns held as a number and written as text.
  pub as_text: &'static [&'static str],
}

/// A table whose columns are the source's own, with nothing changing shape.
const fn same(source: &'static str, target: &'static str, columns: &'static [&'static str]) -> TableMap {
  TableMap {
    source,
    target,
    columns,
    renamed: &[],
    times: &[],
    as_text: &[],
  }
}

const ALLOWED_CHATS: TableMap = same("allowed_chats", "allowed_chats", &["chat_id"]);
const AVATARS: TableMap = TableMap {
  times: &["updated_at"],
  ..same("avatars", "avatars", &["kind", "subject_id", "photo_id", "path", "updated_at"])
};
const BMO_ROOM_CONTEXTS: TableMap =
  same("bmo_room_contexts", "bmo_room_contexts", &["session_id", "fingerprint"]);
const BMO_ROOMS: TableMap = same("bmo_rooms", "bmo_rooms", &["chat_id"]);
const BMO_SESSION_ROOMS: TableMap =
  same("bmo_session_rooms", "bmo_session_rooms", &["session_id", "chat_id"]);
const CHAT_NOTICES: TableMap = same("chat_notices", "chat_notices", &["chat_id", "kind", "enabled"]);
const CHAT_READS: TableMap = same("chat_reads", "chat_reads", &["chat_id", "last_read_message_id"]);
const CHAT_TITLES: TableMap = same("chat_titles", "chat_titles", &["chat_id", "title"]);
const COLLECTED_ITEMS: TableMap = TableMap {
  times: &["collected_at"],
  ..same(
    "collected_items",
    "collected_items",
    &[
      "id",
      "chat_id",
      "message_id",
      "sender_name",
      "kind",
      "source",
      "stored_path",
      "summary",
      "collected_at",
      "file_unique_id",
    ],
  )
};
const HOLIDAYS: TableMap = same("holidays", "holidays", &["day", "name"]);
const INSTRUCTIONS: TableMap = TableMap {
  times: &["created_at", "delivered_at", "expires_at", "handed_at", "ride_until"],
  ..same(
    "instructions",
    "instructions",
    &[
      "id",
      "chat_id",
      "text",
      "created_at",
      "delivered_at",
      "kind",
      "expires_at",
      "handed_at",
      "restarts",
      "via",
      "events",
      "rides",
      "ride_until",
      "agent",
      "sort",
      "summary",
      "round",
      "run_id",
    ],
  )
};
const MESSAGE_NUMBER_MARKS: TableMap =
  same("message_number_marks", "message_number_marks", &["chat_id", "lowest", "highest"]);
const MESSAGES: TableMap = TableMap {
  times: &["sent_at"],
  ..same(
    "messages",
    "messages",
    &[
      "chat_id",
      "message_id",
      "sender_id",
      "sender_name",
      "sent_at",
      "text",
      "image_path",
      "reply_to_message_id",
      "answer_seconds",
      "round",
      "album",
      "via",
    ],
  )
};
const MUTED_MEMBERS: TableMap = same("muted_members", "muted_members", &["chat_id", "user_id"]);
const NEWS_BRIEFS: TableMap = TableMap {
  times: &["created_at"],
  ..same(
    "news_briefs",
    "news_briefs",
    &[
      "id",
      "chat_id",
      "send_at",
      "lead_minutes",
      "share_hours",
      "enabled",
      "last_day",
      "created_at",
    ],
  )
};
const PEOPLE_READS: TableMap = TableMap {
  times: &["read_at"],
  ..same("people_reads", "people_reads", &["agent", "chat_id", "message_id", "read_at"])
};
const PHOTO_CHATS: TableMap = same("photo_chats", "photo_chats", &["chat_id", "look"]);
const PROVIDER_OWNER_USERS: TableMap = TableMap {
  as_text: &["user_id"],
  ..same("provider_owner_users", "provider_owner_users", &["provider", "chat_id", "user_id"])
};
const PROVIDER_PRIVATE_ROOMS: TableMap = same(
  "provider_private_rooms",
  "provider_private_rooms",
  &["provider", "chat_id", "sender_id"],
);
const PROVIDER_ROOMS: TableMap =
  same("provider_rooms", "provider_rooms", &["provider", "room_id", "chat_id", "title"]);
const PROVIDER_USERS: TableMap = TableMap {
  as_text: &["user_id"],
  ..same("provider_users", "provider_users", &["provider", "user_id", "sender_id"])
};
const RANDOM_DRAW_ROOMS: TableMap =
  same("random_draw_rooms", "random_draw_rooms", &["chat_id", "next_seed", "next_commitment"]);
const RANDOM_DRAWS: TableMap = TableMap {
  times: &["created_at"],
  ..same(
    "random_draws",
    "random_draws",
    &[
      "id",
      "chat_id",
      "via",
      "message_id",
      "mode",
      "shape",
      "spec",
      "label",
      "commitment",
      "seed",
      "result",
      "next_commitment",
      "method",
      "created_at",
    ],
  )
};
const RELOAD_PENDING: TableMap = same("reload_pending", "reload_pending", &["chat_id", "path"]);
const RELOAD_SENT: TableMap = TableMap {
  times: &["at"],
  ..same("reload_sent", "reload_sent", &["chat_id", "at"])
};
const ROOM_SUPERVISORS: TableMap = TableMap {
  renamed: &[("supervisor", "agent")],
  ..same("room_answerers", "room_supervisors", &["chat_id", "supervisor"])
};
const ROOM_WINDOWS: TableMap = TableMap {
  times: &["open_until"],
  ..same("room_windows", "room_windows", &["chat_id", "agent", "open_until"])
};
const RUN_RECORDS: TableMap = TableMap {
  times: &["at"],
  ..same("run_records", "run_records", &["id", "run", "kind", "text", "record_id", "wakes", "at"])
};
const SAY_RELAYS: TableMap = TableMap {
  times: &["created_at", "expires_at", "delivered_at"],
  ..same(
    "say_relays",
    "say_relays",
    &["id", "chat_id", "reply_to", "opening", "created_at", "expires_at", "delivered_at"],
  )
};
const TELEGRAM_MESSAGES: TableMap =
  same("telegram_messages", "message_metadata", &["chat_id", "message_id", "metadata"]);
const SETTINGS: TableMap = same("settings", "settings", &["key", "value"]);

/// Every table this import moves, in the order it writes them.
pub const TABLES: &[TableMap] = &[
  ALLOWED_CHATS,
  AVATARS,
  BMO_ROOM_CONTEXTS,
  BMO_ROOMS,
  BMO_SESSION_ROOMS,
  CHAT_NOTICES,
  CHAT_READS,
  CHAT_TITLES,
  COLLECTED_ITEMS,
  HOLIDAYS,
  INSTRUCTIONS,
  MESSAGE_NUMBER_MARKS,
  MESSAGES,
  MUTED_MEMBERS,
  NEWS_BRIEFS,
  PEOPLE_READS,
  PHOTO_CHATS,
  PROVIDER_OWNER_USERS,
  PROVIDER_PRIVATE_ROOMS,
  PROVIDER_ROOMS,
  PROVIDER_USERS,
  RANDOM_DRAW_ROOMS,
  RANDOM_DRAWS,
  RELOAD_PENDING,
  RELOAD_SENT,
  ROOM_SUPERVISORS,
  ROOM_WINDOWS,
  RUN_RECORDS,
  SAY_RELAYS,
  TELEGRAM_MESSAGES,
  SETTINGS,
];

/// The table a source is written into.
pub fn target_of(source: &str) -> Option<&'static str> {
  TABLES.iter().find(|table| table.source == source).map(|table| table.target)
}

fn text(value: Option<&Value>) -> Option<String> {
  match value {
    Some(Value::String(said)) => Some(said.clone()),
    Some(Value::Null) | None => None,
    Some(other) => Some(other.to_string()),
  }
}

fn int(value: Option<&Value>) -> Option<i64> {
  match value {
    Some(Value::Number(number)) => number.as_i64().or_else(|| number.as_f64().map(|n| n as i64)),
    Some(Value::String(said)) => said.parse().ok(),
    _ => None,
  }
}

/// One source row written in its target's columns.
pub fn row(table: &TableMap, old: &Value, roots: &Roots) -> Vec<(&'static str, Cell)> {
  let mut out = Vec::with_capacity(table.columns.len());
  for column in table.columns {
    let source = table
      .renamed
      .iter()
      .find(|(target, _)| target == column)
      .map(|(_, source)| *source)
      .unwrap_or(*column);
    let value = old.get(source);
    let cell = if table.times.contains(&source) {
      match int(value) {
        Some(seconds) => Cell::Int(seconds * 1000),
        None => Cell::Null,
      }
    } else if table.as_text.contains(&source) {
      match text(value) {
        Some(said) => Cell::Text(said),
        None => Cell::Null,
      }
    } else {
      match value {
        None | Some(Value::Null) => Cell::Null,
        Some(Value::String(said)) => {
          if *column == "image_path" || *column == "stored_path" {
            Cell::Text(roots.live_path(said))
          } else {
            Cell::Text(said.clone())
          }
        }
        Some(Value::Bool(flag)) => Cell::Int(i64::from(*flag)),
        Some(Value::Number(number)) => match number.as_i64() {
          Some(number) => Cell::Int(number),
          None => Cell::Text(number.to_string()),
        },
        Some(other) => Cell::Text(other.to_string()),
      }
    };
    out.push((*column, cell));
  }

  out
}

/// The export directory's manifest.
pub fn read_manifest(dir: &Path) -> Result<Manifest, String> {
  let path = dir.join("manifest.json");
  let said = std::fs::read_to_string(&path)
    .map_err(|error| format!("{} could not be read: {error}", path.display()))?;
  let document: Value =
    serde_json::from_str(&said).map_err(|error| format!("{} is not JSON: {error}", path.display()))?;
  let mut tables = HashMap::new();
  let list = document.get("tables").and_then(Value::as_array).ok_or("the manifest names no tables")?;
  for table in list {
    let source = table
      .get("source")
      .and_then(Value::as_str)
      .ok_or("a manifest table names no source")?
      .to_owned();
    let file = table
      .get("file")
      .and_then(Value::as_str)
      .ok_or("a manifest table names no file")?
      .to_owned();
    let count = table.get("count").and_then(Value::as_u64).ok_or("a manifest table names no count")?;
    let key: Vec<String> = table
      .get("key")
      .and_then(Value::as_array)
      .map(|keys| keys.iter().filter_map(Value::as_str).map(str::to_owned).collect())
      .unwrap_or_default();
    tables.insert(source, TableEntry { file, count, key });
  }

  Ok(Manifest {
    snapshot: document.get("snapshot").and_then(Value::as_str).unwrap_or_default().to_owned(),
    read_at: document.get("readAt").and_then(Value::as_str).unwrap_or_default().to_owned(),
    tables,
  })
}

/// One table's rows, each checked against the manifest's count as it is read.
pub fn rows(dir: &Path, manifest: &Manifest, source: &str) -> Result<Vec<Value>, String> {
  let entry = manifest
    .tables
    .get(source)
    .ok_or_else(|| format!("the manifest has no table {source}"))?;
  let path = dir.join(&entry.file);
  let said = std::fs::read_to_string(&path)
    .map_err(|error| format!("{} could not be read: {error}", path.display()))?;
  let mut out = Vec::new();
  for (number, line) in said.lines().enumerate() {
    if line.trim().is_empty() {
      continue;
    }
    let row: Value = serde_json::from_str(line)
      .map_err(|error| format!("{} line {} is not JSON: {error}", path.display(), number + 1))?;
    out.push(row);
  }
  if out.len() as u64 != entry.count {
    return Err(format!(
      "{}: the manifest says {} rows and the file holds {}",
      path.display(),
      entry.count,
      out.len()
    ));
  }

  Ok(out)
}

/// The export read and mapped, before anything is written.
pub struct Prepared {
  pub manifest: Manifest,
  /// Per source table, its target and its rows in the target's columns.
  pub tables: Vec<(&'static str, &'static str, MappedRows)>,
}

/// Reads the export and maps every row to its target's shape.
pub fn prepare(export: &Path, roots: &Roots) -> Result<Prepared, String> {
  let manifest = read_manifest(export)?;
  let mut tables = Vec::new();
  for table in TABLES {
    let old = rows(export, &manifest, table.source)?;
    let mapped: Vec<_> = old.iter().map(|old| row(table, old, roots)).collect();
    tables.push((table.source, table.target, mapped));
  }

  Ok(Prepared { manifest, tables })
}

/// The target's columns and their types, so a value can be cast from its text to the column's own
/// type: a null has no type of its own, and the server refuses an untyped one for a numbered column.
async fn types(pool: &PgPool, table: &str) -> Result<HashMap<String, String>, String> {
  let rows = sqlx::query(
    "SELECT column_name, data_type FROM information_schema.columns \
     WHERE table_schema = 'public' AND table_name = $1",
  )
  .bind(table)
  .fetch_all(pool)
  .await
  .map_err(|error| format!("{table}'s columns could not be read: {error}"))?;
  let mut out = HashMap::new();
  for row in rows {
    out.insert(row.get::<String, _>("column_name"), row.get::<String, _>("data_type"));
  }
  if out.is_empty() {
    return Err(format!("{table} holds no columns"));
  }

  Ok(out)
}

/// Every value bound as text and cast to its column's type.
fn bind<'q>(
  mut query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
  cell: &Cell,
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
  let said = match cell {
    Cell::Null => None,
    Cell::Int(number) => Some(number.to_string()),
    Cell::Text(said) => Some(said.clone()),
  };
  query = query.bind(said);
  query
}

async fn insert(
  tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
  table: &str,
  kinds: &HashMap<String, String>,
  row: &[(&'static str, Cell)],
) -> Result<(), String> {
  let columns = row.iter().map(|(column, _)| format!("\"{column}\"")).collect::<Vec<_>>().join(", ");
  let values = row
    .iter()
    .enumerate()
    .map(|(position, (column, _))| {
      let kind = kinds.get(*column).map(String::as_str).unwrap_or("text");
      format!("CAST(${} AS {kind})", position + 1)
    })
    .collect::<Vec<_>>()
    .join(", ");
  let sql = format!("INSERT INTO \"{table}\" ({columns}) VALUES ({values})");
  let mut query = sqlx::query(&sql);
  for (_, cell) in row {
    query = bind(query, cell);
  }
  query
    .execute(&mut **tx)
    .await
    .map_err(|error| format!("{table} could not be written: {error}"))?;

  Ok(())
}

/// Refuses a store that already holds rows of any table this import writes.
pub async fn refused_if_holding(pool: &PgPool) -> Result<(), String> {
  let mut holding = Vec::new();
  for table in TABLES {
    let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM \"{}\"", table.target))
      .fetch_one(pool)
      .await
      .map_err(|error| format!("{} could not be counted: {error}", table.target))?;
    if count > 0 {
      holding.push(format!("{} holds {count}", table.target));
    }
  }
  if holding.is_empty() {
    Ok(())
  } else {
    Err(format!(
      "the store already holds rows of its tables, so nothing was written: {}",
      holding.join(", ")
    ))
  }
}

fn digest(said: &str) -> String {
  Sha256::digest(said.as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect()
}

fn key_text(row: &[(&'static str, Cell)], key: &[String]) -> String {
  key
    .iter()
    .map(|column| {
      row
        .iter()
        .find(|(held, _)| held == column)
        .map(|(_, cell)| format!("{cell:?}"))
        .unwrap_or_default()
    })
    .collect::<Vec<_>>()
    .join("|")
}

/// The rows the compare checks: every row of a table holding twenty or fewer, else the first, the
/// last and up to twenty by the smallest SHA-256 of the key's text.
fn sample<'a>(rows: &'a [Mapped], key: &[String]) -> Vec<&'a Mapped> {
  if rows.len() <= 20 {
    return rows.iter().collect();
  }
  let mut ordered: Vec<(String, &Mapped)> =
    rows.iter().map(|row| (digest(&key_text(row, key)), row)).collect();
  ordered.sort_by(|left, right| left.0.cmp(&right.0));
  let mut picked: Vec<&Vec<(&'static str, Cell)>> = vec![&rows[0], &rows[rows.len() - 1]];
  for (_, row) in ordered.into_iter().take(20) {
    if !picked.iter().any(|held| std::ptr::eq(*held, row)) {
      picked.push(row);
    }
  }

  picked
}

async fn compare(pool: &PgPool, prepared: &Prepared) -> Sample {
  let mut failures = Vec::new();
  let mut checked = 0;
  for (source, target, rows) in &prepared.tables {
    let entry = match prepared.manifest.tables.get(*source) {
      Some(entry) => entry,
      None => continue,
    };
    let count: Result<i64, _> =
      sqlx::query_scalar(&format!("SELECT COUNT(*) FROM \"{target}\"")).fetch_one(pool).await;
    match count {
      Ok(held) if held as u64 == entry.count => {}
      Ok(held) => {
        failures.push(format!("{source}: the export holds {} rows and {target} holds {held}", entry.count))
      }
      Err(error) => failures.push(format!("{source}: {target} could not be counted: {error}")),
    }
    if entry.key.is_empty() {
      failures.push(format!("{source}: the export records no key to sample by"));
      continue;
    }
    let kinds = match types(pool, target).await {
      Ok(kinds) => kinds,
      Err(error) => {
        failures.push(error);
        continue;
      }
    };
    for row in sample(rows, &entry.key) {
      checked += 1;
      let mut sql = String::from("SELECT * FROM \"");
      sql.push_str(target);
      sql.push_str("\" WHERE ");
      for (position, column) in entry.key.iter().enumerate() {
        if position > 0 {
          sql.push_str(" AND ");
        }
        let kind = kinds.get(column).map(String::as_str).unwrap_or("text");
        sql.push_str(&format!("\"{column}\" = CAST(${} AS {kind})", position + 1));
      }
      let mut query = sqlx::query(&sql);
      let mut bound = true;
      for column in &entry.key {
        match row.iter().find(|(held, _)| held == column).map(|(_, cell)| cell) {
          Some(cell) => query = bind(query, cell),
          None => {
            failures.push(format!("{source}: the mapped row carries no {column}"));
            bound = false;
            break;
          }
        }
      }
      if !bound {
        continue;
      }
      match query.fetch_optional(pool).await {
        Ok(Some(held)) => {
          for (column, cell) in row {
            let actual = match cell {
              Cell::Null => held
                .try_get::<Option<String>, _>(*column)
                .map(|said| said.map_or(Cell::Null, Cell::Text)),
              Cell::Int(_) => held
                .try_get::<i64, _>(*column)
                .map(Cell::Int)
                .or_else(|_| held.try_get::<String, _>(*column).map(Cell::Text)),
              Cell::Text(_) => held
                .try_get::<Option<String>, _>(*column)
                .map(|said| said.map_or(Cell::Null, Cell::Text)),
            };
            match actual {
              Ok(actual) if &actual == cell => {}
              Ok(actual) => failures
                .push(format!("{source} -> {target}: {column} was written {cell:?} but is {actual:?}")),
              Err(error) => {
                failures.push(format!("{source} -> {target}: {column} could not be read: {error}"))
              }
            }
          }
        }
        Ok(None) => failures.push(format!("{source} -> {target}: a sampled row is missing")),
        Err(error) => failures.push(format!("{source} -> {target}: the sample could not be read: {error}")),
      }
    }
  }

  Sample { checked, failures }
}

/// Runs the import: reads the export, refuses a store holding rows, writes every row, and compares
/// what it wrote. A dry run rolls the transaction back and reports what would have been written.
pub async fn run(database: &str, export: &Path, dry_run: bool, roots: &Roots) -> Result<Report, String> {
  let prepared = prepare(export, roots)?;
  // The repository's own migrations, run over the store before anything is read into it.
  let sea = sea_connect(database, PgPoolOptions::new().max_connections(2))
    .await
    .map_err(|error| format!("{database}: {error}"))?;
  migrate(&sea).await.map_err(|error| format!("the migrations: {error}"))?;
  sea.close().await.map_err(|error| error.to_string())?;

  let pool = PgPoolOptions::new()
    .max_connections(1)
    .connect(database)
    .await
    .map_err(|error| format!("{database} could not be opened: {error}"))?;
  refused_if_holding(&pool).await?;

  // Every target's column types, read before the writing transaction takes the one connection.
  let mut kinds: HashMap<&str, HashMap<String, String>> = HashMap::new();
  for (_, target, _) in &prepared.tables {
    kinds.insert(target, types(&pool, target).await?);
  }

  let mut tx = pool
    .begin()
    .await
    .map_err(|error| format!("the transaction could not be opened: {error}"))?;
  for (_source, target, rows) in &prepared.tables {
    let table_kinds = kinds.get(target).expect("a target's types");
    for row in rows {
      insert(&mut tx, target, table_kinds, row).await?;
    }
  }

  if dry_run {
    tx.rollback()
      .await
      .map_err(|error| format!("the dry run could not be rolled back: {error}"))?;
  } else {
    tx.commit()
      .await
      .map_err(|error| format!("the import could not be committed: {error}"))?;
  }

  let sample = if dry_run {
    Sample {
      checked: 0,
      failures: Vec::new(),
    }
  } else {
    compare(&pool, &prepared).await
  };

  let tables: Vec<Moved> = prepared
    .tables
    .iter()
    .map(|(source, target, rows)| Moved {
      source: (*source).to_owned(),
      target: (*target).to_owned(),
      exported: prepared.manifest.tables.get(*source).map(|entry| entry.count).unwrap_or(0),
      written: rows.len() as u64,
    })
    .collect();
  pool.close().await;

  Ok(Report {
    dry_run,
    snapshot: prepared.manifest.snapshot.clone(),
    read_at: prepared.manifest.read_at.clone(),
    tables,
    sample,
  })
}

/// The count-and-sample compare alone, against a store the import wrote.
pub async fn verify(database: &str, export: &Path, roots: &Roots) -> Result<Sample, String> {
  let prepared = prepare(export, roots)?;
  let pool = PgPoolOptions::new()
    .max_connections(1)
    .connect(database)
    .await
    .map_err(|error| format!("{database} could not be opened: {error}"))?;
  let sample = compare(&pool, &prepared).await;
  pool.close().await;

  Ok(sample)
}

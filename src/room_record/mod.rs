//! What a room's record is, as the removal of its registration reads it (docs/receiver.md, 「Removing a room's
//! registration」). The tables holding a room's number in a `chat_id` column are read off the database's own
//! schema; everything else a room holds — rows that stay, rows reached by another column, its folders, the rooms
//! that may not go and what a removal warns of — is declared by the code that owns it, beside that code, through
//! `room_holding!`. Nothing here names a table, a store or a road.
//!
//! The database and a copy of it are read alike, a condition naming the room as `$1`, which both take.

pub mod forget;
pub mod setting;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use receiver_codes::{RoomRemovalRefusal, RoomRemovalWarning};
use sqlx::{AnyConnection, Connection, Row};

use crate::Fault;
use crate::receiver::Receiver;
pub use forget::RoomForget;
pub use setting::RoomSetting;

/// How a condition binds the room: as its number, or as text where the column holds text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum As {
  Number,
  Text,
}

/// Rows of one table, picked by a condition naming the room as `$1`.
#[derive(Clone, Copy, Debug)]
pub struct Rows {
  pub table: &'static str,
  pub condition: &'static str,
  pub room: As,
  /// The tables the condition reads beside its own, which a database must hold for it to be asked.
  pub reads: &'static [&'static str],
}

/// What a removal is asked to judge a room by: the receiver, and the room.
pub struct Facing<'a> {
  pub receiver: &'a Receiver,
  pub room: i64,
}

/// What a declaration says of the room, or nothing.
pub type Says<'a, T> = Pin<Box<dyn Future<Output = Result<Option<T>, Fault>> + Send + 'a>>;
/// The folders a declaration holds for the room, those that stand.
pub type Found<'a> = Pin<Box<dyn Future<Output = Result<Vec<PathBuf>, Fault>> + Send + 'a>>;
/// What removing one folder comes to: nothing, or why it was not removed.
pub type Removing<'a> = Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>>;
/// How a folder is removed where its owner says how, beside the code that writes it.
pub type Removes = for<'a> fn(&'a Receiver, &'a Path) -> Removing<'a>;

/// One thing a room holds that its `chat_id` columns do not say, declared by the code that owns it.
pub enum RoomHolding {
  /// A table whose rows outlive the room: its rows are counted and left, the columns named cleared on them.
  /// `column` is the column naming the room.
  Kept {
    rows: Rows,
    column: &'static str,
    cleared: &'static [&'static str],
    why: &'static str,
  },
  /// Rows of a swept table that stay: the room's rows there go but those matching `stays`.
  Stays {
    table: &'static str,
    stays: &'static str,
    why: &'static str,
  },
  /// Rows going with the room beyond its `chat_id` columns, taken before those: `column` is a column naming the
  /// room that this accounts for, and `files` a column holding the path of a file that goes with each row.
  Goes {
    rows: Rows,
    column: Option<&'static str>,
    files: Option<&'static str>,
    why: &'static str,
  },
  /// Folders kept for the room, removed after the rows: by `removes` where its owner says how, taken away whole
  /// otherwise. `named` says the confirmation names each of their files, not only how many there are.
  Folder {
    what: &'static str,
    of: for<'a> fn(&'a Facing<'a>) -> Found<'a>,
    removes: Option<Removes>,
    named: bool,
  },
  /// A refusal: the refusal it says stops the removal.
  Refuses {
    what: &'static str,
    refuses: for<'a> fn(&'a Facing<'a>) -> Says<'a, RoomRemovalRefusal>,
  },
  /// A warning the confirmation carries.
  Warns {
    what: &'static str,
    warns: for<'a> fn(&'a Facing<'a>) -> Says<'a, RoomRemovalWarning>,
  },
}

inventory::collect!(RoomHolding);

/// Declares something a room holds, beside the code that owns it.
#[macro_export]
macro_rules! room_holding {
  ($holding:expr) => {
    inventory::submit! { $holding }
  };
}

/// Every declaration linked into this binary.
pub fn holdings() -> impl Iterator<Item = &'static RoomHolding> {
  inventory::iter::<RoomHolding>.into_iter()
}

/// Whether a column's name says it holds a room's number.
pub fn names_a_room(column: &str) -> bool {
  column == "chat_id" || column.starts_with("chat_") || column.contains("_chat")
}

fn postgres(database: &AnyConnection) -> bool {
  database.backend_name().to_ascii_lowercase().starts_with("postgres")
}

/// Every table and column of the database, tables alone left out of views.
pub async fn columns(database: &mut AnyConnection) -> Result<Vec<(String, String)>, sqlx::Error> {
  let sql = if postgres(database) {
    "SELECT CAST(c.table_name AS TEXT) AS t, CAST(c.column_name AS TEXT) AS c FROM information_schema.columns c
       JOIN information_schema.tables b ON b.table_schema = c.table_schema AND b.table_name = c.table_name
       WHERE c.table_schema = current_schema() AND b.table_type = 'BASE TABLE'
       ORDER BY c.table_name, c.ordinal_position"
  } else {
    "SELECT m.name AS t, p.name AS c FROM sqlite_master m, pragma_table_info(m.name) p
       WHERE m.type = 'table' AND m.name NOT LIKE 'sqlite_%' ORDER BY m.name, p.cid"
  };
  let rows = sqlx::query(sql).fetch_all(&mut *database).await?;

  rows.iter().map(|row| Ok((row.try_get("t")?, row.try_get("c")?))).collect()
}

/// The tables of the database holding a `chat_id` column, by name.
pub async fn chat_tables(database: &mut AnyConnection) -> Result<Vec<String>, sqlx::Error> {
  let mut tables: Vec<String> = columns(database)
    .await?
    .into_iter()
    .filter(|(_, column)| column == "chat_id")
    .map(|(table, _)| table)
    .collect();

  tables.dedup();
  Ok(tables)
}

/// What a removal does with one pick's rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
  Take,
  Clear(&'static [&'static str]),
  Keep,
}

/// The rows of one table a removal takes, clears or keeps, with the reason a declaration gives.
#[derive(Clone, Debug)]
pub struct Pick {
  pub table: String,
  pub act: Act,
  pub why: Option<&'static str>,
  pub files: Option<&'static str>,
  condition: String,
  room: As,
}

/// How many rows of one pick went, or would go.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Counted {
  pub table: String,
  pub act: Act,
  pub why: Option<&'static str>,
  pub rows: i64,
}

/// A room's record in one database: the picks in the order they are written, and the settings that name it.
#[derive(Clone, Debug, Default)]
pub struct Record {
  pub picks: Vec<Pick>,
  pub settings: setting::Settled,
}

fn holds(tables: &HashSet<String>, rows: &Rows) -> bool {
  tables.contains(rows.table) && rows.reads.iter().all(|read| tables.contains(*read))
}

fn of_rows(rows: &Rows, act: Act, why: &'static str, files: Option<&'static str>) -> Pick {
  Pick {
    table: rows.table.to_owned(),
    act,
    why: Some(why),
    files,
    condition: rows.condition.to_owned(),
    room: rows.room,
  }
}

/// The picks of a room's record in a database, as its schema and the declarations say: what goes beyond the
/// `chat_id` columns first, then every table holding one, but those kept, then what is kept or cleared — the rows
/// of a swept table that stay among them.
pub fn picks(tables: &HashSet<String>, chat_tables: &[String]) -> Vec<Pick> {
  let mut picks = vec![];
  let mut kept = HashSet::new();

  for holding in holdings() {
    match holding {
      RoomHolding::Goes {
        rows, files, why, ..
      } if holds(tables, rows) => {
        picks.push(of_rows(rows, Act::Take, why, *files));
      }
      RoomHolding::Kept { rows, .. } => {
        kept.insert(rows.table);
      }
      _ => {}
    }
  }

  for table in chat_tables.iter().filter(|table| !kept.contains(table.as_str())) {
    let stays: Vec<&str> = holdings()
      .filter_map(|holding| match holding {
        RoomHolding::Stays {
          table: of, stays, ..
        } if of == table => Some(*stays),
        _ => None,
      })
      .collect();
    let condition = stays
      .iter()
      .fold("chat_id = $1".to_owned(), |condition, stays| format!("{condition} AND NOT ({stays})"));

    picks.push(Pick {
      table: table.clone(),
      act: Act::Take,
      why: None,
      files: None,
      condition,
      room: As::Number,
    });
  }

  for holding in holdings() {
    if let RoomHolding::Stays { table, stays, why } = holding
      && chat_tables.iter().any(|of| of == table)
    {
      picks.push(Pick {
        table: (*table).to_owned(),
        act: Act::Keep,
        why: Some(why),
        files: None,
        condition: format!("chat_id = $1 AND ({stays})"),
        room: As::Number,
      });
    }
  }

  for holding in holdings() {
    if let RoomHolding::Kept {
      rows, cleared, why, ..
    } = holding
      && holds(tables, rows)
    {
      let act = if cleared.is_empty() { Act::Keep } else { Act::Clear(cleared) };

      picks.push(of_rows(rows, act, why, None));
    }
  }

  picks
}

/// A room's record in a database, read off its schema, its settings and the declarations.
pub async fn record(database: &mut AnyConnection, room: i64) -> Result<Record, sqlx::Error> {
  let columns = columns(database).await?;
  let tables: HashSet<String> = columns.iter().map(|(table, _)| table.clone()).collect();
  let chat_tables = chat_tables(database).await?;
  let settings = if tables.contains("settings") {
    setting::settled(database, room).await?
  } else {
    setting::Settled::default()
  };

  Ok(Record {
    picks: picks(&tables, &chat_tables),
    settings,
  })
}

type Query<'q> = sqlx::query::Query<'q, sqlx::Any, sqlx::any::AnyArguments<'q>>;

fn bind<'q>(query: Query<'q>, pick: &Pick, room: i64) -> Query<'q> {
  match pick.room {
    As::Number => query.bind(room),
    As::Text => query.bind(room.to_string()),
  }
}

fn bound<'q>(sql: &'q str, pick: &Pick, room: i64) -> Query<'q> {
  bind(sqlx::query(sql), pick, room)
}

fn counted(pick: &Pick, rows: i64) -> Counted {
  Counted {
    table: pick.table.clone(),
    act: pick.act,
    why: pick.why,
    rows,
  }
}

/// How many rows each pick holds, nothing written: a row an earlier pick of its table takes is counted there
/// alone, since the removal takes it once, so each count is what the removal's own write of that pick touches.
pub async fn count(
  database: &mut AnyConnection,
  record: &Record,
  room: i64,
) -> Result<Vec<Counted>, sqlx::Error> {
  let mut all = vec![];

  for (at, pick) in record.picks.iter().enumerate() {
    let earlier: Vec<&Pick> = record.picks[..at]
      .iter()
      .filter(|earlier| earlier.table == pick.table && earlier.act == Act::Take)
      .collect();
    // Each earlier condition names the room by a number of its own, since it may name it as text.
    let condition = earlier.iter().enumerate().fold(pick.condition.clone(), |condition, (n, earlier)| {
      let earlier = earlier.condition.replace("$1", &format!("${}", n + 2));

      format!("{condition} AND NOT COALESCE(({earlier}), 1 = 0)")
    });
    let sql = format!("SELECT count(*) AS n FROM {} WHERE {condition}", pick.table);
    let query = earlier
      .iter()
      .fold(bound(&sql, pick, room), |query, earlier| bind(query, earlier, room));
    let rows: i64 = query.fetch_one(&mut *database).await?.try_get("n")?;

    all.push(counted(pick, rows));
  }

  Ok(all)
}

/// The files the rows a removal takes point to, by the column their declaration names.
pub async fn files(
  database: &mut AnyConnection,
  record: &Record,
  room: i64,
) -> Result<Vec<String>, sqlx::Error> {
  let mut all = vec![];

  for pick in &record.picks {
    let Some(column) = pick.files else { continue };
    let sql = format!(
      "SELECT {column} AS f FROM {} WHERE {} AND {column} IS NOT NULL",
      pick.table, pick.condition
    );

    for row in bound(&sql, pick, room).fetch_all(&mut *database).await? {
      all.push(row.try_get("f")?);
    }
  }

  Ok(all)
}

/// Takes a room's record in one write: the rows taken, the columns cleared and the settings that name it. Says how
/// many rows each pick touched; a kept pick touches none.
pub async fn take(
  database: &mut AnyConnection,
  record: &Record,
  room: i64,
) -> Result<Vec<Counted>, sqlx::Error> {
  let mut written = database.begin().await?;
  let mut all = vec![];

  for pick in &record.picks {
    let sql = match pick.act {
      Act::Take => format!("DELETE FROM {} WHERE {}", pick.table, pick.condition),
      Act::Clear(columns) => {
        let set: Vec<String> = columns.iter().map(|column| format!("{column} = NULL")).collect();

        format!("UPDATE {} SET {} WHERE {}", pick.table, set.join(", "), pick.condition)
      }
      Act::Keep => continue,
    };
    let touched = bound(&sql, pick, room).execute(&mut *written).await?.rows_affected();

    all.push(counted(pick, i64::try_from(touched).unwrap_or(i64::MAX)));
  }

  setting::take(&mut written, &record.settings, room).await?;
  written.commit().await?;

  Ok(all)
}

/// Every refusal the declarations say of the room.
pub async fn refusals(facing: &Facing<'_>) -> Result<Vec<RoomRemovalRefusal>, Fault> {
  let mut said = vec![];

  for holding in holdings() {
    if let RoomHolding::Refuses { refuses, .. } = holding
      && let Some(refusal) = refuses(facing).await?
    {
      said.push(refusal);
    }
  }

  Ok(said)
}

/// Every warning the declarations say of the room.
pub async fn warnings(facing: &Facing<'_>) -> Result<Vec<RoomRemovalWarning>, Fault> {
  let mut said = vec![];

  for holding in holdings() {
    if let RoomHolding::Warns { warns, .. } = holding
      && let Some(warning) = warns(facing).await?
    {
      said.push(warning);
    }
  }

  Ok(said)
}

/// One folder kept for the room, and how it is removed.
pub struct Held {
  pub what: &'static str,
  pub path: PathBuf,
  pub removes: Option<Removes>,
  pub named: bool,
}

impl Held {
  /// The folder removed as its owner says, taken away whole where it says nothing; one already gone is no failure.
  pub async fn remove(&self, receiver: &Receiver) -> Result<(), String> {
    if let Some(removes) = self.removes {
      return removes(receiver, &self.path).await;
    }

    match tokio::fs::remove_dir_all(&self.path).await {
      Err(why) if why.kind() != std::io::ErrorKind::NotFound => Err(why.to_string()),
      _ => Ok(()),
    }
  }
}

/// Every folder the declarations hold for the room that stands.
pub async fn folders(facing: &Facing<'_>) -> Result<Vec<Held>, Fault> {
  let mut held = vec![];

  for holding in holdings() {
    if let RoomHolding::Folder {
      what,
      of,
      removes,
      named,
    } = holding
    {
      for path in of(facing).await? {
        held.push(Held {
          what,
          path,
          removes: *removes,
          named: *named,
        });
      }
    }
  }

  Ok(held)
}

//! The guard over a room's record (docs/receiver.md, 「Removing a room's registration」): on the migrated database,
//! every column naming a room is swept by the schema or accounted for by a declaration, every declaration names a
//! table and columns that stand, and every pick reads and writes as written; a copy of the database is read off its
//! own schema alike; and a setting naming a room is built, matched and written by its declaration's shape.

use std::collections::HashSet;
use std::path::Path;

use bmo_receiver::room_record::setting::{ROOM as ROOM_PART, RoomSetting, declared};
use bmo_receiver::room_record::{self, Act, RoomHolding, names_a_room};
use sqlx::{AnyConnection, Connection, Row};

use crate::integration::common::Scratch;

const ROOM: i64 = -4_200;

// A kind of setting naming a room, as the code that writes one declares it.
static SHAPE: RoomSetting = RoomSetting {
  prefix: "shape_of:",
  parts: &["agent", ROOM_PART],
  why: "a test's",
};

bmo_receiver::room_setting!(SHAPE);

async fn database(address: &str) -> AnyConnection {
  sqlx::any::install_default_drivers();

  AnyConnection::connect(address).await.unwrap()
}

// Each declared table with the columns the declaration names on it.
fn named() -> Vec<(&'static str, Vec<&'static str>)> {
  room_record::holdings()
    .filter_map(|holding| match holding {
      RoomHolding::Kept {
        rows,
        column,
        cleared,
        ..
      } => Some((rows.table, [&[*column][..], cleared].concat())),
      RoomHolding::Goes {
        rows,
        column,
        files,
        ..
      } => Some((rows.table, column.iter().chain(files.iter()).copied().collect())),
      RoomHolding::Stays { table, .. } => Some((*table, vec!["chat_id"])),
      _ => None,
    })
    .collect()
}

#[tokio::test]
async fn every_column_naming_a_room_is_swept_or_declared() {
  let scratch = Scratch::new().await;
  let mut db = database(&scratch.address()).await;
  let columns = room_record::columns(&mut db).await.unwrap();
  let accounted: HashSet<(&str, &str)> = room_record::holdings()
    .filter_map(|holding| match holding {
      RoomHolding::Kept { rows, column, .. } => Some((rows.table, *column)),
      RoomHolding::Goes {
        rows,
        column: Some(column),
        ..
      } => Some((rows.table, *column)),
      _ => None,
    })
    .collect();

  assert!(room_record::chat_tables(&mut db).await.unwrap().iter().any(|table| table == "messages"));

  for (table, column) in columns.iter().filter(|(_, column)| names_a_room(column)) {
    assert!(
      column == "chat_id" || accounted.contains(&(table.as_str(), column.as_str())),
      "{table}.{column} names a room: a declaration beside its owner says whether it goes, stays or is cleared"
    );
  }
}

#[tokio::test]
async fn every_declaration_names_what_stands() {
  let scratch = Scratch::new().await;
  let mut db = database(&scratch.address()).await;
  let columns: HashSet<(String, String)> =
    room_record::columns(&mut db).await.unwrap().into_iter().collect();
  let tables: HashSet<&str> = columns.iter().map(|(table, _)| table.as_str()).collect();

  for (table, named) in named() {
    assert!(tables.contains(table), "{table} is declared and stands");

    for column in named {
      assert!(
        columns.contains(&(table.to_owned(), column.to_owned())),
        "{table}.{column} is declared and stands"
      );
    }
  }

  for holding in room_record::holdings() {
    match holding {
      RoomHolding::Kept { rows, why, .. } | RoomHolding::Goes { rows, why, .. } => {
        assert!(
          !why.is_empty() && rows.condition.contains("$1"),
          "{} names the room and says why",
          rows.table
        );

        for read in rows.reads {
          assert!(tables.contains(read), "{} reads {read}, which stands", rows.table);
        }
      }
      RoomHolding::Stays { table, stays, why } => assert!(!stays.is_empty() && !why.is_empty(), "{table}"),
      RoomHolding::Folder { what, .. }
      | RoomHolding::Refuses { what, .. }
      | RoomHolding::Warns { what, .. } => {
        assert!(!what.is_empty());
      }
    }
  }
}

#[tokio::test]
async fn every_pick_reads_and_is_taken_in_one_write() {
  let scratch = Scratch::new().await;

  scratch.message(ROOM, 1, 7).await;
  scratch.message(ROOM + 1, 1, 7).await;
  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value") VALUES ($1, '1'), ($2, 'x')"#,
      vec![format!("shape_of:agent-a:{ROOM}").into(), format!("plain:{ROOM}").into()],
    )
    .await;

  let mut db = database(&scratch.address()).await;
  let record = room_record::record(&mut db, ROOM).await.unwrap();

  assert_eq!(record.settings.taken, [format!("shape_of:agent-a:{ROOM}")]);
  assert_eq!(record.settings.undeclared, [format!("plain:{ROOM}")]);

  let counted = room_record::count(&mut db, &record, ROOM).await.unwrap();

  assert_eq!(
    counted
      .iter()
      .filter(|counted| counted.table == "messages")
      .map(|counted| counted.rows)
      .sum::<i64>(),
    1
  );

  room_record::files(&mut db, &record, ROOM).await.unwrap();
  room_record::take(&mut db, &record, ROOM).await.unwrap();

  assert_eq!(
    scratch
      .count(r#"SELECT count(*) FROM "messages" WHERE "chat_id" = $1"#, vec![ROOM.into()])
      .await,
    0
  );
  assert_eq!(
    scratch
      .count(r#"SELECT count(*) FROM "messages" WHERE "chat_id" = $1"#, vec![(ROOM + 1).into()])
      .await,
    1
  );
  assert_eq!(
    scratch
      .count(
        r#"SELECT count(*) FROM "settings" WHERE "key" LIKE $1"#,
        vec![format!("%:{ROOM}").into()]
      )
      .await,
    1
  );
}

// A SQLite copy holding some of the tables, each with only the columns the declarations read.
async fn copy(path: &Path) -> AnyConnection {
  let mut made = database(&format!("sqlite://{}?mode=rwc", path.display())).await;

  for sql in [
    "CREATE TABLE messages (chat_id INTEGER, message_id INTEGER)",
    "CREATE TABLE avatars (kind TEXT, subject_id INTEGER, path TEXT)",
    "CREATE TABLE settings (key TEXT, value TEXT)",
    "INSERT INTO messages VALUES (-55, 1), (-55, 2), (-66, 1)",
    "INSERT INTO avatars VALUES ('room', -55, '/avatars/room/-55.jpg'), ('person', -55, '/avatars/person/-55.jpg')",
    "INSERT INTO settings VALUES ('shape_of:agent-a:-55', '1'), ('shape_of:agent-a:-555', '1'), ('plain:-55', 'x')",
  ] {
    sqlx::query(sql).execute(&mut made).await.unwrap();
  }

  made
}

async fn strings(db: &mut AnyConnection, sql: &str) -> Vec<String> {
  sqlx::query(sql)
    .fetch_all(&mut *db)
    .await
    .unwrap()
    .iter()
    .map(|row| row.try_get::<String, _>(0).unwrap())
    .collect()
}

#[tokio::test]
async fn a_copy_is_read_off_its_own_schema_and_its_record_taken_in_one_write() {
  let dir = tempfile::tempdir().unwrap();
  let mut db = copy(&dir.path().join("copy.db")).await;

  assert_eq!(room_record::chat_tables(&mut db).await.unwrap(), ["messages"]);

  let record = room_record::record(&mut db, -55).await.unwrap();

  assert_eq!(record.settings.taken, ["shape_of:agent-a:-55"]);
  assert_eq!(record.settings.undeclared, ["plain:-55"]);
  // The room's picture goes, the file named on its row with it, and a person's of the same number stays.
  assert!(record.picks.iter().any(|pick| pick.table == "avatars" && pick.act == Act::Take));
  assert_eq!(room_record::files(&mut db, &record, -55).await.unwrap(), ["/avatars/room/-55.jpg"]);

  room_record::take(&mut db, &record, -55).await.unwrap();

  assert_eq!(strings(&mut db, "SELECT CAST(chat_id AS TEXT) FROM messages").await, ["-66"]);
  assert_eq!(strings(&mut db, "SELECT kind FROM avatars").await, ["person"]);
  assert_eq!(
    strings(&mut db, "SELECT key FROM settings ORDER BY key").await,
    ["plain:-55", "shape_of:agent-a:-555"]
  );
}

#[test]
fn a_key_is_built_matched_and_written_as_sql_by_its_shape() {
  assert_eq!(SHAPE.key(-7, &["agent-a"]), "shape_of:agent-a:-7");
  assert!(SHAPE.matches("shape_of:agent-a:-7", -7));
  assert!(!SHAPE.matches("shape_of:agent-a:-7", 7));
  assert!(!SHAPE.matches("shape_of:-7", -7));
  assert!(!SHAPE.matches("shape_of:agent-a:-7:more", -7));
  assert_eq!(SHAPE.sql(&["$1", "m.chat_id"]), "'shape_of:' || $1 || ':' || m.chat_id");
  assert_eq!(SHAPE.like(&[Some("agent-a"), None]), "shape_of:agent-a:%");
}

#[test]
fn no_declared_prefix_begins_another() {
  let prefixes: Vec<&str> = declared().map(|setting| setting.prefix).collect();

  assert!(!prefixes.is_empty());

  for (at, one) in prefixes.iter().enumerate() {
    assert!(one.ends_with(':'), "{one} ends in a colon");
    assert!(
      declared().any(|setting| setting.prefix == *one && setting.parts.contains(&ROOM_PART)),
      "{one} names a room"
    );

    for (other_at, other) in prefixes.iter().enumerate() {
      assert!(at == other_at || !other.starts_with(one), "{one} and {other} overlap");
    }
  }
}

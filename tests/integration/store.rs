//! The receiver's database as its migrations make it, the settings rows, and a statement prepared before a
//! migration moved its table run again on the same pooled connection (`docs/store.md`).

use bmo_receiver::store::migration::m20261017_000001_receiver::TABLES;
use bmo_receiver::store::migration::m20261017_000002_record_numbers::TABLE as RECORD_NUMBERS;
use bmo_receiver::store::migration::{Migrator, RECORD_TABLE, migrate};
use bmo_receiver::store::{all, one, pool, settings};
use sea_orm::{DatabaseConnection, TransactionTrait};
use sea_orm_migration::MigratorTrait;

use crate::common::Scratch;

// The tables of the schema, the migrator's own record among them.
const SCHEMA_TABLES: &str = "SELECT table_name::text FROM information_schema.tables
  WHERE table_schema = current_schema() ORDER BY table_name";

// Each index no key stands on, by name.
const INDEXES: &str = "SELECT indexname::text FROM pg_indexes
  WHERE schemaname = current_schema() AND indexname NOT LIKE '%_pkey' AND indexname NOT LIKE '%_key'
  ORDER BY indexname";

fn sorted(mut names: Vec<String>) -> Vec<String> {
  names.sort();
  names
}

#[tokio::test]
async fn the_migrations_make_every_table_once_and_take_them_down_again() {
  let database = Scratch::blank().await;
  let db = database.db();
  let made: Vec<String> = TABLES
    .iter()
    .copied()
    .chain([RECORD_NUMBERS, RECORD_TABLE])
    .map(str::to_owned)
    .collect();

  assert_eq!(migrate(db).await.unwrap(), 2);
  assert_eq!(database.texts(SCHEMA_TABLES).await, sorted(made.clone()));
  assert_eq!(migrate(db).await.unwrap(), 0, "a migrated database has nothing left to run");

  Migrator::down(db, None).await.unwrap();
  assert_eq!(database.texts(SCHEMA_TABLES).await, [RECORD_TABLE]);

  Migrator::up(db, None).await.unwrap();
  assert_eq!(database.texts(SCHEMA_TABLES).await, sorted(made));
}

#[tokio::test]
async fn the_tables_carry_their_indexes_and_an_instruction_names_its_agent() {
  let database = Scratch::new().await;

  assert_eq!(
    database.texts(INDEXES).await,
    [
      "instructions_pending",
      "instructions_round",
      "instructions_run",
      "messages_round",
      "messages_sender",
      "messages_sender_latest",
      "messages_unnamed",
      "random_draws_chat",
      "say_relays_asker",
      "say_relays_chat",
    ]
  );

  let defaults = database
    .texts(
      "SELECT column_name || ' ' || is_nullable || ' ' || COALESCE(column_default, 'none')
         FROM information_schema.columns
        WHERE table_schema = current_schema() AND table_name = 'instructions'
          AND column_name IN ('agent', 'sort', 'summary', 'round', 'run_id')
        ORDER BY column_name",
    )
    .await;

  assert_eq!(
    defaults,
    [
      "agent NO none",
      "round YES none",
      "run_id YES none",
      "sort YES none",
      "summary YES none"
    ]
  );

  database
    .run(
      r#"INSERT INTO "instructions" ("chat_id", "text", "created_at", "agent") VALUES (-7, 'said', 1, 'agent-a')"#,
      vec![],
    )
    .await;

  let row = one(database.db(), r#"SELECT "rowid", "events", "rides" FROM "instructions""#, vec![])
    .await
    .unwrap()
    .unwrap();

  assert_eq!(row.try_get::<i64>("", "rowid").unwrap(), 1);
  assert_eq!(row.try_get::<String>("", "events").unwrap(), "");
  assert_eq!(row.try_get::<i64>("", "rides").unwrap(), 0);
}

#[tokio::test]
async fn a_setting_is_set_read_taken_and_forgotten() {
  let database = Scratch::new().await;
  let db = database.db();

  assert_eq!(settings::get(db, "mode").await.unwrap(), None);

  settings::set(db, "mode", "owner_only").await.unwrap();
  settings::set(db, "mode", "everyone").await.unwrap();
  assert_eq!(settings::get(db, "mode").await.unwrap().as_deref(), Some("everyone"));

  assert_eq!(settings::take(db, "mode").await.unwrap().as_deref(), Some("everyone"));
  assert_eq!(settings::take(db, "mode").await.unwrap(), None);

  settings::set(db, "photos_look", "on").await.unwrap();
  settings::forget(db, "photos_look").await.unwrap();
  assert_eq!(settings::get(db, "photos_look").await.unwrap(), None);
}

#[tokio::test]
async fn the_numbers_of_a_family_of_keys_are_read_in_order_and_its_prefix_matched_as_written() {
  let database = Scratch::new().await;
  let db = database.db();

  for key in ["mark_12", "mark_3", "mark_x", "markA9", "other_4"] {
    settings::set(db, key, "1").await.unwrap();
  }

  // The underscore in the prefix is matched as itself, so `markA9` is no member of the family.
  assert_eq!(settings::numbered(db, "mark_").await.unwrap(), [3, 12]);
}

// The mark a stale plan leaves stands for the whole process and closes every connection opened before
// it, so these tests take turns rather than close each other's.
static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

// A pool of one connection, so every statement of the test stands on the connection the first one
// prepared on.
async fn single(database: &Scratch) -> DatabaseConnection {
  pool::connect(&database.address(), pool::pool(1)).await.unwrap()
}

#[tokio::test]
async fn a_statement_reading_every_column_survives_a_column_added_under_it() {
  let _turn = ONE_AT_A_TIME.lock().await;
  let database = Scratch::new().await;
  database.run("CREATE TABLE shape_probe (id BIGINT PRIMARY KEY)", vec![]).await;
  database.run("INSERT INTO shape_probe (id) VALUES (1)", vec![]).await;
  let db = single(&database).await;
  let read = "SELECT * FROM shape_probe ORDER BY id";

  let mut connection = db.get_postgres_connection_pool().acquire().await.unwrap();
  assert_eq!(sqlx::query(read).fetch_all(&mut *connection).await.unwrap().len(), 1);

  database
    .run("ALTER TABLE shape_probe ADD COLUMN added TEXT NOT NULL DEFAULT 'filled'", vec![])
    .await;

  // The plan the connection keeps is refused by the server as it stands.
  let refused = sqlx::query(read).fetch_all(&mut *connection).await.unwrap_err();
  assert_eq!(refused.as_database_error().and_then(|fault| fault.code()).as_deref(), Some("0A000"));
  drop(connection);

  let rows = all(&db, read, vec![]).await.unwrap();

  assert_eq!(rows.len(), 1);
  assert_eq!(rows[0].try_get::<String>("", "added").unwrap(), "filled");
}

#[tokio::test]
async fn a_transaction_that_met_the_old_plan_fails_alone() {
  let _turn = ONE_AT_A_TIME.lock().await;
  let database = Scratch::new().await;
  database.run("CREATE TABLE shape_probe (id BIGINT PRIMARY KEY)", vec![]).await;
  database.run("INSERT INTO shape_probe (id) VALUES (1)", vec![]).await;
  let db = single(&database).await;
  let read = "SELECT * FROM shape_probe WHERE id = $1";

  assert!(one(&db, read, vec![1i64.into()]).await.unwrap().is_some());

  database.run("ALTER TABLE shape_probe ADD COLUMN added TEXT", vec![]).await;

  let transaction = db.begin().await.unwrap();
  assert!(one(&transaction, read, vec![1i64.into()]).await.is_err());
  transaction.rollback().await.unwrap();

  assert!(one(&db, read, vec![1i64.into()]).await.unwrap().is_some());
}

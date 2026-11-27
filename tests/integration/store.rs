//! The receiver's database as its migrations make it, from the 1.0.0 baseline, an install from before it taken
//! as it stands, the settings rows, and a statement prepared before a migration moved its table run again on the
//! same pooled connection (`docs/store.md`).

use bmo_receiver::store::migration::{
  BASELINE, BEFORE, Migrator, NO_ROOM_TONES, RECORD_TABLE, ROOM_GRANTS, migrate,
};
use bmo_receiver::store::{all, one, pool, settings};
use sea_orm::{ConnectionTrait, DatabaseConnection, TransactionTrait};
use sea_orm_migration::MigratorTrait;

use crate::integration::common::Scratch;

// The tables of the schema, the migrator's own record among them.
const SCHEMA_TABLES: &str = "SELECT table_name::text FROM information_schema.tables
  WHERE table_schema = current_schema() ORDER BY table_name";

// Each index no key stands on, by name.
const INDEXES: &str = "SELECT indexname::text FROM pg_indexes
  WHERE schemaname = current_schema() AND indexname NOT LIKE '%_pkey' AND indexname NOT LIKE '%_key'
  ORDER BY indexname";

// Every table of the schema, the migrator's own record among them.
const MADE: [&str; 49] = [
  "agent_avatar_sets",
  "allowed_chats",
  "avatar_sets",
  "avatars",
  "bmo_room_contexts",
  "bmo_rooms",
  "bmo_session_notes",
  "bmo_session_rooms",
  "chat_notices",
  "chat_reads",
  "chat_titles",
  "collected_items",
  "holidays",
  "instruction_rounds",
  "instruction_stages",
  "instruction_texts",
  "instructions",
  "message_file_asks",
  "message_metadata",
  "message_number_marks",
  "message_reactions",
  "messages",
  "muted_members",
  "news_briefs",
  "people_names",
  "people_reads",
  "photo_chats",
  "private_rooms",
  "provider_owner_users",
  "provider_private_rooms",
  "provider_rooms",
  "provider_users",
  "random_draw_rooms",
  "random_draws",
  "reload_pending",
  "reload_sent",
  "room_accounts",
  "room_archives",
  "room_deliveries",
  "room_grants",
  "room_modes",
  "room_providers",
  "room_situations",
  "room_supervisors",
  "room_windows",
  "run_records",
  "say_relays",
  "seaql_migrations",
  "settings",
];

#[tokio::test]
async fn the_migrations_make_every_table_once_and_take_them_down_again() {
  let database = Scratch::blank().await;
  let db = database.db();

  assert_eq!(migrate(db).await.unwrap(), Migrator::migrations().len());
  assert_eq!(database.texts(SCHEMA_TABLES).await, MADE);
  assert_eq!(migrate(db).await.unwrap(), 0, "a migrated database has nothing left to run");
  assert_eq!(database.texts(RECORDED).await, [BASELINE, ROOM_GRANTS, NO_ROOM_TONES]);

  Migrator::down(db, None).await.unwrap();
  assert_eq!(database.texts(SCHEMA_TABLES).await, [RECORD_TABLE]);

  Migrator::up(db, None).await.unwrap();
  assert_eq!(database.texts(SCHEMA_TABLES).await, MADE);
}

const RECORDED: &str = r#"SELECT "version" FROM "seaql_migrations" ORDER BY "version""#;

// Every column and index of the schema but the record's, in one line.
const SHAPE: &str = "SELECT COALESCE((SELECT string_agg(table_name || '.' || column_name || ' ' || data_type || ' ' || \
   is_nullable || ' ' || COALESCE(column_default, ''), ', ' ORDER BY table_name, ordinal_position) \
   FROM information_schema.columns WHERE table_schema = current_schema() AND table_name <> 'seaql_migrations'), '') \
   || ' / ' || COALESCE((SELECT string_agg(indexname || ' ' || replace(indexdef, current_schema() || '.', ''), ', ' \
   ORDER BY indexname) FROM pg_indexes WHERE schemaname = current_schema() AND tablename <> 'seaql_migrations'), '')";

// A blank schema holding the baseline's tables and a setting, recorded as an install from before 1.0.0 that ran
// the first `through` migrations of its last release.
async fn installed(through: usize) -> Scratch {
  let database = Scratch::blank().await;
  database
    .db()
    .execute_unprepared(include_str!("../../src/store/migration/baseline.sql"))
    .await
    .unwrap();
  database
    .run(
      r#"CREATE TABLE "seaql_migrations" ("version" varchar PRIMARY KEY, "applied_at" bigint NOT NULL)"#,
      vec![],
    )
    .await;
  for version in &BEFORE[..through] {
    database
      .run(
        r#"INSERT INTO "seaql_migrations" ("version", "applied_at") VALUES ($1, 1)"#,
        vec![(*version).into()],
      )
      .await;
  }
  settings::set(database.db(), "photos_look", "on").await.unwrap();
  database
}

#[tokio::test]
async fn an_install_at_the_last_release_before_the_baseline_only_has_its_record_rewritten() {
  let database = installed(BEFORE.len()).await;
  let db = database.db();

  // A schema laid down whole and a blank migrate to the same shape, migrations after the baseline included.
  let full = Scratch::blank().await;
  migrate(full.db()).await.unwrap();

  // Adoption rewrites the record to the baseline, then the migrations after it run on the standing schema.
  assert_eq!(migrate(db).await.unwrap(), 2);
  assert_eq!(database.texts(RECORDED).await, [BASELINE, ROOM_GRANTS, NO_ROOM_TONES]);
  assert_eq!(database.texts(SHAPE).await, full.texts(SHAPE).await);
  assert_eq!(database.texts(r#"SELECT "value" FROM "settings""#).await, ["on"]);

  // A second start finds every migration recorded and changes nothing.
  assert_eq!(migrate(db).await.unwrap(), 0);
  assert_eq!(database.texts(RECORDED).await, [BASELINE, ROOM_GRANTS, NO_ROOM_TONES]);
}

#[tokio::test]
async fn an_install_short_of_the_last_release_before_the_baseline_is_refused() {
  let database = installed(3).await;

  let refused = migrate(database.db()).await.unwrap_err().to_string();
  assert!(refused.contains(BEFORE[2]), "{refused}");
  assert_eq!(database.texts(RECORDED).await.len(), 3);
}

#[tokio::test]
async fn a_blank_schema_gets_the_baseline_as_it_is_laid_down_whole() {
  let migrated = Scratch::blank().await;
  migrate(migrated.db()).await.unwrap();
  let laid = Scratch::blank().await;
  for sql in [
    include_str!("../../src/store/migration/baseline.sql"),
    include_str!("../../src/store/migration/room_grants.sql"),
    include_str!("../../src/store/migration/no_room_tones.sql"),
  ] {
    laid.db().execute_unprepared(sql).await.unwrap();
  }

  assert_eq!(migrated.texts(SHAPE).await, laid.texts(SHAPE).await);
}

#[tokio::test]
async fn the_tables_carry_their_indexes_and_an_instruction_names_its_agent() {
  let database = Scratch::new().await;

  assert_eq!(
    database.texts(INDEXES).await,
    [
      "instruction_rounds_room",
      "instruction_rounds_round",
      "instruction_rounds_session",
      "instructions_pending",
      "instructions_round",
      "instructions_run",
      "messages_line",
      "messages_round",
      "messages_sender",
      "messages_sender_latest",
      "messages_via",
      "private_rooms_account",
      "random_draws_chat",
      "room_accounts_account",
      "room_deliveries_copy",
      "room_deliveries_message",
      "room_grants_account",
      "run_records_run",
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

// The indexes on `messages`, each as the database states it.
const MESSAGE_INDEXES: &str = "SELECT indexname::text || ': ' || indexdef FROM pg_indexes
  WHERE schemaname = current_schema() AND tablename = 'messages' AND indexname NOT LIKE '%_pkey'
  AND indexname NOT LIKE '%_key'
  ORDER BY indexname";

#[tokio::test]
async fn a_rooms_line_is_indexed_by_time_and_its_roads_by_the_messages_that_name_one() {
  let database = Scratch::new().await;
  let stated = |indexes: Vec<String>| -> Vec<String> {
    indexes
      .iter()
      .map(|index| {
        let (name, definition) = index.split_once(": ").unwrap();
        let columns = definition.split_once(" USING btree ").unwrap().1;

        format!("{name} {columns}")
      })
      .collect()
  };

  assert_eq!(
    stated(database.texts(MESSAGE_INDEXES).await),
    [
      "messages_line (chat_id, sent_at, rowid)",
      "messages_round (chat_id, round)",
      "messages_sender (sender_id, chat_id)",
      "messages_sender_latest (chat_id, sender_id, sent_at)",
      "messages_via (chat_id, via, rowid) WHERE (via IS NOT NULL)",
    ]
  );
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

#[test]
fn a_pool_closes_no_connection_for_its_age_or_for_standing_idle() {
  let pool = pool::pool(4);

  assert_eq!(pool.get_idle_timeout(), None);
  assert_eq!(pool.get_max_lifetime(), None);
  assert_eq!(pool.get_max_connections(), 4);
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

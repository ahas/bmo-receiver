//! The receiver's database as its migrations make it, the settings rows, and a statement prepared before a
//! migration moved its table run again on the same pooled connection (`docs/store.md`).

use bmo_receiver::store::migration::m20261017_000001_receiver::TABLES;
use bmo_receiver::store::migration::m20261017_000002_record_numbers::TABLE as RECORD_NUMBERS;
use bmo_receiver::store::migration::m20261017_000003_room_windows::TABLE as ROOM_WINDOWS;
use bmo_receiver::store::migration::m20261017_000004_people_reads::TABLE as PEOPLE_READS;
use bmo_receiver::store::migration::m20261019_000005_message_metadata::TABLE as MESSAGE_METADATA;
use bmo_receiver::store::migration::m20261022_000008_news_briefs::TABLES as NEWS_BRIEFS_TABLES;
use bmo_receiver::store::migration::m20261023_000009_room_providers::TABLES as ROOM_PROVIDERS_TABLES;
use bmo_receiver::store::migration::m20261023_000010_room_modes::TABLE as ROOM_MODES;
use bmo_receiver::store::migration::m20261023_000012_draft_process::TABLES as DRAFT_PROCESS_TABLES;
use bmo_receiver::store::migration::m20261027_000018_avatar_sets::TABLE as AVATAR_SETS;
use bmo_receiver::store::migration::m20261027_000020_record_acts::TABLE as MESSAGE_REACTIONS;
use bmo_receiver::store::migration::m20261027_000022_room_situations::TABLE as ROOM_SITUATIONS;
use bmo_receiver::store::migration::m20261029_000030_room_settings_dropped::TABLES as DROPPED_SETTINGS;
use bmo_receiver::store::migration::m20261106_000034_session_notes::TABLE as SESSION_NOTES;
use bmo_receiver::store::migration::{Migrator, RECORD_TABLE, migrate};
use bmo_receiver::store::{all, one, pool, settings};
use sea_orm::{DatabaseConnection, TransactionTrait};
use sea_orm_migration::MigratorTrait;

use crate::integration::common::{Scratch, undone_from};

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
    .filter(|table| !DROPPED_SETTINGS.contains(table))
    .map(|table| if table == "room_answerers" { "room_supervisors" } else { table })
    .chain([
      RECORD_NUMBERS,
      ROOM_WINDOWS,
      PEOPLE_READS,
      MESSAGE_METADATA,
      ROOM_MODES,
      AVATAR_SETS,
      MESSAGE_REACTIONS,
      ROOM_SITUATIONS,
      SESSION_NOTES,
      RECORD_TABLE,
      "room_archives",
      "message_file_asks",
      "room_accounts",
      "private_rooms",
      "agent_avatar_sets",
      "people_names",
    ])
    .chain(NEWS_BRIEFS_TABLES)
    .chain(ROOM_PROVIDERS_TABLES)
    .chain(DRAFT_PROCESS_TABLES)
    .map(str::to_owned)
    .collect();

  assert_eq!(migrate(db).await.unwrap(), Migrator::migrations().len());
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
      "draft_rounds_room",
      "draft_rounds_round",
      "draft_rounds_session",
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
  let db = database.db();
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

  // The line's index is made thirteen migrations before the forwards, every migration since standing between them.
  // Down to the same version as before the note's columns were added, one step further back.
  let steps = undone_from(db, "m20261023_000011_message_line_index").await;

  let before = stated(database.texts(MESSAGE_INDEXES).await);

  assert!(before.contains(&"messages_unnamed (chat_id) WHERE (via IS NULL)".to_owned()));
  assert!(
    !before
      .iter()
      .any(|index| index.starts_with("messages_line") || index.starts_with("messages_via"))
  );

  assert_eq!(migrate(db).await.unwrap(), steps);
  assert_eq!(stated(database.texts(MESSAGE_INDEXES).await).len(), 5);
}

#[tokio::test]
async fn the_thinking_messages_a_record_holds_take_their_kind_and_lose_their_marker() {
  let database = Scratch::new().await;
  let db = database.db();
  let marker = bmo_receiver::store::migration::m20261109_000037_thinking_kind::MARKER;
  let steps = undone_from(db, "m20261109_000037_thinking_kind").await;

  // A thinking message as it was kept, an answer that only quotes the marker, a person's words opening on it, and a
  // round's answer opening on it: the first alone is a thinking message.
  let rows: [(i64, Option<&str>, Option<&str>, String); 4] = [
    (1, Some("agent-a"), None, format!("{marker}\nweighing the room's words")),
    (2, Some("agent-a"), None, format!("the line said {marker}")),
    (3, None, None, format!("{marker}\na person wrote this")),
    (4, Some("agent-a"), Some("round:1"), format!("{marker}\nan answer")),
  ];

  for (id, agent, round, text) in rows {
    database
      .run(
        r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_name", "sent_at", "text", "agent_name", "round")
           VALUES (1, $1, 'agent-a', $1, $2, $3, $4)"#,
        vec![id.into(), text.into(), agent.map(str::to_owned).into(), round.map(str::to_owned).into()],
      )
      .await;
  }

  assert_eq!(migrate(db).await.unwrap(), steps);

  let kept = all(
    db,
    r#"SELECT "message_id", "kind", "text" FROM "messages" ORDER BY "message_id""#,
    vec![],
  )
  .await
  .unwrap()
  .iter()
  .map(|row| {
    (
      row.try_get::<i64>("", "message_id").unwrap(),
      row.try_get::<Option<String>>("", "kind").unwrap(),
      row.try_get::<String>("", "text").unwrap(),
    )
  })
  .collect::<Vec<_>>();

  assert_eq!(kept[0], (1, Some("thinking".to_owned()), "weighing the room's words".to_owned()));
  assert_eq!(kept[1].1, None, "an answer that quotes the marker is an answer");
  assert_eq!(kept[2], (3, None, format!("{marker}\na person wrote this")));
  assert_eq!(kept[3].1, None, "a round's answer is an answer whatever it opens on");

  // Stepping back hands the message its marker again.
  undone_from(db, "m20261109_000037_thinking_kind").await;
  assert_eq!(
    one(db, r#"SELECT "text" FROM "messages" WHERE "message_id" = 1 AND "kind" IS NULL"#, vec![])
      .await
      .unwrap()
      .map(|row| row.try_get::<String>("", "text").unwrap()),
    Some(format!("{marker}\nweighing the room's words"))
  );
}

#[tokio::test]
async fn a_message_holding_a_kept_sticker_takes_the_sticker_kind() {
  let database = Scratch::new().await;
  let db = database.db();
  let steps = undone_from(db, "m20261117_000053_sticker_messages").await;

  // A message that kept a sticker file, one that kept a video, and a thinking block that somehow holds a sticker
  // row: only the first takes the kind, and a kind already set stays.
  let rows: [(i64, Option<&str>, &str); 3] =
    [(1, None, "sticker"), (2, None, "video"), (3, Some("thinking"), "sticker")];

  for (id, kind, kept) in rows {
    database
      .run(
        r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_name", "sent_at", "text", "kind")
           VALUES (1, $1, 'person-a', $1, '[file]', $2)"#,
        vec![id.into(), kind.map(str::to_owned).into()],
      )
      .await;
    database
      .run(
        r#"INSERT INTO "collected_items" ("chat_id", "message_id", "sender_name", "kind", "source", "collected_at")
           VALUES (1, $1, 'person-a', $2, 'file', $1)"#,
        vec![id.into(), kept.into()],
      )
      .await;
  }

  assert_eq!(migrate(db).await.unwrap(), steps);
  assert_eq!(
    database
      .texts(r#"SELECT COALESCE("kind", '') FROM "messages" ORDER BY "message_id""#)
      .await,
    ["sticker", "", "thinking"]
  );
}

#[tokio::test]
async fn what_stood_for_every_supervisor_becomes_each_supervisor_s_own() {
  let database = Scratch::new().await;
  let db = database.db();

  let steps = undone_from(db, "m20261110_000038_draft_per_supervisor").await;

  for row in [
    r#"INSERT INTO "room_supervisors" ("chat_id", "supervisor") VALUES (1, 'agent-a'), (2, 'agent-a'), (2, 'agent-b')"#,
    r#"INSERT INTO "draft_questions" ("name", "text", "set_at") VALUES ('facts', 'check the facts', 7)"#,
    r#"INSERT INTO "draft_processes"
         ("provider", "tier", "agent", "rounds", "send_from", "light", "off", "light_chars", "ceiling_seconds",
          "past_ceiling", "set_at")
       VALUES ('*', '*', '*', '["facts","final"]', '', '["final"]', '[]', 12, 0, 'send', 7),
              ('provider-a', '*', '*', '["final"]', '', '["final"]', '[]', 12, 0, 'send', 7)"#,
  ] {
    database.run(row, vec![]).await;
  }

  assert_eq!(migrate(db).await.unwrap(), steps);
  assert_eq!(
    database
      .texts(r#"SELECT "agent" || ' ' || "name" || ' ' || "text" FROM "draft_questions" ORDER BY "agent""#)
      .await,
    ["agent-a facts check the facts", "agent-b facts check the facts"]
  );
  assert_eq!(
    database
      .texts(
        r#"SELECT "provider" || '/' || "tier" || '/' || "agent" || ' ' || "rounds" FROM "draft_processes"
            ORDER BY "agent""#
      )
      .await,
    [r#"*/*/agent-a ["facts","final"]"#, r#"*/*/agent-b ["facts","final"]"#],
    "each supervisor holds the row that stood for all, and a row of no supervisor is gone"
  );

  // Stepping back keeps one text a name.
  undone_from(db, "m20261110_000038_draft_per_supervisor").await;
  assert_eq!(
    database.texts(r#"SELECT "name" || ' ' || "text" FROM "draft_questions""#).await,
    ["facts check the facts"]
  );
}

#[tokio::test]
async fn the_stand_ins_settings_go_and_every_other_stays() {
  let database = Scratch::new().await;
  let db = database.db();

  let steps = undone_from(db, "m20261113_000044_no_stand_in").await;
  for (key, value) in [("fallback_since", "1"), ("fallback_provider", "provider-a"), ("photos_look", "on")]
  {
    settings::set(db, key, value).await.unwrap();
  }

  assert_eq!(migrate(db).await.unwrap(), steps);
  assert_eq!(database.texts(r#"SELECT "key" FROM "settings" ORDER BY "key""#).await, ["photos_look"]);
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

#[tokio::test]
async fn the_rows_under_the_consoles_former_key_are_the_records_and_an_agent_keeps_one_record_number() {
  let database = Scratch::blank().await;
  let db = database.db();

  Migrator::up(db, Some(18)).await.unwrap();

  for row in [
    r#"INSERT INTO "provider_users" ("provider", "user_id", "sender_id") VALUES ('console', 'agent-a', 9000000000012), ('record', 'agent-a', -1), ('console', 'agent-b', 9000000000011)"#,
    r#"INSERT INTO "provider_rooms" ("provider", "room_id", "chat_id", "title") VALUES ('console', 'room-a', -9000000000003, 'Room A')"#,
    r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via") VALUES (-9000000000003, -1, 9000000000012, 'Agent A', 1, 'hello', 'console')"#,
    r#"INSERT INTO "avatars" ("kind", "subject_id", "photo_id", "path", "updated_at") VALUES ('person', 9000000000012, 'p', '/a.jpg', 1)"#,
  ] {
    database.run(row, vec![]).await;
  }

  assert_eq!(
    migrate(db).await.unwrap(),
    Migrator::migrations().len() - 18,
    "every migration after the first eighteen"
  );
  assert_eq!(
    database.texts(r#"SELECT "via" FROM "messages""#).await,
    ["record"],
    "the road an older archive named the record by is named as the record is"
  );
  assert_eq!(
    database.texts(r#"SELECT "provider" || ':' || "user_id" || ':' || "sender_id" FROM "provider_users" ORDER BY "user_id""#).await,
    ["record:agent-a:-1", "record:agent-b:9000000000011"],
    "one record number to an agent, the record's where it held two"
  );
  assert_eq!(database.texts(r#"SELECT "provider" FROM "provider_rooms""#).await, ["record"]);
  assert_eq!(
    database.texts(r#"SELECT "sender_id"::text FROM "messages""#).await,
    ["-1"],
    "its words fold into it"
  );
  assert_eq!(
    database.texts(r#"SELECT "subject_id"::text FROM "avatars""#).await,
    ["-1"],
    "and its picture"
  );
}

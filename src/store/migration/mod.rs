//! The receiver's migrations, from the 1.0.0 baseline, run by SeaORM's migrator in the order the list below
//! names them, which keeps the ones it ran in its own table beside the receiver's (`docs/store.md`, 「The
//! database」).

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{
  ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait,
};

pub mod m20261124_000060_baseline;
pub mod m20261125_000061_room_grants;

/// The table the migrator keeps its record in, which is no part of the receiver's shape.
pub const RECORD_TABLE: &str = "seaql_migrations";

/// The migration that makes the schema whole.
pub const BASELINE: &str = m20261124_000060_baseline::NAME;

/// The migration that stands the room grants: a room member's rights over its people, beside its membership.
pub const ROOM_GRANTS: &str = m20261125_000061_room_grants::NAME;

/// The record an install from before 1.0.0 holds once every migration of its last release ran. Its schema
/// is the baseline's.
pub const BEFORE: [&str; 59] = [
  "m20261017_000001_receiver",
  "m20261017_000002_record_numbers",
  "m20261017_000003_room_windows",
  "m20261017_000004_people_reads",
  "m20261019_000005_message_metadata",
  "m20261019_000006_collected_file_ids",
  "m20261020_000007_millisecond_times",
  "m20261022_000008_news_briefs",
  "m20261023_000009_room_providers",
  "m20261023_000010_room_modes",
  "m20261023_000011_message_line_index",
  "m20261023_000012_draft_process",
  "m20261024_000013_message_voice",
  "m20261026_000014_draft_send_from",
  "m20261026_000015_draft_turn_seconds",
  "m20261026_000016_people_languages",
  "m20261026_000017_draft_reads",
  "m20261027_000018_avatar_sets",
  "m20261027_000019_record_numbers",
  "m20261027_000020_record_acts",
  "m20261027_000021_forwards",
  "m20261027_000022_room_situations",
  "m20261027_000023_message_situations",
  "m20261027_000023_voice_transcripts",
  "m20261028_000024_message_image_state",
  "m20261028_000026_instructions_agent_only",
  "m20261028_000027_draft_turn_seconds_removed",
  "m20261029_000028_people_languages_dropped",
  "m20261029_000029_room_situations_read_through",
  "m20261029_000030_room_settings_dropped",
  "m20261030_000031_draft_round_sessions",
  "m20261102_000032_instruction_causes",
  "m20261103_000033_room_supervisors",
  "m20261106_000034_session_notes",
  "m20261106_000035_instruction_publisher",
  "m20261109_000036_draft_off",
  "m20261109_000037_thinking_kind",
  "m20261110_000038_draft_per_supervisor",
  "m20261111_000039_room_archives",
  "m20261111_000040_sticker_kind",
  "m20261112_000041_no_light_turn",
  "m20261112_000042_instruction_carrying",
  "m20261112_000043_double_slots",
  "m20261113_000044_no_stand_in",
  "m20261114_000045_message_file_asks",
  "m20261115_000046_draft_versions",
  "m20261116_000047_room_accounts",
  "m20261116_000048_private_rooms",
  "m20261116_000049_draft_stages_off",
  "m20261116_000050_draft_rounds_by_room",
  "m20261116_000051_agent_avatar_sets",
  "m20261116_000052_people_names",
  "m20261117_000053_sticker_messages",
  "m20261117_000054_double_slots_dropped",
  "m20261117_000055_relays_wait_on_orders",
  "m20261118_000056_fixed_line_messages",
  "m20261119_000057_retire_draft_process",
  "m20261120_000058_room_tones",
  "m20261121_000059_keep_mark_texts",
];

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
  fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
      Box::new(m20261124_000060_baseline::Migration),
      Box::new(m20261125_000061_room_grants::Migration),
    ]
  }
}

/// Every migration a database has not run yet, run on it, after an install from before 1.0.0 is taken as it
/// stands; how many there were.
pub async fn migrate(db: &DatabaseConnection) -> Result<usize, DbErr> {
  adopt(db).await?;
  let pending = Migrator::get_pending_migrations(db).await?.len();

  Migrator::up(db, None).await?;

  Ok(pending)
}

/// A record naming every migration of the last release before 1.0.0, and nothing else, rewritten to name the
/// baseline alone, touching no table and no row; one naming only part of them refused.
async fn adopt(db: &DatabaseConnection) -> Result<(), DbErr> {
  let tx = db.begin().await?;
  let statement = |sql: String, values: Vec<sea_orm::Value>| {
    Statement::from_sql_and_values(DbBackend::Postgres, sql, values)
  };
  let recorded = tx
    .query_one(statement(format!("SELECT to_regclass('{RECORD_TABLE}') IS NOT NULL AS held"), vec![]))
    .await?
    .map(|row| row.try_get::<bool>("", "held"))
    .transpose()?
    .unwrap_or(false);
  if !recorded {
    return Ok(());
  }
  tx.execute_unprepared(&format!(r#"LOCK TABLE "{RECORD_TABLE}" IN EXCLUSIVE MODE"#))
    .await?;
  let held = tx
    .query_all(statement(
      format!(r#"SELECT "version" FROM "{RECORD_TABLE}" ORDER BY "version""#),
      vec![],
    ))
    .await?
    .into_iter()
    .map(|row| row.try_get::<String>("", "version"))
    .collect::<Result<Vec<_>, _>>()?;
  if held.is_empty() || !held.iter().all(|version| BEFORE.contains(&version.as_str())) {
    return Ok(());
  }
  if held.len() != BEFORE.len() {
    let reached = held.last().map_or("", String::as_str);
    return Err(DbErr::Migration(format!(
      "the database holds the migrations of a release before 1.0.0 only through {reached}; that release's \
       last migration runs first"
    )));
  }
  tx.execute_unprepared(&format!(r#"DELETE FROM "{RECORD_TABLE}""#)).await?;
  tx.execute(statement(
    format!(
      r#"INSERT INTO "{RECORD_TABLE}" ("version", "applied_at") VALUES ($1, EXTRACT(EPOCH FROM now())::bigint)"#
    ),
    vec![BASELINE.into()],
  ))
  .await?;
  tx.commit().await
}

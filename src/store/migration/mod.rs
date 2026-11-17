//! The receiver's migrations, run by SeaORM's migrator, which keeps the ones it ran in its own table beside
//! the receiver's (`docs/store.md`, 「The database」).

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseConnection;

pub mod m20261017_000001_receiver;
pub mod m20261017_000002_record_numbers;
pub mod m20261017_000003_room_windows;
pub mod m20261017_000004_people_reads;
pub mod m20261019_000005_message_metadata;
pub mod m20261019_000006_collected_file_ids;
pub mod m20261020_000007_millisecond_times;
pub mod m20261022_000008_news_briefs;
pub mod m20261023_000009_room_providers;
pub mod m20261023_000010_room_modes;
pub mod m20261023_000011_message_line_index;
pub mod m20261023_000012_draft_process;
pub mod m20261024_000013_message_voice;
pub mod m20261026_000014_draft_send_from;
pub mod m20261026_000015_draft_turn_seconds;
pub mod m20261026_000016_people_languages;
pub mod m20261026_000017_draft_reads;
pub mod m20261027_000018_avatar_sets;
pub mod m20261027_000019_record_numbers;
pub mod m20261027_000020_record_acts;
pub mod m20261027_000021_forwards;
pub mod m20261027_000022_room_situations;
pub mod m20261027_000023_message_situations;
pub mod m20261027_000023_voice_transcripts;
pub mod m20261028_000024_message_image_state;
pub mod m20261028_000026_instructions_agent_only;
pub mod m20261028_000027_draft_turn_seconds_removed;
pub mod m20261029_000028_people_languages_dropped;
pub mod m20261029_000029_room_situations_read_through;
pub mod m20261029_000030_room_settings_dropped;
pub mod m20261030_000031_draft_round_sessions;
pub mod m20261102_000032_instruction_causes;
pub mod m20261103_000033_room_supervisors;
pub mod m20261106_000034_session_notes;
pub mod m20261106_000035_instruction_publisher;
pub mod m20261109_000036_draft_off;
pub mod m20261109_000037_thinking_kind;
pub mod m20261110_000038_draft_per_supervisor;
pub mod m20261111_000039_room_archives;
pub mod m20261111_000040_sticker_kind;
pub mod m20261112_000041_no_light_turn;
pub mod m20261112_000042_instruction_carrying;
pub mod m20261112_000043_double_slots;
pub mod m20261113_000044_no_stand_in;
pub mod m20261114_000045_message_file_asks;
pub mod m20261115_000046_draft_versions;
pub mod m20261116_000047_room_accounts;
pub mod m20261116_000048_private_rooms;
pub mod m20261116_000049_draft_stages_off;
pub mod m20261116_000050_draft_rounds_by_room;
pub mod m20261116_000051_agent_avatar_sets;
pub mod m20261116_000052_people_names;
pub mod m20261117_000053_sticker_messages;

/// The table the migrator keeps its record in, which is no part of the receiver's shape.
pub const RECORD_TABLE: &str = "seaql_migrations";

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
  fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
      Box::new(m20261017_000001_receiver::Migration),
      Box::new(m20261017_000002_record_numbers::Migration),
      Box::new(m20261017_000003_room_windows::Migration),
      Box::new(m20261017_000004_people_reads::Migration),
      Box::new(m20261019_000005_message_metadata::Migration),
      Box::new(m20261019_000006_collected_file_ids::Migration),
      Box::new(m20261020_000007_millisecond_times::Migration),
      Box::new(m20261022_000008_news_briefs::Migration),
      Box::new(m20261023_000009_room_providers::Migration),
      Box::new(m20261023_000010_room_modes::Migration),
      Box::new(m20261023_000011_message_line_index::Migration),
      Box::new(m20261023_000012_draft_process::Migration),
      Box::new(m20261024_000013_message_voice::Migration),
      Box::new(m20261026_000014_draft_send_from::Migration),
      Box::new(m20261026_000015_draft_turn_seconds::Migration),
      Box::new(m20261026_000016_people_languages::Migration),
      Box::new(m20261026_000017_draft_reads::Migration),
      Box::new(m20261027_000018_avatar_sets::Migration),
      Box::new(m20261027_000019_record_numbers::Migration),
      Box::new(m20261027_000020_record_acts::Migration),
      Box::new(m20261027_000021_forwards::Migration),
      Box::new(m20261027_000022_room_situations::Migration),
      Box::new(m20261027_000023_message_situations::Migration),
      Box::new(m20261027_000023_voice_transcripts::Migration),
      Box::new(m20261028_000024_message_image_state::Migration),
      Box::new(m20261028_000026_instructions_agent_only::Migration),
      Box::new(m20261028_000027_draft_turn_seconds_removed::Migration),
      Box::new(m20261029_000028_people_languages_dropped::Migration),
      Box::new(m20261029_000029_room_situations_read_through::Migration),
      Box::new(m20261029_000030_room_settings_dropped::Migration),
      Box::new(m20261030_000031_draft_round_sessions::Migration),
      Box::new(m20261102_000032_instruction_causes::Migration),
      Box::new(m20261103_000033_room_supervisors::Migration),
      Box::new(m20261106_000034_session_notes::Migration),
      Box::new(m20261106_000035_instruction_publisher::Migration),
      Box::new(m20261109_000036_draft_off::Migration),
      Box::new(m20261109_000037_thinking_kind::Migration),
      Box::new(m20261110_000038_draft_per_supervisor::Migration),
      Box::new(m20261111_000039_room_archives::Migration),
      Box::new(m20261111_000040_sticker_kind::Migration),
      Box::new(m20261112_000041_no_light_turn::Migration),
      Box::new(m20261112_000042_instruction_carrying::Migration),
      Box::new(m20261112_000043_double_slots::Migration),
      Box::new(m20261113_000044_no_stand_in::Migration),
      Box::new(m20261114_000045_message_file_asks::Migration),
      Box::new(m20261115_000046_draft_versions::Migration),
      Box::new(m20261116_000047_room_accounts::Migration),
      Box::new(m20261116_000048_private_rooms::Migration),
      Box::new(m20261116_000049_draft_stages_off::Migration),
      Box::new(m20261116_000050_draft_rounds_by_room::Migration),
      Box::new(m20261116_000051_agent_avatar_sets::Migration),
      Box::new(m20261116_000052_people_names::Migration),
      Box::new(m20261117_000053_sticker_messages::Migration),
    ]
  }
}

/// Every migration a database has not run yet, run on it; how many there were.
pub async fn migrate(db: &DatabaseConnection) -> Result<usize, DbErr> {
  let pending = Migrator::get_pending_migrations(db).await?.len();

  Migrator::up(db, None).await?;

  Ok(pending)
}

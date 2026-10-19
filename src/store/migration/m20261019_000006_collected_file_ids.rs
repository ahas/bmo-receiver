//! A kept file told apart from another of its message by its id rather than its name: `collected_items` holds
//! one row per room, message and the file's id among its message's files (`docs/room-files.md`, 「Message
//! files」).

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261019_000006_collected_file_ids"
  }
}

/// The key a kept file's row stands on: its room, its message and its id.
pub const KEY: &str = "collected_items_chat_id_message_id_file_unique_id_key";

// The key the first migration makes and this one's `down` puts back: its room, its message and its name.
const NAME_KEY: &str = "collected_items_chat_id_message_id_source_key";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(&format!(
        r#"ALTER TABLE "collected_items" DROP CONSTRAINT "{NAME_KEY}";
ALTER TABLE "collected_items" ADD CONSTRAINT "{KEY}" UNIQUE ("chat_id", "message_id", "file_unique_id");"#
      ))
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(&format!(
        r#"ALTER TABLE "collected_items" DROP CONSTRAINT IF EXISTS "{KEY}";
ALTER TABLE "collected_items" ADD CONSTRAINT "{NAME_KEY}" UNIQUE ("chat_id", "message_id", "source");"#
      ))
      .await?;

    Ok(())
  }
}

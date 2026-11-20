//! The tone layer a room is set to (`docs/store.md`, 「The tables」).

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261120_000058_room_tones"
  }
}

/// The table this migration makes.
pub const TABLE: &str = "room_tones";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"CREATE TABLE "room_tones" (
  "chat_id" bigint NOT NULL,
  "tone" text NOT NULL,
  "set_at" bigint NOT NULL,
  rowid bigint GENERATED ALWAYS AS ("chat_id") STORED,
  PRIMARY KEY ("chat_id")
);"#,
      )
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(&format!(r#"DROP TABLE IF EXISTS "{TABLE}""#))
      .await?;

    Ok(())
  }
}

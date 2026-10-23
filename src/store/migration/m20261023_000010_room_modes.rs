//! The mode a room was added in (`docs/store.md`, 「The tables」).

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261023_000010_room_modes"
  }
}

/// The table this migration makes.
pub const TABLE: &str = "room_modes";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"CREATE TABLE "room_modes" (
  "chat_id" bigint NOT NULL,
  "mode" text NOT NULL,
  "added_at" bigint NOT NULL,
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

//! The situation note the receiver keeps for a room (`docs/store.md`, 「The tables」; `docs/instruction.md`, 「The
//! situation note」): one row a room, replaced when the note changes.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261027_000022_room_situations"
  }
}

/// The table this migration makes.
pub const TABLE: &str = "room_situations";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"CREATE TABLE "room_situations" (
  "chat_id" bigint NOT NULL,
  "text" text NOT NULL,
  "updated_at" bigint NOT NULL,
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

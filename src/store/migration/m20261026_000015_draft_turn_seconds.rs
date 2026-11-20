//! The bound of one turn of a draft process row (`docs/store.md`, 「The tables」; `docs/instruction.md`, 「Time」):
//! the policy's 120 seconds on a row set before it was named.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261026_000015_draft_turn_seconds"
  }
}

/// The column this migration adds.
pub const COLUMN: &str = "turn_seconds";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"ALTER TABLE "draft_processes" ADD COLUMN "turn_seconds" bigint NOT NULL DEFAULT 120;"#,
      )
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(r#"ALTER TABLE "draft_processes" DROP COLUMN IF EXISTS "turn_seconds";"#)
      .await?;

    Ok(())
  }
}

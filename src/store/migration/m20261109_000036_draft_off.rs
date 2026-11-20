//! The questions a draft process row has switched off (`docs/store.md`, 「The tables」; `docs/instruction.md`, 「The
//! process table」): none on a row set before a question could be switched off.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261109_000036_draft_off"
  }
}

/// The column this migration adds.
pub const COLUMN: &str = "off";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"ALTER TABLE "draft_processes" ADD COLUMN IF NOT EXISTS "off" text NOT NULL DEFAULT '[]';"#,
      )
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(r#"ALTER TABLE "draft_processes" DROP COLUMN IF EXISTS "off";"#)
      .await?;

    Ok(())
  }
}

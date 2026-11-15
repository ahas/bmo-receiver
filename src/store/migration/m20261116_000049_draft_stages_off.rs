//! The stages after the message a draft process row has switched off (`docs/store.md`, 「The tables」;
//! `docs/drafting.md`, 「The process table」): none on a row set before a stage could be switched off.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261116_000049_draft_stages_off"
  }
}

/// The column this migration adds.
pub const COLUMN: &str = "stages_off";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"ALTER TABLE "draft_processes" ADD COLUMN IF NOT EXISTS "stages_off" text NOT NULL DEFAULT '[]';"#,
      )
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(r#"ALTER TABLE "draft_processes" DROP COLUMN IF EXISTS "stages_off";"#)
      .await?;

    Ok(())
  }
}

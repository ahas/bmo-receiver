//! The question a draft process row reads a send from (`docs/store.md`, 「The tables」; `docs/instruction.md`, 「The
//! process table」): empty on a row set before it was named, which reads a send from its first question.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261026_000014_draft_send_from"
  }
}

/// The column this migration adds.
pub const COLUMN: &str = "send_from";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"ALTER TABLE "draft_processes" ADD COLUMN "send_from" text NOT NULL DEFAULT '';"#,
      )
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(r#"ALTER TABLE "draft_processes" DROP COLUMN IF EXISTS "send_from";"#)
      .await?;

    Ok(())
  }
}

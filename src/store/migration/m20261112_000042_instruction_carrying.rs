//! A quiet instruction's one lifecycle (`docs/bmo-window.md`, 「Quiet instruction」): the session and turn that carry
//! its text, how often a round the agency answered failed with it, and why a row not carried was let go, so no row
//! is ever deleted.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261112_000042_instruction_carrying"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"ALTER TABLE "instructions" ADD COLUMN IF NOT EXISTS "session" text,
             ADD COLUMN IF NOT EXISTS "turn" bigint,
             ADD COLUMN IF NOT EXISTS "failures" bigint NOT NULL DEFAULT 0,
             ADD COLUMN IF NOT EXISTS "let_go" text"#,
      )
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"ALTER TABLE "instructions" DROP COLUMN IF EXISTS "session", DROP COLUMN IF EXISTS "turn",
             DROP COLUMN IF EXISTS "failures", DROP COLUMN IF EXISTS "let_go""#,
      )
      .await?;

    Ok(())
  }
}

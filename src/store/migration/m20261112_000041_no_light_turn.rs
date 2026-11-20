//! No turn is light (`docs/instruction.md`, 「The table」): every turn follows its row's `rounds`, so a row keeps no light
//! list and no light bound.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261112_000041_no_light_turn"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"ALTER TABLE "draft_processes" DROP COLUMN IF EXISTS "light", DROP COLUMN IF EXISTS "light_chars""#,
      )
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"ALTER TABLE "draft_processes" ADD COLUMN IF NOT EXISTS "light" text NOT NULL DEFAULT '[]',
             ADD COLUMN IF NOT EXISTS "light_chars" bigint NOT NULL DEFAULT 0"#,
      )
      .await?;

    Ok(())
  }
}

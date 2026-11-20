//! A room's rounds listed off the round log by room and row (`docs/store.md`, 「The tables」;
//! `docs/instruction.md`, 「The round log」).

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261116_000050_draft_rounds_by_room"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"CREATE INDEX IF NOT EXISTS "draft_rounds_room" ON "draft_rounds" ("chat_id", "id");"#,
      )
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(r#"DROP INDEX IF EXISTS "draft_rounds_room";"#)
      .await?;

    Ok(())
  }
}

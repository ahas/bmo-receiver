//! What a round read from the brain (`docs/store.md`, 「The tables」; `docs/drafting.md`, 「The round log」): a column
//! of the round log holding, on a round's end, each path read, the index's words for it and how it was read.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261026_000017_draft_reads"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(r#"ALTER TABLE "draft_rounds" ADD COLUMN "reads" text"#)
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(r#"ALTER TABLE "draft_rounds" DROP COLUMN IF EXISTS "reads""#)
      .await?;

    Ok(())
  }
}

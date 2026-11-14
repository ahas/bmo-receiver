//! The program's turns a turn of the draft process spanned (`docs/store.md`, 「The tables」; `docs/drafting.md`, 「The
//! round log」): a column of the round log holding, on a turn's row, each of those turns' answers and what opened it.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261115_000046_draft_versions"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(r#"ALTER TABLE "draft_rounds" ADD COLUMN "versions" text"#)
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(r#"ALTER TABLE "draft_rounds" DROP COLUMN IF EXISTS "versions""#)
      .await?;

    Ok(())
  }
}

//! The situation note as the round that judged a message read it (`docs/store.md`, 「The tables」;
//! `docs/drafting.md`, 「The situation note」): the note's text on the message, and whether that round changed it.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261027_000023_message_situations"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"ALTER TABLE "messages" ADD COLUMN "situation" text;
ALTER TABLE "messages" ADD COLUMN "situation_changed" boolean;"#,
      )
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"ALTER TABLE "messages" DROP COLUMN "situation";
ALTER TABLE "messages" DROP COLUMN "situation_changed";"#,
      )
      .await?;

    Ok(())
  }
}

//! A fixed line the receiver says into a room is kept as one (`docs/store.md`, `messages`; `docs/voiced-notices.md`,
//! 「Lines that stay fixed」): the message carries the line it was said from, its code and fields as JSON text,
//! beside the kind `fixed-line`.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261118_000056_fixed_line_messages"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(r#"ALTER TABLE "messages" ADD COLUMN IF NOT EXISTS "line" text"#)
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(r#"ALTER TABLE "messages" DROP COLUMN IF EXISTS "line""#)
      .await?;

    Ok(())
  }
}

//! A message a sticker came on is a sticker (`docs/room-files.md`, 「Message files」): every message that holds a kept
//! row of kind `sticker` takes that kind, whenever the row was kept.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261117_000053_sticker_messages"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"UPDATE "messages" SET "kind" = 'sticker'
             WHERE "kind" IS NULL
               AND EXISTS (
                 SELECT 1 FROM "collected_items"
                  WHERE "collected_items"."chat_id" = "messages"."chat_id"
                    AND "collected_items"."message_id" = "messages"."message_id"
                    AND "collected_items"."kind" = 'sticker'
               )"#,
      )
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Ok(())
  }
}

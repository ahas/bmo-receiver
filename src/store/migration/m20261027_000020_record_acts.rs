//! The acts the record keeps on its own messages (`docs/store.md`, 「The tables」; `docs/messages.md`, 「What an agent
//! can send back」): a message's kind, when it was edited, deleted and pinned, and the reactions put on it, one to a
//! reactor; and the road an older archive named the record by, named as the record is.

use sea_orm_migration::prelude::*;

/// The table the record's reactions stand in.
pub const TABLE: &str = "message_reactions";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261027_000020_record_acts"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    let db = manager.get_connection();

    db.execute_unprepared(
      r#"ALTER TABLE "messages" ADD COLUMN IF NOT EXISTS "kind" text,
           ADD COLUMN IF NOT EXISTS "edited_at" bigint,
           ADD COLUMN IF NOT EXISTS "deleted_at" bigint,
           ADD COLUMN IF NOT EXISTS "pinned_at" bigint"#,
    )
    .await?;
    db.execute_unprepared(
      r#"CREATE TABLE IF NOT EXISTS "message_reactions" (
           "chat_id" bigint NOT NULL,
           "message_id" bigint NOT NULL,
           "by" text NOT NULL,
           "emoji" text NOT NULL,
           "at" bigint NOT NULL,
           PRIMARY KEY ("chat_id", "message_id", "by")
         )"#,
    )
    .await?;
    db.execute_unprepared(r#"UPDATE "messages" SET "via" = 'record' WHERE "via" = 'console'"#)
      .await?;
    db.execute_unprepared(r#"UPDATE "instructions" SET "via" = 'record' WHERE "via" = 'console'"#)
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    let db = manager.get_connection();

    db.execute_unprepared(r#"DROP TABLE IF EXISTS "message_reactions""#).await?;
    db.execute_unprepared(
      r#"ALTER TABLE "messages" DROP COLUMN IF EXISTS "kind", DROP COLUMN IF EXISTS "edited_at",
           DROP COLUMN IF EXISTS "deleted_at", DROP COLUMN IF EXISTS "pinned_at""#,
    )
    .await?;

    Ok(())
  }
}

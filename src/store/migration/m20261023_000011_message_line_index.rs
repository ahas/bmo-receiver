//! The indexes a room's line and its providers are read by (`docs/store.md`, 「The tables」): a room's messages in
//! the order they were sent, and the roads its messages came in on.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261023_000011_message_line_index"
  }
}

/// The indexes this migration makes.
pub const INDEXES: [&str; 2] = ["messages_line", "messages_via"];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"CREATE INDEX "messages_line" ON "messages" ("chat_id", "sent_at", "rowid");
DROP INDEX IF EXISTS "messages_unnamed";
CREATE INDEX "messages_via" ON "messages" ("chat_id", "via", "rowid") WHERE "via" IS NOT NULL;"#,
      )
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"DROP INDEX IF EXISTS "messages_via";
DROP INDEX IF EXISTS "messages_line";
CREATE INDEX "messages_unnamed" ON "messages" ("chat_id") WHERE "via" IS NULL;"#,
      )
      .await?;

    Ok(())
  }
}

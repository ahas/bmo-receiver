//! The room settings the owner no longer switches from a room (docs/rooms.md): the tools switch, the delegation
//! switch and the listen mode. Their tables go, and the mode's setting row with them.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// The tables this migration takes down.
pub const TABLES: [&str; 2] = ["tool_chats", "delegate_chats"];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    for table in TABLES {
      manager
        .get_connection()
        .execute_unprepared(&format!(r#"DROP TABLE IF EXISTS "{table}""#))
        .await?;
    }

    manager
      .get_connection()
      .execute_unprepared(r#"DELETE FROM "settings" WHERE "key" = 'mode'"#)
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"CREATE TABLE "tool_chats" ("chat_id" bigint NOT NULL, PRIMARY KEY ("chat_id"));
           CREATE TABLE "delegate_chats" ("chat_id" bigint NOT NULL, PRIMARY KEY ("chat_id"));"#,
      )
      .await?;

    Ok(())
  }
}

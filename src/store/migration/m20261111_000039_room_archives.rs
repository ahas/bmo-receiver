//! A room archived and locked (`docs/receiver.md`, 「An archived room」): when it was locked and by whom.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261111_000039_room_archives"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"CREATE TABLE IF NOT EXISTS "room_archives" (
             "chat_id" bigint PRIMARY KEY,
             "archived_at" bigint NOT NULL,
             "archived_by" text NOT NULL
           );"#,
      )
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(r#"DROP TABLE IF EXISTS "room_archives""#)
      .await?;

    Ok(())
  }
}

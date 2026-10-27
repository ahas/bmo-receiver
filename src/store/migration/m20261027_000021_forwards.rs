//! A message the owner forwarded (`docs/store.md`, 「The tables」; `docs/api.md`, 「Sends」): the room, the message
//! and the name of who said the message the copy stands for.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261027_000021_forwards"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"ALTER TABLE "messages" ADD COLUMN IF NOT EXISTS "forwarded_room" bigint,
             ADD COLUMN IF NOT EXISTS "forwarded_message" bigint,
             ADD COLUMN IF NOT EXISTS "forwarded_name" text"#,
      )
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"ALTER TABLE "messages" DROP COLUMN IF EXISTS "forwarded_room", DROP COLUMN IF EXISTS "forwarded_message",
             DROP COLUMN IF EXISTS "forwarded_name""#,
      )
      .await?;

    Ok(())
  }
}

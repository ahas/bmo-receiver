//! A kept row of a sticker takes its own kind (`docs/room-files.md`, 「Message files」): every row kept before the
//! chat service's kind was read whose file is named as a sticker.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261111_000040_sticker_kind"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"UPDATE "collected_items" SET "kind" = 'sticker'
             WHERE "kind" IN ('document', 'video') AND "source" LIKE 'sticker-%'"#,
      )
      .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    Ok(())
  }
}

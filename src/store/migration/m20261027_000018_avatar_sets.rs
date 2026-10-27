//! The pictures set from the console (`docs/store.md`, 「The tables」; `docs/pictures.md`, 「Profile pictures」): one
//! to a room or a person, beside the picture its chat service hands over, the newer of the two standing.

use sea_orm_migration::prelude::*;

/// The table the console-set pictures stand in.
pub const TABLE: &str = "avatar_sets";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261027_000018_avatar_sets"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"CREATE TABLE IF NOT EXISTS "avatar_sets" (
             "kind" text NOT NULL,
             "subject_id" bigint NOT NULL,
             "path" text NOT NULL,
             "set_at" bigint NOT NULL,
             PRIMARY KEY ("kind", "subject_id")
           )"#,
      )
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(r#"DROP TABLE IF EXISTS "avatar_sets""#)
      .await?;

    Ok(())
  }
}

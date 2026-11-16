//! The picture set on an agent from the console, kept under its registration name (`docs/store.md`, 「The
//! tables」; `docs/pictures.md`, 「Profile pictures」).

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261116_000051_agent_avatar_sets"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"CREATE TABLE IF NOT EXISTS "agent_avatar_sets" (
             "agent_name" text PRIMARY KEY,
             "path" text NOT NULL,
             "set_at" bigint NOT NULL
           )"#,
      )
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(r#"DROP TABLE IF EXISTS "agent_avatar_sets""#)
      .await?;

    Ok(())
  }
}

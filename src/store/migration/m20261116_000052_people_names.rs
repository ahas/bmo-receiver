//! The name a person goes by and when it was last given (`docs/store.md`, 「The tables」;
//! `docs/receiver.md`, 「Console rooms and accounts」).

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261116_000052_people_names"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"CREATE TABLE IF NOT EXISTS "people_names" (
             "sender_id" bigint PRIMARY KEY,
             "name" text NOT NULL,
             "named_at" bigint NOT NULL
           )"#,
      )
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(r#"DROP TABLE IF EXISTS "people_names""#)
      .await?;

    Ok(())
  }
}

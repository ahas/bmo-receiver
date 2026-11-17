//! The table of the agency's double slots dropped (`docs/receiver.md`, 「Where it stands」): no round is handed to
//! a double, and the keys it still held are read before this migration runs, to be given back once the service
//! stands.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261117_000054_double_slots_dropped"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(r#"DROP TABLE IF EXISTS "double_slots""#)
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"CREATE TABLE IF NOT EXISTS "double_slots" (
             "key" text PRIMARY KEY,
             "agent" text NOT NULL,
             "chat_id" bigint NOT NULL,
             "asked_at" bigint NOT NULL
           )"#,
      )
      .await?;

    Ok(())
  }
}

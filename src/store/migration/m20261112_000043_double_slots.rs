//! The double's slots the receiver holds (`docs/drafting.md`, 「A round handed to the double」): each slot it asked
//! for and has not seen given back, so `quiet` counts the double's rounds and the start gives back what a stopped
//! process left.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261112_000043_double_slots"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(r#"DROP TABLE IF EXISTS "double_slots""#)
      .await?;

    Ok(())
  }
}

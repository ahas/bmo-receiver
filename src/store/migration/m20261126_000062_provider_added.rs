//! The mark of the addition that set a room's list of providers, stood beside the list itself
//! (`docs/receiver.md`, 「A room's chat providers」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261126_000062_provider_added";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("provider_added.sql")).await?;
    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(include_str!("provider_added.down.sql"))
      .await?;
    Ok(())
  }
}

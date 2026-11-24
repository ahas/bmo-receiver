//! The receiver's schema at 1.0.0, whole: every table with its indexes, and its way down dropping them all.

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261124_000060_baseline";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("baseline.sql")).await?;
    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("baseline.down.sql")).await?;
    Ok(())
  }
}

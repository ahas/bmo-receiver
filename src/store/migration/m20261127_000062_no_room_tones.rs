//! No room carries a tone layer any more: the table that held one is dropped (`docs/store.md`, 「The tables」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261127_000062_no_room_tones";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("no_room_tones.sql")).await?;
    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(include_str!("no_room_tones.down.sql"))
      .await?;
    Ok(())
  }
}

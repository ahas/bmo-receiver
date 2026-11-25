//! A room member's rights over the room's people, stood beside its membership (`docs/receiver.md`, 「Console
//! rooms and accounts」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261125_000061_room_grants";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("room_grants.sql")).await?;
    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(include_str!("room_grants.down.sql"))
      .await?;
    Ok(())
  }
}

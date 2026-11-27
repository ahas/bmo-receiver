//! The supervisors a room has turned off, each standing in the room and answering nothing there (`docs/wake.md`,
//! 「Who reaches the session」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261128_000063_room_supervisors_off";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(include_str!("room_supervisors_off.sql"))
      .await?;
    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(include_str!("room_supervisors_off.down.sql"))
      .await?;
    Ok(())
  }
}

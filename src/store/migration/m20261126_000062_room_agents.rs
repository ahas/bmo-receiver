//! The index a room's standing agents are read off, so the room's registration answers who answers there by walking
//! the agents' lines alone (`docs/api.md`, 「Rooms」).

use sea_orm_migration::prelude::*;

pub const NAME: &str = "m20261126_000062_room_agents";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    NAME
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("room_agents.sql")).await?;
    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(include_str!("room_agents.down.sql"))
      .await?;
    Ok(())
  }
}

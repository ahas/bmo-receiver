//! A room's supervisors (docs/store.md, 「The tables」): the table naming the supervisors that answer a room, each
//! row a room and a supervisor's name.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"ALTER TABLE "room_answerers" RENAME TO "room_supervisors";
           ALTER TABLE "room_supervisors" RENAME COLUMN "agent" TO "supervisor""#,
      )
      .await
      .map(drop)
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"ALTER TABLE "room_supervisors" RENAME COLUMN "supervisor" TO "agent";
           ALTER TABLE "room_supervisors" RENAME TO "room_answerers""#,
      )
      .await
      .map(drop)
  }
}

//! A relay of another room's answer waits on the handed order its line rides (`docs/store.md`, `say_relays`): the
//! order's number stands on the row until the round carrying that order sends its message, and a row naming one
//! relays nothing.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261117_000055_relays_wait_on_orders"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(r#"ALTER TABLE "say_relays" ADD COLUMN IF NOT EXISTS "order_id" bigint"#)
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(r#"ALTER TABLE "say_relays" DROP COLUMN IF EXISTS "order_id""#)
      .await?;

    Ok(())
  }
}

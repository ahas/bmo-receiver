//! An instruction's cause (docs/wake.md, 「Quiet instructions」): a room reply, which opens its agent's turn by
//! itself, or housekeeping, which waits for a round where the session is past its cache lifetime.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(Alias::new("instructions"))
          .add_column_if_not_exists(ColumnDef::new(Alias::new("cause")).text().not_null().default("reply"))
          .to_owned(),
      )
      .await?;

    // A row already marked for the agent alone is housekeeping.
    manager
      .get_connection()
      .execute_unprepared(r#"UPDATE "instructions" SET "cause" = 'housekeeping' WHERE "agent_only""#)
      .await
      .map(drop)
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(Alias::new("instructions"))
          .drop_column(Alias::new("cause"))
          .to_owned(),
      )
      .await
  }
}

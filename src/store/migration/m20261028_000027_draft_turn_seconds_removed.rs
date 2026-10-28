//! The draft process's removed turn bound (docs/drafting.md, 「Time」): no clock bounds a turn of the process, so the
//! column a row set it in goes.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(Alias::new("draft_processes"))
          .drop_column(Alias::new("turn_seconds"))
          .to_owned(),
      )
      .await
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(Alias::new("draft_processes"))
          .add_column(ColumnDef::new(Alias::new("turn_seconds")).big_integer().not_null().default(120))
          .to_owned(),
      )
      .await
  }
}

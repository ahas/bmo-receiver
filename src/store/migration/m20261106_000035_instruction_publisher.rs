//! The unit that published a quiet instruction (docs/bmo-window.md, 「Quiet instruction」): kept with the row so
//! every turn it rides can name where it came from.

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
          .add_column_if_not_exists(ColumnDef::new(Alias::new("publisher")).text())
          .to_owned(),
      )
      .await
      .map(drop)
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(Alias::new("instructions"))
          .drop_column(Alias::new("publisher"))
          .to_owned(),
      )
      .await
      .map(drop)
  }
}

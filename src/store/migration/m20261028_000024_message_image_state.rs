//! A message's picture state (docs/room-files.md, 「Message files」): whether the picture it carried was kept, so a
//! message whose picture could not be written still reads as one that carried a picture rather than as empty words.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(Alias::new("messages"))
          .add_column_if_not_exists(ColumnDef::new(Alias::new("image_state")).text().null())
          .to_owned(),
      )
      .await
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(Alias::new("messages"))
          .drop_column(Alias::new("image_state"))
          .to_owned(),
      )
      .await
  }
}

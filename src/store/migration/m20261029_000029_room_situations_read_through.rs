//! The position a room's situation note has read its room's record to (docs/instruction.md, 「The situation note」):
//! the row of the last message the note took in, so the update turn is handed only what came after it.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(Alias::new("room_situations"))
          .add_column(ColumnDef::new(Alias::new("read_through")).big_integer().null())
          .to_owned(),
      )
      .await
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(Alias::new("room_situations"))
          .drop_column(Alias::new("read_through"))
          .to_owned(),
      )
      .await
  }
}

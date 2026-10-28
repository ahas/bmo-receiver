//! An instruction's agent-only class (docs/bmo-window.md, 「Quiet instruction」): a record the agent reads and no
//! room hears, kept with the row so the round can refuse to send it as a message.

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
          .add_column_if_not_exists(
            ColumnDef::new(Alias::new("agent_only")).boolean().not_null().default(false),
          )
          .to_owned(),
      )
      .await
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .alter_table(
        Table::alter()
          .table(Alias::new("instructions"))
          .drop_column(Alias::new("agent_only"))
          .to_owned(),
      )
      .await
  }
}

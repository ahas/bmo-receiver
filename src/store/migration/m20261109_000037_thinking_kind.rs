//! The thinking messages a record already holds, brought to the shape every later one is kept in (`docs/messages.md`,
//! 「A working turn's thinking」): the kind `thinking` on the row, and its words the block alone, without the marker a
//! chat service's road is sent it under.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261109_000037_thinking_kind"
  }
}

/// The marker those messages opened on, a line of its own before the block.
pub const MARKER: &str = "THINKING.";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(&format!(
        r#"UPDATE "messages" SET "kind" = 'thinking', "text" = substr("text", char_length('{MARKER}') + 2)
           WHERE "kind" IS NULL AND "agent_name" IS NOT NULL AND "round" IS NULL
             AND starts_with("text", '{MARKER}' || chr(10));"#
      ))
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(&format!(
        r#"UPDATE "messages" SET "kind" = NULL, "text" = '{MARKER}' || chr(10) || "text" WHERE "kind" = 'thinking';"#
      ))
      .await?;

    Ok(())
  }
}

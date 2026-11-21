//! A stage text a supervisor holds of its own keeps the message that stands with the keep mark: where a set text
//! still tells the agent to write nothing to keep it, that sentence ends on the keep mark instead, since a turn whose
//! program writes no answer fails the round (docs/instruction.md, 「What it is」). Every other word of the text stays
//! as it was set.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261121_000059_keep_mark_texts"
  }
}

/// The ending a set text kept the message with, and the one it keeps it with now.
pub const WRITTEN_NOTHING: &str = "otherwise write nothing.";
pub const KEPT: &str = "otherwise answer <<KEEP>>.";

// One ending put in the other's place in every set text that holds it.
fn swapped(from: &str, to: &str) -> String {
  format!(
    r#"UPDATE "instruction_texts" SET "text" = REPLACE("text", '{from}', '{to}') WHERE STRPOS("text", '{from}') > 0"#
  )
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(&swapped(WRITTEN_NOTHING, KEPT)).await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(&swapped(KEPT, WRITTEN_NOTHING)).await?;

    Ok(())
  }
}

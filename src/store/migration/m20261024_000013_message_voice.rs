//! The agent and the voice a message the receiver sent itself was archived under (`docs/store.md`, 「The
//! tables」): both null on a message that came in from a provider.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261024_000013_message_voice"
  }
}

/// The columns this migration adds.
pub const COLUMNS: [&str; 2] = ["agent_name", "voice"];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"ALTER TABLE "messages" ADD COLUMN "agent_name" text;
ALTER TABLE "messages" ADD COLUMN "voice" text;"#,
      )
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"ALTER TABLE "messages" DROP COLUMN IF EXISTS "voice";
ALTER TABLE "messages" DROP COLUMN IF EXISTS "agent_name";"#,
      )
      .await?;

    Ok(())
  }
}

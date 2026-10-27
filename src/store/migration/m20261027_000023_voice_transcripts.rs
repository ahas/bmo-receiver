//! The transcript, length and hearing state a voice note's message carries (`docs/messages.md`, 「Every kind of
//! message」; `docs/store.md`, 「The tables」).

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261027_000023_voice_transcripts"
  }
}

/// The columns this migration adds.
pub const COLUMNS: [&str; 3] = ["transcript", "voice_seconds", "voice_state"];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"ALTER TABLE "messages" ADD COLUMN "transcript" text;
ALTER TABLE "messages" ADD COLUMN "voice_seconds" double precision;
ALTER TABLE "messages" ADD COLUMN "voice_state" text;"#,
      )
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"ALTER TABLE "messages" DROP COLUMN IF EXISTS "voice_state";
ALTER TABLE "messages" DROP COLUMN IF EXISTS "voice_seconds";
ALTER TABLE "messages" DROP COLUMN IF EXISTS "transcript";"#,
      )
      .await?;

    Ok(())
  }
}

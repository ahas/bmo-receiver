//! The session a round ran on and what it had read before (docs/store.md, 「The tables」; docs/drafting.md, 「The
//! round log」): two columns of the round log holding, on a round's end, the agency session its turn ran on and how
//! many pieces that session's earlier rounds in the room read in full, and an index of the end rows by room,
//! session and number that the next end reads.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261030_000031_draft_round_sessions"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"ALTER TABLE "draft_rounds" ADD COLUMN "session" text, ADD COLUMN "loaded" bigint;
CREATE INDEX "draft_rounds_session" ON "draft_rounds" ("chat_id", "session", "id") WHERE "kind" = 'end';"#,
      )
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(
        r#"DROP INDEX IF EXISTS "draft_rounds_session";
ALTER TABLE "draft_rounds" DROP COLUMN IF EXISTS "loaded", DROP COLUMN IF EXISTS "session";"#,
      )
      .await?;

    Ok(())
  }
}

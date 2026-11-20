//! The draft process's rows and question texts as each supervisor's own (`docs/store.md`, 「The tables」;
//! `docs/instruction.md`, 「How a supervisor reads the chain」 and 「The stage chain and its texts」): a question's text is kept by the supervisor it was
//! set for, and what stood for every supervisor stands as each one's own, for the supervisors a room names.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261110_000038_draft_per_supervisor"
  }
}

/// The column this migration adds to a question's text.
pub const COLUMN: &str = "agent";

const UP: &str = r#"
ALTER TABLE "draft_questions" ADD COLUMN IF NOT EXISTS "agent" text NOT NULL DEFAULT '';
ALTER TABLE "draft_questions" DROP CONSTRAINT IF EXISTS "draft_questions_pkey";

INSERT INTO "draft_questions" ("agent", "name", "text", "set_at")
SELECT s."supervisor", q."name", q."text", q."set_at"
  FROM "draft_questions" q
 CROSS JOIN (SELECT DISTINCT "supervisor" FROM "room_supervisors") s
 WHERE q."agent" = '';

DELETE FROM "draft_questions" WHERE "agent" = '';
ALTER TABLE "draft_questions" ALTER COLUMN "agent" DROP DEFAULT;
ALTER TABLE "draft_questions" ADD PRIMARY KEY ("agent", "name");

INSERT INTO "draft_processes"
  ("provider", "tier", "agent", "rounds", "send_from", "light", "off", "light_chars", "ceiling_seconds",
   "past_ceiling", "set_at")
SELECT '*', '*', s."supervisor", p."rounds", p."send_from", p."light", p."off", p."light_chars",
       p."ceiling_seconds", p."past_ceiling", p."set_at"
  FROM "draft_processes" p
 CROSS JOIN (SELECT DISTINCT "supervisor" FROM "room_supervisors") s
 WHERE p."provider" = '*' AND p."tier" = '*' AND p."agent" = '*'
ON CONFLICT ("provider", "tier", "agent") DO NOTHING;

DELETE FROM "draft_processes" WHERE NOT ("provider" = '*' AND "tier" = '*' AND "agent" <> '*');
"#;

// One text a name is kept, the first supervisor's by name, and the key is the name alone again.
const DOWN: &str = r#"
DELETE FROM "draft_questions" a USING "draft_questions" b WHERE a."name" = b."name" AND a."agent" > b."agent";
ALTER TABLE "draft_questions" DROP CONSTRAINT IF EXISTS "draft_questions_pkey";
ALTER TABLE "draft_questions" DROP COLUMN IF EXISTS "agent";
ALTER TABLE "draft_questions" ADD PRIMARY KEY ("name");
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(UP).await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(DOWN).await?;

    Ok(())
  }
}

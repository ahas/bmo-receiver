//! Every agent's one number on the record (`docs/receiver.md`, 「Identity, and reaching the owner」): the rooms and
//! numbers an older archive kept under the console's former key are the record's, and where an agent held a
//! number under both keys, the record's stands and the other is folded into it, its words and its picture with it.

use sea_orm_migration::prelude::*;

/// The key an older archive kept the record's rooms and numbers under.
pub const FORMER: &str = "console";

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261027_000019_record_numbers"
  }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    let db = manager.get_connection();

    // A number held under both keys: the agent's words and picture under the former stand under the record's.
    db.execute_unprepared(
      r#"UPDATE "messages" AS "m" SET "sender_id" = "r"."sender_id"
           FROM "provider_users" AS "f" JOIN "provider_users" AS "r" ON "r"."user_id" = "f"."user_id" AND "r"."provider" = 'record'
           WHERE "f"."provider" = 'console' AND "m"."sender_id" = "f"."sender_id""#,
    )
    .await?;
    db.execute_unprepared(
      r#"UPDATE "avatars" AS "a" SET "subject_id" = "r"."sender_id"
           FROM "provider_users" AS "f" JOIN "provider_users" AS "r" ON "r"."user_id" = "f"."user_id" AND "r"."provider" = 'record'
           WHERE "f"."provider" = 'console' AND "a"."kind" = 'person' AND "a"."subject_id" = "f"."sender_id"
             AND NOT EXISTS (SELECT 1 FROM "avatars" AS "b" WHERE "b"."kind" = 'person' AND "b"."subject_id" = "r"."sender_id")"#,
    )
    .await?;
    db.execute_unprepared(
      r#"DELETE FROM "provider_users" AS "f" USING "provider_users" AS "r"
           WHERE "f"."provider" = 'console' AND "r"."provider" = 'record' AND "r"."user_id" = "f"."user_id""#,
    )
    .await?;

    // The rest are the record's as they stand.
    db.execute_unprepared(
      r#"UPDATE "provider_users" SET "provider" = 'record' WHERE "provider" = 'console'"#,
    )
    .await?;
    db.execute_unprepared(
      r#"UPDATE "provider_rooms" AS "f" SET "provider" = 'record'
           WHERE "f"."provider" = 'console'
             AND NOT EXISTS (SELECT 1 FROM "provider_rooms" AS "r" WHERE "r"."provider" = 'record' AND "r"."room_id" = "f"."room_id")"#,
    )
    .await?;

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    // A fold is not undone: which words stood under the former number is no longer told apart.
    Ok(())
  }
}

//! Every time the receiver keeps, in epoch milliseconds (`docs/store.md`, 「The database」): the rows a database
//! already holds are multiplied in place, so a message spoken in the same second as another keeps its place in the
//! room's line.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261020_000007_millisecond_times"
  }
}

// The fallback's beginning is a setting's text, and only a whole number is a time to carry over.
const FALLBACK_SINCE: &str = "fallback_since";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(&format!(
        r#"UPDATE "messages" SET "sent_at" = "sent_at" * 1000;
UPDATE "instructions" SET
  "created_at" = "created_at" * 1000,
  "delivered_at" = "delivered_at" * 1000,
  "expires_at" = "expires_at" * 1000,
  "handed_at" = "handed_at" * 1000,
  "ride_until" = "ride_until" * 1000;
UPDATE "room_windows" SET "open_until" = "open_until" * 1000;
UPDATE "people_reads" SET "read_at" = "read_at" * 1000;
UPDATE "collected_items" SET "collected_at" = "collected_at" * 1000;
UPDATE "avatars" SET "updated_at" = "updated_at" * 1000;
UPDATE "random_draws" SET "created_at" = "created_at" * 1000;
UPDATE "reload_sent" SET "at" = "at" * 1000;
UPDATE "run_records" SET "at" = "at" * 1000;
UPDATE "say_relays" SET
  "created_at" = "created_at" * 1000,
  "expires_at" = "expires_at" * 1000,
  "delivered_at" = "delivered_at" * 1000;
UPDATE "settings" SET "value" = (("value")::bigint * 1000)::text
  WHERE "key" = '{FALLBACK_SINCE}' AND "value" ~ '^-?[0-9]+$';"#
      ))
      .await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager
      .get_connection()
      .execute_unprepared(&format!(
        r#"UPDATE "messages" SET "sent_at" = "sent_at" / 1000;
UPDATE "instructions" SET
  "created_at" = "created_at" / 1000,
  "delivered_at" = "delivered_at" / 1000,
  "expires_at" = "expires_at" / 1000,
  "handed_at" = "handed_at" / 1000,
  "ride_until" = "ride_until" / 1000;
UPDATE "room_windows" SET "open_until" = "open_until" / 1000;
UPDATE "people_reads" SET "read_at" = "read_at" / 1000;
UPDATE "collected_items" SET "collected_at" = "collected_at" / 1000;
UPDATE "avatars" SET "updated_at" = "updated_at" / 1000;
UPDATE "random_draws" SET "created_at" = "created_at" / 1000;
UPDATE "reload_sent" SET "at" = "at" / 1000;
UPDATE "run_records" SET "at" = "at" / 1000;
UPDATE "say_relays" SET
  "created_at" = "created_at" / 1000,
  "expires_at" = "expires_at" / 1000,
  "delivered_at" = "delivered_at" / 1000;
UPDATE "settings" SET "value" = (("value")::bigint / 1000)::text
  WHERE "key" = '{FALLBACK_SINCE}' AND "value" ~ '^-?[0-9]+$';"#
      ))
      .await?;

    Ok(())
  }
}

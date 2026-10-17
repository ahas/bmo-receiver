//! The receiver's twenty-six tables and their indexes, in the shapes its rows have always had
//! (`docs/store.md`, 「The tables」).

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261017_000001_receiver"
  }
}

/// The tables this migration makes, in the order they are dropped.
pub const TABLES: [&str; 26] = [
  "tool_chats",
  "settings",
  "say_relays",
  "room_answerers",
  "reload_sent",
  "reload_pending",
  "random_draws",
  "random_draw_rooms",
  "provider_users",
  "provider_rooms",
  "provider_private_rooms",
  "provider_owner_users",
  "photo_chats",
  "muted_members",
  "messages",
  "instructions",
  "delegate_chats",
  "collected_items",
  "chat_titles",
  "chat_reads",
  "chat_notices",
  "bmo_session_rooms",
  "bmo_rooms",
  "bmo_room_contexts",
  "avatars",
  "allowed_chats",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(include_str!("receiver.sql")).await?;

    Ok(())
  }

  async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    let db = manager.get_connection();

    for table in TABLES {
      db.execute_unprepared(&format!(r#"DROP TABLE IF EXISTS "{table}""#)).await?;
    }

    Ok(())
  }
}

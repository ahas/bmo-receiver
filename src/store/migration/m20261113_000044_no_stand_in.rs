//! A supervisor stands on one road and no stand-in answers for it (`docs/store.md`, 「The settings」): the settings
//! that kept which stand-in answered the default answerer, and since when, go.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
  fn name(&self) -> &str {
    "m20261113_000044_no_stand_in"
  }
}

/// The settings this migration takes out.
pub const KEYS: [&str; 2] = ["fallback_since", "fallback_provider"];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
  async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    for key in KEYS {
      manager
        .get_connection()
        .execute_unprepared(&format!(r#"DELETE FROM "settings" WHERE "key" = '{key}'"#))
        .await?;
    }

    Ok(())
  }

  async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    // A stand-in's answering is gone with the stand-in, so there is nothing to put back.
    Ok(())
  }
}

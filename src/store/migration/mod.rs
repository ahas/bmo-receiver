//! The receiver's migrations, run by SeaORM's migrator, which keeps the ones it ran in its own table beside
//! the receiver's (`docs/store.md`, 「The database」).

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseConnection;

pub mod m20261017_000001_receiver;

/// The table the migrator keeps its record in, which is no part of the receiver's shape.
pub const RECORD_TABLE: &str = "seaql_migrations";

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
  fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(m20261017_000001_receiver::Migration)]
  }
}

/// Every migration a database has not run yet, run on it; how many there were.
pub async fn migrate(db: &DatabaseConnection) -> Result<usize, DbErr> {
  let pending = Migrator::get_pending_migrations(db).await?.len();

  Migrator::up(db, None).await?;

  Ok(pending)
}

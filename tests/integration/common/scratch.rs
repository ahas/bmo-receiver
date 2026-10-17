//! An empty receiver database of a test's own on the suite's PostgreSQL server: the receiver's migrations run
//! once per test binary into a template database, and each test handed a copy of it, which the server makes in a
//! fraction of the time a migration takes. A copy is dropped when its test is done, and the template and copies
//! a binary left behind are dropped by the next binary to make a template.

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};

use bmo_receiver::store::migration::Migrator;
use sea_orm::{
  ConnectOptions, ConnectionTrait, Database as Connector, DatabaseConnection, DbBackend, Statement, Value,
};
use sea_orm_migration::MigratorTrait;
use sqlx::postgres::{PgConnectOptions, PgConnection};
use sqlx::{Connection as _, Executor};

// What a template's and a copy's names begin with, ahead of the process that made them.
const TEMPLATE: &str = "receiver_rs_tmpl_";
const COPY: &str = "receiver_rs_copy_";

static COPIES: AtomicU32 = AtomicU32::new(0);
static MADE: tokio::sync::OnceCell<String> = tokio::sync::OnceCell::const_new();

/// The suite's PostgreSQL server, as the run names it, and the machine's own socket where nothing does.
pub fn test_address() -> String {
  std::env::var("BMO_TEST_POSTGRES")
    .ok()
    .filter(|named| !named.is_empty())
    .unwrap_or_else(|| "postgres:///bmo_test?host=/var/run/postgresql".into())
}

/// Whether a process is alive, read off the kernel's own listing.
pub fn alive(pid: &str) -> bool {
  std::path::Path::new("/proc").join(pid).exists()
}

/// The test server's address with another database in place of its own.
pub fn address_of(database: &str) -> String {
  let address = test_address();
  let named = PgConnectOptions::from_str(&address)
    .unwrap()
    .get_database()
    .unwrap_or("bmo_test")
    .to_owned();
  let (base, query) = address
    .split_once('?')
    .map_or((address.as_str(), None), |(base, query)| (base, Some(query)));
  let base = base.strip_suffix(&named).unwrap_or(base);

  match query {
    Some(query) => format!("{base}{database}?{query}"),
    None => format!("{base}{database}"),
  }
}

async fn server() -> PgConnection {
  PgConnection::connect_with(&PgConnectOptions::from_str(&address_of("postgres")).unwrap())
    .await
    .unwrap()
}

async fn create(admin: &mut PgConnection, database: &str, template: &str) {
  admin
    .execute(format!("CREATE DATABASE \"{database}\" TEMPLATE {template}").as_str())
    .await
    .unwrap();
}

// The template this binary's copies are made from, migrated the first time a test asks. The databases a
// binary that is gone left behind are dropped first.
async fn template() -> String {
  MADE
    .get_or_init(|| async {
      let mut admin = server().await;
      let left: Vec<String> = sqlx::query_scalar(
        "SELECT datname::text FROM pg_database WHERE datname LIKE $1 OR datname LIKE $2",
      )
      .bind(format!("{}%", TEMPLATE.replace('_', "\\_")))
      .bind(format!("{}%", COPY.replace('_', "\\_")))
      .fetch_all(&mut admin)
      .await
      .unwrap();

      for old in left {
        let pid = old
          .trim_start_matches(TEMPLATE)
          .trim_start_matches(COPY)
          .split('_')
          .next()
          .unwrap_or_default();

        if !alive(pid) {
          let _ = admin.execute(format!("DROP DATABASE IF EXISTS \"{old}\" WITH (FORCE)").as_str()).await;
        }
      }

      let name = format!("{TEMPLATE}{}", std::process::id());

      create(&mut admin, &name, "template0 ENCODING 'UTF8' LC_COLLATE 'C' LC_CTYPE 'C'").await;
      admin.close().await.unwrap();

      let db = connect(&name).await;

      Migrator::up(&db, None).await.unwrap();
      db.close().await.unwrap();

      name
    })
    .await
    .clone()
}

/// A connection to a database of the test server.
pub async fn connect(database: &str) -> DatabaseConnection {
  let mut options = ConnectOptions::new(address_of(database));

  options.sqlx_logging(false).max_connections(4);

  Connector::connect(options).await.unwrap()
}

// A new database's name, unique to this process.
fn copy_name() -> String {
  format!("{COPY}{}_{}", std::process::id(), COPIES.fetch_add(1, Ordering::SeqCst))
}

/// A new copy of this binary's migrated, empty database; its name.
pub async fn copy() -> String {
  let template = template().await;

  made(&format!("\"{template}\"")).await
}

/// A new database with nothing in it, not even the migrator's record; its name.
pub async fn blank() -> String {
  made("template0").await
}

async fn made(template: &str) -> String {
  let database = copy_name();
  let mut admin = server().await;

  create(&mut admin, &database, template).await;
  admin.close().await.unwrap();

  database
}

/// A copy dropped, its connections ended with it. It goes on a thread of its own, since a guard dropped inside
/// the runtime may not block on it.
pub fn drop_copy(database: String) {
  let _ = std::thread::spawn(move || {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();

    runtime.block_on(async move {
      let mut admin = server().await;

      let _ = admin
        .execute(format!("DROP DATABASE IF EXISTS \"{database}\" WITH (FORCE)").as_str())
        .await;
      let _ = admin.close().await;
    });
  })
  .join();
}

/// A database of the test's own, dropped when the guard goes.
pub struct Database {
  database: String,
  db: DatabaseConnection,
}

impl Database {
  /// A migrated, empty receiver database.
  pub async fn new() -> Database {
    Self::opened(copy().await).await
  }

  /// A database with nothing in it, for a test that runs the migrations itself.
  pub async fn blank() -> Database {
    Self::opened(blank().await).await
  }

  async fn opened(database: String) -> Database {
    let db = connect(&database).await;

    Database { database, db }
  }

  /// The database's address, as a reader handed one opens it.
  pub fn address(&self) -> String {
    address_of(&self.database)
  }

  pub fn db(&self) -> &DatabaseConnection {
    &self.db
  }

  /// A statement run as written.
  pub async fn run(&self, sql: &str, values: Vec<Value>) {
    self
      .db
      .execute(Statement::from_sql_and_values(DbBackend::Postgres, sql, values))
      .await
      .unwrap();
  }

  /// The texts a statement answers in its first column, in its order.
  pub async fn texts(&self, sql: &str) -> Vec<String> {
    self
      .db
      .query_all(Statement::from_string(DbBackend::Postgres, sql))
      .await
      .unwrap()
      .iter()
      .map(|row| row.try_get_by_index::<String>(0).unwrap())
      .collect()
  }

  /// A count a statement answers.
  pub async fn count(&self, sql: &str, values: Vec<Value>) -> i64 {
    let row = self
      .db
      .query_one(Statement::from_sql_and_values(DbBackend::Postgres, sql, values))
      .await
      .unwrap()
      .unwrap();

    row.try_get_by_index::<i64>(0).unwrap()
  }
}

impl Drop for Database {
  fn drop(&mut self) {
    drop_copy(std::mem::take(&mut self.database));
  }
}

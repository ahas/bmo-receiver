//! What the receiver's integration tests stand on: a test run alone in a process of its own, a receiver database
//! of the test's own, a copy of the one this test binary migrated or a module's shared schema in the suite's test
//! database, migrated by the store's own migrator, and stand-ins for the units a receiver reaches.

#![allow(dead_code)]

pub mod alone;
pub mod queue;
pub mod scratch;
pub mod served_agency;
pub mod session;
pub mod units;

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use bmo_receiver::store::migration::Migrator;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement, Value};
use sea_orm_migration::MigratorTrait;
use sqlx::postgres::{PgConnectOptions, PgConnection};
use sqlx::{Connection as _, Executor};

use scratch::alive;
pub use scratch::test_address;

static MADE: AtomicU32 = AtomicU32::new(0);
// One migration at a time: several side by side in one database contend for the locks the server shares
// with every other database on it.
static MIGRATING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A receiver database, dropped when the guard goes unless it is one a module's tests share.
pub struct Scratch {
  /// The schema the database stands in: a module's shared one, or a copy's own `public`.
  pub schema: String,
  db: DatabaseConnection,
  kept: bool,
  // The database a copy stands in, where it is one.
  copy: Option<String>,
}

/// The schema one module's tests share, migrated and seeded once. Each module that shares one keeps its own in
/// a static, since the modules of this binary seed theirs differently.
pub struct SharedSchema(tokio::sync::OnceCell<String>);

impl SharedSchema {
  pub const fn new() -> SharedSchema {
    SharedSchema(tokio::sync::OnceCell::const_new())
  }

  /// The schema, migrated and seeded by the first test to ask, each test on a connection of its own, since a
  /// pool lives no longer than the test that made it.
  pub async fn get<F, Seeded>(&self, seed: F) -> Scratch
  where
    F: FnOnce(Scratch) -> Seeded,
    Seeded: Future<Output = Scratch>,
  {
    let schema = self
      .0
      .get_or_init(|| async { seed(Scratch::made(true).await).await.schema.clone() })
      .await
      .clone();

    Scratch {
      db: connect(&schema).await,
      schema,
      kept: true,
      copy: None,
    }
  }
}

impl Scratch {
  /// A migrated, empty receiver database of the test's own: a copy of the one this binary migrated.
  pub async fn new() -> Scratch {
    Self::copied(scratch::copy().await).await
  }

  /// A database of the test's own with nothing in it, not even the migrator's record, for a test that runs the
  /// migrations itself.
  pub async fn blank() -> Scratch {
    Self::copied(scratch::blank().await).await
  }

  async fn copied(copy: String) -> Scratch {
    Scratch {
      schema: "public".to_owned(),
      db: scratch::connect(&copy).await,
      kept: false,
      copy: Some(copy),
    }
  }

  /// The database's address, as a reader handed one opens it.
  pub fn address(&self) -> String {
    match &self.copy {
      Some(copy) => scratch::address_of(copy),
      None => {
        let address = test_address();
        let joint = if address.contains('?') { '&' } else { '?' };

        format!("{address}{joint}options=-c%20search_path%3D{}%2Cpublic", self.schema)
      }
    }
  }

  /// A name no other database of the run carries.
  pub fn name(&self) -> &str {
    self.copy.as_deref().unwrap_or(&self.schema)
  }

  async fn made(kept: bool) -> Scratch {
    let options = PgConnectOptions::from_str(&test_address()).unwrap();
    let database = options.get_database().unwrap_or("bmo_test").to_owned();
    let mut server = PgConnection::connect_with(&options.clone().database("postgres")).await.unwrap();
    let exists: Option<i32> = sqlx::query_scalar("SELECT 1 FROM pg_database WHERE datname = $1")
      .bind(&database)
      .fetch_optional(&mut server)
      .await
      .unwrap();

    if exists.is_none() {
      let made = server
        .execute(
          format!(
            "CREATE DATABASE \"{database}\" TEMPLATE template0 ENCODING 'UTF8' LC_COLLATE 'C' LC_CTYPE 'C'"
          )
          .as_str(),
        )
        .await;

      if let Err(e) = made {
        assert!(e.to_string().contains("already exists"), "{e}");
      }
    }
    server.close().await.unwrap();

    let mut admin = PgConnection::connect_with(&options).await.unwrap();
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let made = MADE.fetch_add(1, Ordering::SeqCst);
    let schema = if kept {
      format!("k_{}_{seconds}_{made}", std::process::id())
    } else {
      format!("s_{seconds}_{}_{made}", std::process::id())
    };

    // A shared schema outlives its binary, so the next one to make one drops those whose process is gone.
    if kept {
      let left: Vec<String> =
        sqlx::query_scalar("SELECT nspname::text FROM pg_namespace WHERE nspname LIKE $1")
          .bind("k\\_%")
          .fetch_all(&mut admin)
          .await
          .unwrap();

      for old in left {
        if old.split('_').nth(1).is_some_and(|pid| !alive(pid)) {
          let _ = admin.execute(format!("DROP SCHEMA IF EXISTS \"{old}\" CASCADE").as_str()).await;
        }
      }
    }

    admin.execute(format!("CREATE SCHEMA \"{schema}\"").as_str()).await.unwrap();
    admin.close().await.unwrap();

    let db = connect(&schema).await;

    {
      let _turn = MIGRATING.lock().await;

      Migrator::up(&db, None).await.unwrap();
    }

    Scratch {
      schema,
      db,
      kept,
      copy: None,
    }
  }

  pub fn db(&self) -> &DatabaseConnection {
    &self.db
  }

  /// A statement run as written.
  pub async fn run(&self, sql: &str, values: Vec<Value>) {
    self
      .db()
      .execute(Statement::from_sql_and_values(DbBackend::Postgres, sql, values))
      .await
      .unwrap();
  }

  /// A message standing in a room, sent by a sender.
  pub async fn message(&self, room: i64, message: i64, sender: i64) {
    self
      .run(
        r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text")
           VALUES ($1, $2, $3, 'someone', 1000, 'words')"#,
        vec![room.into(), message.into(), sender.into()],
      )
      .await;
  }

  /// A count a statement answers.
  pub async fn count(&self, sql: &str, values: Vec<Value>) -> i64 {
    let row = self
      .db()
      .query_one(Statement::from_sql_and_values(DbBackend::Postgres, sql, values))
      .await
      .unwrap()
      .unwrap();

    row.try_get_by_index::<i64>(0).unwrap()
  }

  /// The texts a statement answers in its first column, in its order.
  pub async fn texts(&self, sql: &str) -> Vec<String> {
    self
      .db()
      .query_all(Statement::from_string(DbBackend::Postgres, sql))
      .await
      .unwrap()
      .iter()
      .map(|row| row.try_get_by_index::<String>(0).unwrap())
      .collect()
  }
}

impl Drop for Scratch {
  fn drop(&mut self) {
    if self.kept {
      return;
    }

    if let Some(copy) = self.copy.take() {
      if let Err(error) = scratch::drop_copy(copy)
        && !std::thread::panicking()
      {
        panic!("the test's database is dropped: {error}");
      }

      return;
    }

    let schema = self.schema.clone();

    // The schema goes on a thread of its own, since a guard dropped inside the runtime may not block on it.
    let _ = std::thread::spawn(move || {
      let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();

      runtime.block_on(async move {
        let options = PgConnectOptions::from_str(&test_address()).unwrap();

        if let Ok(mut admin) = PgConnection::connect_with(&options).await {
          let _ = admin.execute(format!("DROP SCHEMA \"{schema}\" CASCADE").as_str()).await;
          let _ = admin.close().await;
        }
      });
    })
    .join();
  }
}

/// A connection reading the scratch schema first, and `public`.
pub async fn connect(schema: &str) -> DatabaseConnection {
  let mut options = ConnectOptions::new(test_address());

  options
    .sqlx_logging(false)
    .max_connections(4)
    .set_schema_search_path(format!("\"{schema}\", public"));

  Database::connect(options).await.unwrap()
}

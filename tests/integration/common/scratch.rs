//! An empty receiver database of a test's own on the suite's PostgreSQL server, as a database of its own: the
//! receiver's migrations run once per test process into a template database, and each test handed a copy of it,
//! which the server makes in a fraction of the time a migration takes. A copy is dropped when its test is done,
//! and a test whose copy cannot be dropped fails. As a process makes its first database it starts dropping, behind
//! its tests, the templates and copies that processes of this harness which no longer run left behind. The guard a
//! test holds is `common::Scratch`.
//!
//! A drop ends only once every process of the server has taken the signal it sends, and an idle I/O worker of
//! PostgreSQL 18 can leave it untaken until other work on the server wakes it, so a drop made while nothing else
//! reads can hold for minutes. Making a database therefore never waits on one: the sweep goes on a thread of its
//! own that nothing waits for. A test waits for its own copy's drop for a bound, past which the drop goes on behind
//! it and a copy it leaves standing is the next process's sweep's.

use std::str::FromStr;
use std::sync::Once;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use bmo_receiver::store::migration::Migrator;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;
use sqlx::postgres::{PgConnectOptions, PgConnection};
use sqlx::{Connection as _, Executor};

// What a template's and a copy's names begin with, ahead of the process that made them.
const TEMPLATE: &str = "receiver_rs_tmpl_";
const COPY: &str = "receiver_rs_copy_";
// How many times a drop ends the database's sessions and tries again before it gives up.
const DROP_ATTEMPTS: usize = 3;
// How long a test waits for its copy's drop before the drop goes on behind it.
const DROP_BOUND: Duration = Duration::from_secs(10);

static COPIES: AtomicU32 = AtomicU32::new(0);
static SWEPT: Once = Once::new();
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
  swept();
  admin
    .execute(format!("CREATE DATABASE \"{database}\" TEMPLATE {template}").as_str())
    .await
    .unwrap();
}

// The template this process's copies are made from, migrated the first time a test asks.
async fn template() -> String {
  MADE
    .get_or_init(|| async {
      let name = format!("{TEMPLATE}{}", std::process::id());
      let mut admin = server().await;

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

  Database::connect(options).await.unwrap()
}

// A new database's name, unique to this process.
fn copy_name() -> String {
  format!("{COPY}{}_{}", std::process::id(), COPIES.fetch_add(1, Ordering::SeqCst))
}

/// A new copy of this process's migrated, empty database; its name.
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

/// A copy dropped, its connections ended with it; what kept it standing where it could not be. It goes on a
/// thread of its own, since a guard dropped inside the runtime may not block on it, and is waited for up to
/// `DROP_BOUND`: a drop the server still holds then goes on behind the caller, said on the error output, and
/// counts as made.
pub fn drop_copy(database: String) -> Result<(), String> {
  let (told, heard) = mpsc::channel();
  let name = database.clone();

  std::thread::spawn(move || {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let dropped = runtime.block_on(async move {
      let mut admin =
        PgConnection::connect_with(&PgConnectOptions::from_str(&address_of("postgres")).unwrap())
          .await
          .map_err(|error| format!("{database}: {error}"))?;
      let dropped = drop_database(&mut admin, &database)
        .await
        .map_err(|error| format!("{database}: {error}"));
      let _ = admin.close().await;

      dropped
    });

    let _ = told.send(dropped);
  });

  match heard.recv_timeout(DROP_BOUND) {
    Ok(dropped) => dropped,
    Err(RecvTimeoutError::Timeout) => {
      eprintln!("{name} is still being dropped after {DROP_BOUND:?}; the drop goes on behind the test");

      Ok(())
    }
    Err(RecvTimeoutError::Disconnected) => Err("the drop's thread panicked".to_owned()),
  }
}

/// Drops `name`. This role's own sessions on it are ended first, and the drop is a plain one: a drop that forces
/// every session off needs the right to end other roles' processes, an autovacuum worker's among them, which
/// the suite's role need not hold, while the plain drop stops an autovacuum worker itself. A session of this
/// role that opened between the two is ended on the next attempt.
async fn drop_database(admin: &mut PgConnection, name: &str) -> Result<(), sqlx::Error> {
  let mut attempt = 1;

  loop {
    sqlx::query(
      "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
       WHERE datname = $1 AND pid <> pg_backend_pid() AND usename = current_user",
    )
    .bind(name)
    .execute(&mut *admin)
    .await?;

    match admin.execute(format!("DROP DATABASE IF EXISTS \"{name}\"").as_str()).await {
      Ok(_) => return Ok(()),
      Err(error) if attempt == DROP_ATTEMPTS => return Err(error),
      Err(_) => attempt += 1,
    }
  }
}

// The sweep, started once as this process makes its first database, on a thread no test waits for.
fn swept() {
  SWEPT.call_once(|| {
    std::thread::spawn(|| {
      tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(sweep());
    });
  });
}

/// Drops the templates and copies this role owns that a process of this harness left behind once it no longer
/// runs. A database is left alone while its process lives or a client's session is on it (an autovacuum worker
/// is no client, and the plain drop stops it), so a run of another checkout's harness against the same server
/// keeps its own, and only this harness's two names are read. A drop that fails is told on the test's error
/// output and the database left to the next sweep.
pub async fn sweep() {
  let mut admin = server().await;
  let left: Vec<String> = sqlx::query_scalar(
    "SELECT datname::text FROM pg_database d \
     WHERE (starts_with(datname, $1) OR starts_with(datname, $2)) \
     AND pg_get_userbyid(datdba) = current_user \
     AND NOT EXISTS (SELECT 1 FROM pg_stat_activity a \
       WHERE a.datname = d.datname AND a.backend_type = 'client backend')",
  )
  .bind(TEMPLATE)
  .bind(COPY)
  .fetch_all(&mut admin)
  .await
  .unwrap();

  for name in left {
    if process_of(&name).is_none_or(|process| alive(&process.to_string())) {
      continue;
    }

    if let Err(error) = drop_database(&mut admin, &name).await {
      eprintln!("a dead process's scratch database {name} is not dropped: {error}");
    }
  }

  admin.close().await.unwrap();
}

/// The process id a template's or a copy's name carries, if the name is one this harness makes.
pub fn process_of(name: &str) -> Option<u32> {
  if let Some(process) = name.strip_prefix(TEMPLATE) {
    return process.parse().ok();
  }

  let (process, count) = name.strip_prefix(COPY)?.split_once('_')?;
  count.parse::<u32>().ok()?;

  process.parse().ok()
}

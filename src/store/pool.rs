//! The pools the receiver opens on its database, how a statement prepared before the database's shape moved
//! under them is prepared again, and the one way a statement is bound and run on them (`docs/store.md`,
//! 「The database」).

use std::future::Future;
use std::str::FromStr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use sea_orm::{
  ConnectionTrait, DatabaseConnection, DbBackend, DbErr, QueryResult, RuntimeErr, SqlxPostgresConnector,
  Statement, Value,
};
use sqlx::ConnectOptions;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

/// Why work on the database stopped, in a sentence a caller can pass on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fault {
  pub detail: String,
}

impl Fault {
  pub fn new(detail: impl Into<String>) -> Self {
    Self {
      detail: detail.into(),
    }
  }
}

impl std::fmt::Display for Fault {
  fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    out.write_str(&self.detail)
  }
}

impl std::error::Error for Fault {}

impl From<DbErr> for Fault {
  fn from(error: DbErr) -> Self {
    Self::new(format!("the receiver's database: {error}"))
  }
}

/// One statement with its values, read as PostgreSQL reads it, every number at its widest.
pub fn statement(sql: &str, values: Vec<Value>) -> Statement {
  Statement::from_sql_and_values(DbBackend::Postgres, sql, values.into_iter().map(widest))
}

// A number at the one width each kind is bound at. The driver prepares a text once per connection with
// the types its first values had, and a value of another width bound to it later is refused.
fn widest(value: Value) -> Value {
  match value {
    Value::TinyInt(n) => Value::BigInt(n.map(i64::from)),
    Value::SmallInt(n) => Value::BigInt(n.map(i64::from)),
    Value::Int(n) => Value::BigInt(n.map(i64::from)),
    Value::TinyUnsigned(n) => Value::BigInt(n.map(i64::from)),
    Value::SmallUnsigned(n) => Value::BigInt(n.map(i64::from)),
    Value::Unsigned(n) => Value::BigInt(n.map(i64::from)),
    Value::Float(n) => Value::Double(n.map(f64::from)),
    other => other,
  }
}

/// The one row a statement answers, or none.
pub async fn one<C: ConnectionTrait>(
  db: &C,
  sql: &str,
  values: Vec<Value>,
) -> Result<Option<QueryResult>, Fault> {
  let statement = statement(sql, values);

  Ok(replanned(|| db.query_one(statement.clone())).await?)
}

/// Every row a statement answers.
pub async fn all<C: ConnectionTrait>(
  db: &C,
  sql: &str,
  values: Vec<Value>,
) -> Result<Vec<QueryResult>, Fault> {
  let statement = statement(sql, values);

  Ok(replanned(|| db.query_all(statement.clone())).await?)
}

/// A statement run for what it writes.
pub async fn run<C: ConnectionTrait>(db: &C, sql: &str, values: Vec<Value>) -> Result<u64, Fault> {
  let statement = statement(sql, values);

  Ok(replanned(|| db.execute(statement.clone())).await?.rows_affected())
}

// PostgreSQL's code for a prepared statement whose table moved under it: "cached plan must not
// change result type" once a column is added to what a `SELECT *` reads.
const PLAN_OUTLIVED: &str = "0A000";

// When this process last met a statement prepared on the database's shape before it moved.
static SHAPE_MOVED_AT: Mutex<Option<Instant>> = Mutex::new(None);

/// Whether an error is a statement prepared before the database's shape moved under it.
pub fn plan_outlived(error: &DbErr) -> bool {
  match error {
    DbErr::Query(RuntimeErr::SqlxError(sqlx::Error::Database(fault)))
    | DbErr::Exec(RuntimeErr::SqlxError(sqlx::Error::Database(fault))) => {
      fault.code().is_some_and(|code| code == PLAN_OUTLIVED)
    }
    _ => false,
  }
}

/// Marks every connection this process opened until now as prepared on the shape before: each is
/// closed as it is next handed out, and its prepared statements with it.
pub fn shape_moved() {
  *SHAPE_MOVED_AT.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Instant::now());
}

// Whether a connection of this age was opened before the last mark.
fn opened_before_move(age: Duration) -> bool {
  SHAPE_MOVED_AT
    .lock()
    .unwrap_or_else(|poisoned| poisoned.into_inner())
    .is_some_and(|at| at.elapsed() <= age)
}

/// A pool of at most `max` connections on the database, which never hands out a connection opened
/// before the database's shape last moved under this process.
pub fn pool(max: u32) -> PgPoolOptions {
  PgPoolOptions::new()
    .max_connections(max)
    .before_acquire(|_, meta| Box::pin(async move { Ok(!opened_before_move(meta.age)) }))
}

fn options(address: &str) -> Result<PgConnectOptions, sqlx::Error> {
  Ok(PgConnectOptions::from_str(address)?.disable_statement_logging())
}

/// The database at a PostgreSQL address, through a pool `pool` built.
pub async fn connect(address: &str, pool: PgPoolOptions) -> Result<DatabaseConnection, sqlx::Error> {
  let pool = pool.connect_with(options(address)?).await?;

  Ok(SqlxPostgresConnector::from_sqlx_postgres_pool(pool))
}

/// The same, connecting nothing until the first statement asks.
pub fn connect_lazy(address: &str, pool: PgPoolOptions) -> Result<DatabaseConnection, sqlx::Error> {
  Ok(SqlxPostgresConnector::from_sqlx_postgres_pool(
    pool.connect_lazy_with(options(address)?),
  ))
}

/// Work on the database run once more where it met a statement prepared before the database's shape
/// moved, after marking the shape moved, so the second run stands on a connection that prepares it
/// afresh. A second run that fails too answers the first error: inside a transaction it can only
/// meet the transaction already aborted.
pub async fn replanned<T, F, Fut>(mut work: F) -> Result<T, DbErr>
where
  F: FnMut() -> Fut,
  Fut: Future<Output = Result<T, DbErr>>,
{
  match work().await {
    Err(error) if plan_outlived(&error) => {
      shape_moved();
      work().await.map_err(|_| error)
    }
    answer => answer,
  }
}

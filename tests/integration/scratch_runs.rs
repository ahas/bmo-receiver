//! The databases the tests stand on (`common::Scratch`): a copy of the migrated database of a test's own, and a
//! module's shared schema, seeded once; a copy dropped with its guard, and the sweep of what a dead process left.

use bmo_receiver::store::{one, pool};

use sqlx::postgres::PgConnection;
use sqlx::{Connection as _, Executor};

use crate::integration::common::{Scratch, SharedSchema, scratch};

const MESSAGES: &str = r#"SELECT COUNT(*) FROM "messages""#;

static SHARED: SharedSchema = SharedSchema::new();

#[tokio::test]
async fn a_scratch_database_takes_a_message_row_and_keeps_it_to_itself() {
  let scratch = Scratch::new().await;
  let other = Scratch::new().await;

  scratch.message(-7, 1, 42).await;

  assert_eq!(scratch.count(MESSAGES, vec![]).await, 1);
  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "messages" WHERE "chat_id" = $1 AND "sender_id" = $2"#,
        vec![(-7i64).into(), 42i64.into()]
      )
      .await,
    1
  );
  assert_eq!(other.count(MESSAGES, vec![]).await, 0, "each test's copy is its own");
}

#[tokio::test]
async fn a_shared_schema_is_seeded_once_and_read_at_its_address() {
  let seed = |scratch: Scratch| async move {
    scratch.message(-8, 1, 42).await;
    scratch
  };
  let first = SHARED.get(seed).await;
  let second = SHARED.get(seed).await;

  assert_eq!(first.schema, second.schema);
  assert_eq!(second.count(MESSAGES, vec![]).await, 1, "the seed ran once");

  let reader = pool::connect(&second.address(), pool::pool(1)).await.unwrap();
  let row = one(&reader, MESSAGES, vec![]).await.unwrap().unwrap();

  assert_eq!(row.try_get_by_index::<i64>(0).unwrap(), 1, "the address reads the shared schema");
}

// The databases on the suite's server whose names are among `names`.
async fn standing(names: &[&str]) -> Vec<String> {
  let mut admin = server().await;
  let mut found: Vec<String> =
    sqlx::query_scalar("SELECT datname::text FROM pg_database WHERE datname = ANY($1)")
      .bind(names)
      .fetch_all(&mut admin)
      .await
      .unwrap();

  admin.close().await.unwrap();
  found.sort();

  found
}

async fn server() -> PgConnection {
  PgConnection::connect(&scratch::address_of("postgres")).await.unwrap()
}

#[tokio::test]
async fn a_tests_copy_is_gone_from_the_server_once_its_guard_is() {
  let scratch = Scratch::new().await;
  let name = scratch.name().to_owned();

  assert_eq!(standing(&[&name]).await, std::slice::from_ref(&name));
  drop(scratch);
  assert!(standing(&[&name]).await.is_empty(), "the copy is dropped with its guard");
}

#[tokio::test]
async fn the_sweep_drops_only_this_harness_s_databases_whose_process_is_gone() {
  // No process runs under the largest id; this one runs under its own.
  let gone = u32::MAX;
  let living = std::process::id();
  let dead_copy = format!("receiver_rs_copy_{gone}_0");
  let dead_template = format!("receiver_rs_tmpl_{gone}");
  let living_copy = format!("receiver_rs_copy_{living}_{}", u32::MAX);
  let elsewhere = format!("elsewhere_rs_copy_{gone}_0");
  let all = [dead_copy.as_str(), &dead_template, &living_copy, &elsewhere];
  let mut admin = server().await;

  for name in all {
    scratch::drop_copy(name.to_owned()).unwrap();
    admin
      .execute(format!("CREATE DATABASE \"{name}\" TEMPLATE template0").as_str())
      .await
      .unwrap();
  }
  admin.close().await.unwrap();

  scratch::sweep().await;

  let mut kept = vec![living_copy.clone(), elsewhere.clone()];

  kept.sort();
  assert_eq!(standing(&all).await, kept, "a living process's and another harness's databases stay");

  for name in kept {
    scratch::drop_copy(name).unwrap();
  }
}

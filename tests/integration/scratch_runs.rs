//! The databases the tests stand on (`common::Scratch`): a copy of the migrated database of a test's own, and a
//! module's shared schema, seeded once.

use bmo_receiver::store::{one, pool};

use crate::integration::common::{Scratch, SharedSchema};

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

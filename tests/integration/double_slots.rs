//! The double's slots a store still keeps (docs/receiver.md, 「Where it stands」; docs/boundary-api.md, `quiet`):
//! `quiet` counts each as a round under way while the table stands, the start reads them and gives each back once
//! by its key, and a store without the table, as the migrations leave it, holds none.

use bmo_receiver::chat::double_slots::{Held, give_back, held};
use bmo_receiver::quiet::asked;

use crate::integration::common::Scratch;
use crate::integration::common::units::{AGENT, OWNER, StandInAgency};

fn slot(key: &str, asked_at: i64) -> Held {
  Held {
    key: key.to_owned(),
    agent: AGENT.to_owned(),
    chat_id: OWNER,
    asked_at,
  }
}

#[tokio::test]
async fn a_store_still_keeping_the_double_s_slots_counts_each_and_the_start_gives_each_back_once() {
  let scratch = Scratch::new().await;
  let agency = StandInAgency::new();

  // The store a running release keeps before the migration that drops the table.
  scratch
    .run(
      r#"CREATE TABLE "double_slots" (
           "key" text PRIMARY KEY, "agent" text NOT NULL, "chat_id" bigint NOT NULL, "asked_at" bigint NOT NULL
         )"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "double_slots" ("key", "agent", "chat_id", "asked_at") VALUES ($1, $3, $4, 30), ($2, $3, $4, 10)"#,
      vec!["slot-late".into(), "slot-early".into(), AGENT.into(), OWNER.into()],
    )
    .await;

  let under_way = asked(&agency, scratch.db(), false).await.unwrap();

  assert_eq!(under_way.len(), 2, "{under_way:?}");
  assert!(
    under_way[0].contains("slot-early") && under_way[1].contains("slot-late"),
    "{under_way:?}"
  );

  let left = held(scratch.db()).await.unwrap();

  assert_eq!(left, [slot("slot-early", 10), slot("slot-late", 30)]);

  give_back(&agency, &left).await;

  assert_eq!(*agency.double_releases.lock().unwrap(), ["slot-early", "slot-late"]);
}

#[tokio::test]
async fn a_store_the_migrations_left_holds_no_slot_of_the_double() {
  let scratch = Scratch::new().await;

  assert_eq!(held(scratch.db()).await.unwrap(), Vec::<Held>::new());
  assert_eq!(asked(&StandInAgency::new(), scratch.db(), false).await.unwrap(), Vec::<String>::new());
}

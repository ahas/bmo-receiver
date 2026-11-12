//! The double's slots the receiver holds (docs/drafting.md, 「A round handed to the double」): `quiet` counts each
//! as a round under way, and the start gives back, by its key, every slot a stopped process left, keeping those of
//! its own rounds.

use bmo_receiver::chat::double_slots::{Held, held, keep, release_left};
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
async fn a_held_slot_counts_as_under_way_until_the_next_start_gives_it_back() {
  let scratch = Scratch::new().await;
  let agency = StandInAgency::new();

  keep(scratch.db(), &slot("slot-left", 10)).await.unwrap();
  keep(scratch.db(), &slot("slot-own", 30)).await.unwrap();

  let under_way = asked(&agency, scratch.db(), false).await.unwrap();

  assert_eq!(under_way.len(), 2, "{under_way:?}");
  assert!(under_way[0].contains("slot-left") && under_way[1].contains("slot-own"), "{under_way:?}");

  // The start at 20 gives back what a process before it left, and keeps what it asked itself.
  release_left(scratch.db(), &agency, 20).await;

  assert_eq!(*agency.double_releases.lock().unwrap(), ["slot-left"]);
  assert_eq!(held(scratch.db()).await.unwrap(), [slot("slot-own", 30)]);
}

#[tokio::test]
async fn a_store_not_yet_migrated_to_hold_slots_holds_none() {
  let scratch = Scratch::new().await;

  // The store the running release keeps, read by the release about to go live before its migrations run.
  scratch.run(r#"DROP TABLE "double_slots""#, vec![]).await;

  assert_eq!(asked(&StandInAgency::new(), scratch.db(), false).await.unwrap(), Vec::<String>::new());
}

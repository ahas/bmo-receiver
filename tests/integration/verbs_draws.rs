//! The fair draw of the receiver's verbs over a scratch database (docs/random-draw.md): a draw made in a room,
//! its chain held across draws, and a request refused before any seed is spent. The binary's own verbs are read
//! where the binary stands, in its tests.

use std::sync::Arc;
use tempfile::tempdir;

use serde_json::{Map, Value};

use bmo_receiver::data_root::DataRoot;
use bmo_receiver::verbs::sends::Sends;
use bmo_receiver::verbs::{
  VerbError,
  draws::{commitment_of, draw, verify_recorded},
};

use crate::common::Scratch;
use crate::common::units::{ADDRESS, AGENT, OWNER, SENDER, StandInChat, registration, standing};

const ROOM: i64 = -300;
const SEED: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
const COMMITMENT: &str = "6c86c6aac5fb24bcf5d9939cb7d7d5645ce39418f449e03b262dd4fa14b4b92b";

fn spec(written: &str) -> Map<String, Value> {
  serde_json::from_str(written).unwrap()
}

// A receiver over a scratch database, with one road holding the room.
async fn stood() -> (Scratch, Arc<StandInChat>, Sends) {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let road = Arc::new(StandInChat::new("road-a").holding(&[ROOM]).owners(OWNER));
  let standing = standing(scratch.db(), DataRoot::new(data.path()), [road.clone()].to_vec());

  let mut agent = registration(AGENT, SENDER, [ADDRESS].to_vec(), None, true);

  agent.row.insert("role".to_owned(), serde_json::json!("chat"));
  standing.agency.registering([agent].to_vec());

  (scratch, road, Sends::new(standing.receiver.clone(), None, None))
}

#[tokio::test]
async fn a_draw_posts_its_line_answers_its_result_and_its_successor_answers_the_commitment_it_left() {
  let (scratch, road, sends) = stood().await;
  let first = draw(
    &sends,
    ROOM,
    Some("sync"),
    &spec(r#"{"shape":"dice","sides":6,"label":"who goes first"}"#),
    None,
  )
  .await
  .unwrap();

  assert!(first[0].starts_with("draw 1: "), "{first:?}");
  assert!(first[2].contains("it is the room's first draw"), "{first:?}");

  let second = draw(
    &sends,
    ROOM,
    Some("async"),
    &spec(r#"{"shape":"range","min":1,"max":10}"#),
    Some("road-a"),
  )
  .await
  .unwrap();

  assert!(second[0].starts_with("draw 2 is made"), "{second:?}");
  assert_eq!(
    scratch
      .count(r#"SELECT COUNT(*) FROM "instructions" WHERE "chat_id" = $1 AND "sort" = 'random_draw' AND "via" = 'road-a'"#, [ROOM.into()].to_vec())
      .await,
    1,
  );

  // The room line of each draw went out on the room's road, in the road's own voice.
  assert_eq!(road.sent().len(), 2);

  let verdict = verify_recorded(scratch.db(), 2).await.unwrap();

  assert!(verdict.matched, "{:?}", verdict.lines);
}

#[tokio::test]
async fn a_request_is_refused_before_any_seed_is_spent() {
  let (scratch, _road, sends) = stood().await;

  for (mode, written, says) in [
    (Some("later"), r#"{"shape":"dice","sides":6}"#, "mode must be one of sync, async"),
    (Some("sync"), r#"{"shape":"dice"}"#, "a dice draw needs sides"),
  ] {
    let Err(VerbError::Refused(why)) = draw(&sends, ROOM, mode, &spec(written), None).await else {
      panic!("not refused: {written}");
    };

    assert_eq!(why, says, "{why}");
  }

  assert_eq!(scratch.count(r#"SELECT COUNT(*) FROM "random_draws""#, vec![]).await, 0);
  assert_eq!(scratch.count(r#"SELECT COUNT(*) FROM "random_draw_rooms""#, vec![]).await, 0);
}

#[tokio::test]
async fn the_published_chain_answers_a_commitment_and_a_result_the_method_does_not_draw_is_refused() {
  assert_eq!(commitment_of(SEED), COMMITMENT, "{SEED}");
}

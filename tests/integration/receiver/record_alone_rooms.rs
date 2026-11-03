//! A room the record holds alone (docs/receiver.md, 「A room's chat providers」): one whose list names the record
//! alone, and one the record numbered whose list was never chosen, is asked of no chat service, so the owner's
//! words there wake its answerer while every road is down; a room a road may hold still asks that road.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use axum::{
  Router,
  body::{Body, to_bytes},
  extract::Request,
  http::StatusCode,
};
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

use crate::integration::common::Scratch;
use crate::integration::common::units::{
  ADDRESS, AGENT, SENDER, StandInChat, Standing, registration, standing, woken,
};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::roads::{Road, providers};
use bmo_receiver::routes::routes;

const TOKEN: &str = "the-token";
// A room whose list names the record alone, one the record numbered with no list chosen, and one the record
// knows by its title alone, which a road may hold.
const CHOSEN: i64 = -9000000000003;
const NUMBERED: i64 = -9000000000002;
const TITLED: i64 = -180;

// A receiver over a scratch database, judging by its own wake, whose one chat service nothing listens for, and the
// three rooms, each with the default answerer as its one answerer.
async fn stood() -> (Scratch, TempDir, Standing, Arc<StandInChat>, Router) {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let road = Arc::new(StandInChat::new("road-a").down());
  let standing = woken(standing(scratch.db(), DataRoot::new(data.path()), vec![road.clone()]));

  standing.agency.registering(vec![registration(AGENT, SENDER, vec![ADDRESS], None)]);

  for chat_id in [CHOSEN, NUMBERED, TITLED] {
    scratch
      .run(
        r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES ($1, 'A room')"#,
        vec![chat_id.into()],
      )
      .await;
    scratch
      .run(
        r#"INSERT INTO "room_supervisors" ("chat_id", "supervisor") VALUES ($1, $2)"#,
        vec![chat_id.into(), AGENT.into()],
      )
      .await;
  }

  providers::choose(scratch.db(), CHOSEN, &[Road::Record.key().to_owned()]).await.unwrap();
  scratch
    .run(
      r#"INSERT INTO "provider_rooms" ("provider", "room_id", "chat_id", "title") VALUES ($1, 'a-room', $2, 'A room')"#,
      vec![Road::Record.key().into(), NUMBERED.into()],
    )
    .await;

  let port = routes(standing.receiver.clone(), TOKEN);

  (scratch, data, standing, road, port)
}

async fn said(port: &Router, chat_id: i64, text: &str) -> (StatusCode, Value) {
  let request = Request::post("/owner/messages")
    .header("authorization", format!("Bearer {TOKEN}"))
    .header("content-type", "application/json")
    .body(Body::from(json!({ "chatId": chat_id, "text": text }).to_string()))
    .unwrap();
  let answer = port.clone().oneshot(request).await.unwrap();
  let status = answer.status();
  let written = to_bytes(answer.into_body(), usize::MAX).await.unwrap();

  (status, serde_json::from_slice(&written).unwrap_or(Value::Null))
}

// Every request the road that is down was offered, of a room or of itself.
fn offered(road: &StandInChat) -> usize {
  road.refused.load(Ordering::SeqCst)
}

#[tokio::test]
async fn the_owners_words_in_a_room_on_the_record_alone_wake_its_answerer_and_ask_no_road_that_is_down() {
  let (_scratch, _data, standing, road, port) = stood().await;

  for chat_id in [CHOSEN, NUMBERED] {
    let (status, answer) = said(&port, chat_id, "What day is it today?").await;

    assert_eq!(status, StatusCode::OK, "{answer}");
  }

  let woken: Vec<(String, i64)> = standing
    .taken
    .handed()
    .iter()
    .map(|handed| (handed.agent.clone(), handed.chat_id))
    .collect();

  assert_eq!(woken, [(AGENT.to_owned(), CHOSEN), (AGENT.to_owned(), NUMBERED)]);
  assert_eq!(
    road.rooms_asked.load(Ordering::SeqCst),
    0,
    "no road is asked what it says of either room"
  );
  assert_eq!(offered(&road), 0, "the road that is down is offered nothing for them");

  let roads = standing.receiver.roads();

  for chat_id in [CHOSEN, NUMBERED] {
    assert!(roads.record_alone(chat_id).await);
    assert_eq!(roads.own(chat_id).await, Some(Road::Record));
    assert_eq!(roads.room_road(chat_id).await, None);
    assert!(!roads.room_facts(chat_id).await.owner_room);
    assert!(!roads.reads_only("road-a", chat_id).await);
  }

  assert_eq!(offered(&road), 0, "what each room is comes from its record");
}

#[tokio::test]
async fn a_room_a_road_may_hold_still_asks_that_road() {
  let (_scratch, _data, standing, road, port) = stood().await;

  assert!(!standing.receiver.roads().record_alone(TITLED).await);

  let (status, answer) = said(&port, TITLED, "What day is it today?").await;

  assert_eq!(status, StatusCode::OK, "{answer}");
  assert!(road.rooms_asked.load(Ordering::SeqCst) > 0, "the road is asked of the room it may hold");
}

//! Every chat service's profile pictures reach the console through the receiver alone (`docs/pictures.md`,
//! 「Profile pictures」): a room's, a person's and the owner's own account's picture taken from the road whose word
//! names it, a picture a road's own list leaves out taken all the same, and a picture let go only where its road
//! says it keeps none.

use std::io::Cursor;
use std::sync::Arc;

use axum::http::{StatusCode, header};
use bmo_chat_client::ChatAvatarKind::{Person, Room};
use bmo_receiver::receiver::avatars::walk;
use image::{ImageFormat, Rgb, RgbImage};
use serde_json::{Value, json};

use crate::integration::common::Scratch;
use crate::integration::common::queue::{Front, fetched, get, posted_to, stood_on};
use crate::integration::common::units::{OWNER, StandInChat};

use super::message_files::{bytes_of, header_of};

const ROOM_A: i64 = -9_000_000_000_931;
const ROOM_B: i64 = -9_000_000_000_932;
const ROOM_C: i64 = -9_000_000_000_933;
const PERSON_A: i64 = 9_000_000_000_941;
const PERSON_UNPICTURED: i64 = 9_000_000_000_942;

const RED: [u8; 3] = [220, 30, 30];
const GREEN: [u8; 3] = [30, 200, 30];
const BLUE: [u8; 3] = [30, 30, 220];
const YELLOW: [u8; 3] = [220, 220, 30];

// A small picture of one colour, as a chat service serves one.
fn png(colour: [u8; 3]) -> Vec<u8> {
  let mut out = Cursor::new(vec![]);

  RgbImage::from_pixel(4, 4, Rgb(colour)).write_to(&mut out, ImageFormat::Png).unwrap();
  out.into_inner()
}

// The receiver's answer for one picture: a JPEG of the colour the road served.
async fn served(front: &Front, path: &str, colour: [u8; 3]) {
  let answer = fetched(front, path, &[]).await;

  assert_eq!(answer.status(), StatusCode::OK, "{path}");
  assert_eq!(header_of(&answer, header::CONTENT_TYPE).as_deref(), Some("image/jpeg"), "{path}");

  let bytes = bytes_of(answer).await;
  let pixel = image::load_from_memory_with_format(&bytes, ImageFormat::Jpeg)
    .unwrap()
    .to_rgb8()
    .get_pixel(1, 1)
    .0;

  assert!(
    pixel.iter().zip(colour).all(|(got, want)| got.abs_diff(want) <= 16),
    "{path}: {pixel:?} for {colour:?}"
  );
}

// A road's word that the pictures of these rooms and people changed, as the base library hands it over.
async fn told(front: &Front, via: &str, rooms: &[i64], people: &[i64]) {
  let body = json!({ "via": via, "roomIds": rooms, "personIds": people });
  let (status, answer) = posted_to(front.handover_routes(), "/chat/avatar-changes", body).await;

  assert_eq!(status, StatusCode::CREATED, "{answer}");
}

// The subjects the receiver lists, each kind and number with when its picture was taken.
async fn listed(front: &Front) -> Vec<(String, i64, i64)> {
  let (status, page) = get(front, "/avatars").await;

  assert_eq!(status, StatusCode::OK, "{page}");

  let mut items: Vec<(String, i64, i64)> = page["items"]
    .as_array()
    .unwrap()
    .iter()
    .map(|item: &Value| {
      (
        item["kind"].as_str().unwrap().to_owned(),
        item["id"].as_i64().unwrap(),
        item["takenAt"].as_i64().unwrap(),
      )
    })
    .collect();

  items.sort();
  items
}

// A message kept in a room as one that came on a road.
async fn came(scratch: &Scratch, room: i64, message: i64, sender: i64, via: &str) {
  scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via")
         VALUES ($1, $2, $3, 'tester', 1000, 'said', $4)"#,
      vec![room.into(), message.into(), sender.into(), via.into()],
    )
    .await;
}

#[tokio::test]
async fn every_chat_services_room_person_and_owner_pictures_are_served_through_the_receiver() {
  let scratch = Scratch::new().await;
  // Neither road lists the pictures it serves, so only the word that they changed brings them.
  let road_a = StandInChat::new("road-a").picturing(Room, ROOM_A, 1_000, png(RED), false).picturing(
    Person,
    PERSON_A,
    1_000,
    png(GREEN),
    false,
  );
  let road_b = StandInChat::new("road-b").picturing(Room, ROOM_B, 1_000, png(BLUE), false).picturing(
    Person,
    OWNER,
    1_000,
    png(YELLOW),
    false,
  );
  let front = stood_on(&scratch, OWNER, vec![Arc::new(road_a), Arc::new(road_b)]).await;

  told(&front, "road-a", &[ROOM_A], &[PERSON_A]).await;
  told(&front, "road-b", &[ROOM_B], &[OWNER]).await;

  let subjects: Vec<(String, i64)> =
    listed(&front).await.into_iter().map(|(kind, id, _)| (kind, id)).collect();

  assert_eq!(
    subjects,
    vec![
      ("person".to_owned(), OWNER),
      ("person".to_owned(), PERSON_A),
      ("room".to_owned(), ROOM_B),
      ("room".to_owned(), ROOM_A),
    ]
  );
  served(&front, &format!("/rooms/{ROOM_A}/avatar"), RED).await;
  served(&front, &format!("/people/{PERSON_A}/avatar"), GREEN).await;
  served(&front, &format!("/rooms/{ROOM_B}/avatar"), BLUE).await;
  served(&front, &format!("/people/{OWNER}/avatar"), YELLOW).await;
}

#[tokio::test]
async fn a_picture_a_roads_own_list_leaves_out_reaches_the_console_from_the_walk_as_the_service_starts() {
  let scratch = Scratch::new().await;
  let road = StandInChat::new("road-a")
    .picturing(Room, ROOM_A, 5_000, png(RED), true)
    .picturing(Room, ROOM_C, 6_000, png(GREEN), false)
    .picturing(Person, PERSON_A, 7_000, png(BLUE), false);
  let front = stood_on(&scratch, OWNER, vec![Arc::new(road)]).await;

  came(&scratch, ROOM_C, 1, PERSON_A, "road-a").await;
  came(&scratch, ROOM_C, 2, PERSON_UNPICTURED, "road-a").await;

  assert_eq!(walk(&front.receiver(), "road-a").await, 3);

  let kept = listed(&front).await;

  assert!(kept.contains(&("room".to_owned(), ROOM_A, 5_000)), "{kept:?}");
  assert!(kept.iter().any(|(kind, id, _)| kind == "room" && *id == ROOM_C), "{kept:?}");
  assert!(kept.iter().any(|(kind, id, _)| kind == "person" && *id == PERSON_A), "{kept:?}");
  assert_eq!(kept.len(), 3, "{kept:?}");
  served(&front, &format!("/rooms/{ROOM_C}/avatar"), GREEN).await;
  served(&front, &format!("/people/{PERSON_A}/avatar"), BLUE).await;

  // Walked again, nothing on the list is newer than what is kept and the one subject with no picture still has none.
  assert_eq!(walk(&front.receiver(), "road-a").await, 0);
}

#[tokio::test]
async fn a_picture_goes_where_its_road_keeps_none_and_stands_where_a_road_does_not_answer() {
  let scratch = Scratch::new().await;
  let road_a = Arc::new(StandInChat::new("road-a").picturing(Room, ROOM_A, 1_000, png(RED), false));
  let road_b = Arc::new(StandInChat::new("road-b").down());
  let front = stood_on(&scratch, OWNER, vec![road_a.clone(), road_b]).await;
  let path = format!("/rooms/{ROOM_A}/avatar");

  told(&front, "road-a", &[ROOM_A], &[]).await;
  served(&front, &path, RED).await;

  told(&front, "road-b", &[ROOM_A], &[]).await;
  served(&front, &path, RED).await;

  road_a.unpicturing(Room, ROOM_A);
  told(&front, "road-a", &[ROOM_A], &[]).await;

  let (status, refused) = get(&front, &path).await;

  assert_eq!(status, StatusCode::NOT_FOUND, "{refused}");
  assert!(!front.data.path().join(format!("avatars/room/{ROOM_A}.jpg")).exists());
  assert!(listed(&front).await.is_empty());
}

//! A room's pictures as the console's viewer walks them (`docs/boundary-api.md`, 「A room's pictures」): the
//! messages that stored a picture, oldest first, asked for by a picture the reader holds — that picture alone,
//! the stretch before or after it, or the newest — with whether more stand beyond the stretch, how many stand
//! before it and how many the room holds, and each picture's file where it still stands.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::common::Scratch;
use crate::common::queue::{Front, get, stood};
use crate::common::units::OWNER;

const ROOM: i64 = -711;

// Seven messages, sent a second apart; five of them stored a picture, and the last one's file is gone.
async fn bench() -> (Scratch, Front) {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;
  let folder = front.data.path().join("images").join(ROOM.to_string());

  std::fs::create_dir_all(&folder).unwrap();

  for message in 1..=7_i64 {
    let pictured = [2, 3, 5, 6, 7].contains(&message);
    let stored = pictured.then(|| folder.join(format!("{message}.jpg")).to_string_lossy().into_owned());

    if pictured && message != 7 {
      std::fs::write(stored.as_ref().unwrap(), vec![0_u8; usize::try_from(message).unwrap() * 10]).unwrap();
    }

    scratch
      .run(
        r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text",
             "image_path", "album") VALUES ($1, $2, $3, 'someone', $4, $5, $6, $7)"#,
        vec![
          ROOM.into(),
          message.into(),
          OWNER.into(),
          (1_760_000_000 + message).into(),
          format!("line {message}").into(),
          stored.into(),
          (message >= 5).then_some(5_i64).into(),
        ],
      )
      .await;
  }

  (scratch, front)
}

async fn walked(front: &Front, query: &str) -> Value {
  let (status, page) = get(front, &format!("/rooms/{ROOM}/pictures{query}")).await;

  assert_eq!(status, StatusCode::OK, "{query}: {page}");
  page
}

// The messages a page shows, then whether more stand, how many before and how many in all.
fn shape(page: &Value) -> (Vec<i64>, bool, i64, i64) {
  (
    page["items"]
      .as_array()
      .unwrap()
      .iter()
      .map(|picture| picture["id"].as_i64().unwrap())
      .collect(),
    page["more"].as_bool().unwrap(),
    page["before"].as_i64().unwrap(),
    page["total"].as_i64().unwrap(),
  )
}

#[tokio::test]
async fn the_newest_pictures_are_walked_with_their_place_among_the_rooms() {
  let (_scratch, front) = bench().await;

  assert_eq!(shape(&walked(&front, "").await), (vec![2, 3, 5, 6, 7], false, 0, 5));
  assert_eq!(shape(&walked(&front, "?limit=2").await), (vec![6, 7], true, 3, 5));

  let page = walked(&front, "?limit=2").await;

  assert_eq!(
    page["items"][0],
    json!({
      "id": 6,
      "album": 5,
      "senderName": "someone",
      "sentAt": 1_760_000_006_000_i64,
      "text": "line 6",
      "file": { "name": "6.jpg", "bytes": 60, "type": "image/jpeg" },
    }),
  );
  assert_eq!(page["items"][1]["file"], Value::Null, "a picture whose file is gone stands without one");
}

#[tokio::test]
async fn a_picture_the_reader_holds_is_answered_alone() {
  let (_scratch, front) = bench().await;

  assert_eq!(shape(&walked(&front, "?at=5").await), (vec![5], false, 2, 5));
  assert_eq!(
    shape(&walked(&front, "?at=4").await),
    (vec![], false, 0, 5),
    "a message with no picture"
  );
}

#[tokio::test]
async fn the_stretch_before_a_picture_ends_just_before_it_and_stops_at_the_first() {
  let (_scratch, front) = bench().await;

  assert_eq!(shape(&walked(&front, "?before=6&limit=2").await), (vec![3, 5], true, 1, 5));
  assert_eq!(shape(&walked(&front, "?before=5&limit=2").await), (vec![2, 3], false, 0, 5));
  assert_eq!(shape(&walked(&front, "?before=2&limit=2").await), (vec![], false, 0, 5));
}

#[tokio::test]
async fn the_stretch_after_a_picture_begins_just_past_it_and_stops_at_the_last() {
  let (_scratch, front) = bench().await;

  assert_eq!(shape(&walked(&front, "?after=3&limit=2").await), (vec![5, 6], true, 2, 5));
  assert_eq!(shape(&walked(&front, "?after=6&limit=2").await), (vec![7], false, 4, 5));
  assert_eq!(shape(&walked(&front, "?after=7&limit=2").await), (vec![], false, 0, 5));
}

#[tokio::test]
async fn a_room_with_no_pictures_answers_none() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  scratch.message(ROOM, 1, OWNER).await;

  assert_eq!(shape(&walked(&front, "").await), (vec![], false, 0, 0));
}

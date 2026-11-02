//! The profile pictures the archive keeps, served to the console (`docs/pictures.md`, 「Profile pictures」): a page
//! of what is kept, oldest taken first, and each room's or person's picture as its bytes, served from the data
//! root's avatars folder alone.

use axum::http::{StatusCode, header};
use serde_json::json;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{Front, fetched, get, stood};
use crate::integration::common::units::OWNER;

use super::message_files::{bytes_of, header_of, written};

const ROOM: i64 = -9_000_000_000_902;
const PERSON: i64 = 9_000_000_000_911;

// A room and a person with a picture, a person whose file is gone, and a path leading out of the folder.
async fn bench() -> (Scratch, Front) {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;
  let root = front.data.path().to_path_buf();
  let room = written(&root, &format!("avatars/room/{ROOM}.jpg"), b"\xFF\xD8\xFFroom");
  let person = written(&root, &format!("avatars/person/{PERSON}.jpg"), b"\xFF\xD8\xFFperson");
  let outside = written(&root, "images/elsewhere.jpg", b"\xFF\xD8\xFFoutside");
  let insert = r#"INSERT INTO "avatars" ("kind", "subject_id", "photo_id", "path", "updated_at") VALUES ($1, $2, $3, $4, $5)"#;

  scratch
    .run(
      insert,
      vec!["room".into(), ROOM.into(), Option::<String>::None.into(), room.into(), 1_000_i64.into()],
    )
    .await;
  scratch
    .run(
      insert,
      vec!["person".into(), PERSON.into(), "p1".into(), person.into(), 2_000_i64.into()],
    )
    .await;
  scratch
    .run(
      insert,
      vec![
        "person".into(),
        7_i64.into(),
        Option::<String>::None.into(),
        "/nowhere/7.jpg".into(),
        3_000_i64.into(),
      ],
    )
    .await;
  scratch
    .run(
      insert,
      vec!["person".into(), 8_i64.into(), "p8".into(), outside.into(), 4_000_i64.into()],
    )
    .await;
  scratch
    .run(
      insert,
      vec![
        "person".into(),
        9_i64.into(),
        "p9".into(),
        Option::<String>::None.into(),
        5_000_i64.into(),
      ],
    )
    .await;

  (scratch, front)
}

#[tokio::test]
async fn the_kept_pictures_are_listed_oldest_first_and_walked_by_when_they_were_taken() {
  let (_scratch, front) = bench().await;

  let (status, page) = get(&front, "/avatars").await;

  assert_eq!(status, StatusCode::OK, "{page}");
  assert_eq!(
    page,
    json!({
      "items": [
        { "kind": "room", "id": ROOM, "takenAt": 1_000 },
        { "kind": "person", "id": PERSON, "takenAt": 2_000 },
        { "kind": "person", "id": 7, "takenAt": 3_000 },
        { "kind": "person", "id": 8, "takenAt": 4_000 }
      ],
      "more": false
    })
  );

  let (status, page) = get(&front, "/avatars?after=1000&limit=2").await;

  assert_eq!(status, StatusCode::OK, "{page}");
  assert_eq!(page["items"].as_array().map(Vec::len), Some(2));
  assert_eq!(page["items"][0]["id"], PERSON);
  assert_eq!(page["more"], true);
}

#[tokio::test]
async fn a_rooms_and_a_persons_picture_are_served_from_the_avatars_folder_alone() {
  let (_scratch, front) = bench().await;

  let answer = fetched(&front, &format!("/rooms/{ROOM}/avatar"), &[]).await;

  assert_eq!(answer.status(), StatusCode::OK);
  assert_eq!(header_of(&answer, header::CONTENT_TYPE).as_deref(), Some("image/jpeg"));
  assert_eq!(bytes_of(answer).await, b"\xFF\xD8\xFFroom");

  let answer = fetched(&front, &format!("/people/{PERSON}/avatar"), &[]).await;

  assert_eq!(answer.status(), StatusCode::OK);
  assert_eq!(bytes_of(answer).await, b"\xFF\xD8\xFFperson");

  // A file that is gone, a path outside the folder, no path at all, and a number the table does not hold.
  for path in [
    "/people/7/avatar",
    "/people/8/avatar",
    "/people/9/avatar",
    "/people/99/avatar",
    "/rooms/99/avatar",
  ] {
    let (status, refused) = get(&front, path).await;

    assert_eq!(status, StatusCode::NOT_FOUND, "{path}: {refused}");
    assert_eq!(refused["code"], "not-found", "{path}");
  }
}

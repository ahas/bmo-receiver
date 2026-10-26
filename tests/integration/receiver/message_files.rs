//! A message's kept files and picture as the console reads them (`docs/boundary-api.md`, 「A message's files and
//! picture」): listed in a page's order by place, served whole or by the range asked, saved under the file's own
//! name when downloaded, and refused where nothing is held, the place lies past the files, or the stored path
//! resolves outside the room's own folder of its store.

use std::path::Path;

use axum::body::to_bytes;
use axum::http::{StatusCode, header};
use axum::response::Response;
use serde_json::json;

use crate::common::Scratch;
use crate::common::queue::{Front, fetched, get, stood};
use crate::common::units::OWNER;

const ROOM: i64 = -611;
const OTHER: i64 = -612;
const MESSAGE: i64 = 3;

// A file written under the data root, its folders made first; answers its path.
pub(super) fn written(root: &Path, at: &str, bytes: &[u8]) -> String {
  let path = root.join(at);

  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(&path, bytes).unwrap();
  path.to_string_lossy().into_owned()
}

async fn kept(scratch: &Scratch, place: i64, kind: &str, source: &str, stored: Option<&str>) {
  scratch
    .run(
      r#"INSERT INTO "collected_items" ("chat_id", "message_id", "sender_name", "kind", "source", "stored_path",
           "collected_at") VALUES ($1, $2, 'someone', $3, $4, $5, $6)"#,
      vec![
        ROOM.into(),
        MESSAGE.into(),
        kind.to_owned().into(),
        source.to_owned().into(),
        stored.map(str::to_owned).into(),
        place.into(),
      ],
    )
    .await;
}

// A message carrying six files: a voice note and a document kept, one never kept, one kept for another room, one
// whose link leads out of the store, and one gone from the disk.
async fn bench() -> (Scratch, Front) {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;
  let root = front.data.path().to_path_buf();

  scratch.message(ROOM, MESSAGE, OWNER).await;

  let voice = written(&root, &format!("collected/{ROOM}/{MESSAGE}-voice.ogg"), b"0123456789");
  let document = written(&root, &format!("collected/{ROOM}/{MESSAGE}-report final.pdf"), b"%PDF");
  let elsewhere = written(&root, &format!("collected/{OTHER}/{MESSAGE}-theirs.txt"), b"another room's");
  let outside = written(&root, "secrets/outside.txt", b"outside every store");
  let linked = root.join(format!("collected/{ROOM}/{MESSAGE}-linked.txt"));

  std::os::unix::fs::symlink(&outside, &linked).unwrap();

  kept(&scratch, 1, "voice", "voice.ogg", Some(&voice)).await;
  kept(&scratch, 2, "document", "report final.pdf", Some(&document)).await;
  kept(&scratch, 3, "document", "big.zip", None).await;
  kept(&scratch, 4, "document", "theirs.txt", Some(&elsewhere)).await;
  kept(&scratch, 5, "document", "linked.txt", Some(&linked.to_string_lossy())).await;
  kept(
    &scratch,
    6,
    "video",
    "gone.mp4",
    Some(&format!("{}/collected/{ROOM}/{MESSAGE}-gone.mp4", root.display())),
  )
  .await;

  (scratch, front)
}

pub(super) async fn bytes_of(answer: Response) -> Vec<u8> {
  to_bytes(answer.into_body(), usize::MAX).await.unwrap().to_vec()
}

pub(super) fn header_of(answer: &Response, name: header::HeaderName) -> Option<String> {
  answer.headers().get(name).map(|value| value.to_str().unwrap().to_owned())
}

#[tokio::test]
async fn a_messages_files_are_listed_in_a_pages_order_each_by_its_place() {
  let (_scratch, front) = bench().await;
  let (status, listed) = get(&front, &format!("/rooms/{ROOM}/messages/{MESSAGE}/files")).await;

  assert_eq!(status, StatusCode::OK, "{listed}");
  assert_eq!(
    listed["items"],
    json!([
      { "place": 0, "kind": "voice", "name": "voice.ogg", "type": "audio/ogg", "size": 10, "kept": true },
      { "place": 1, "kind": "document", "name": "report final.pdf", "type": "application/pdf", "size": 4, "kept": true },
      { "place": 2, "kind": "document", "name": "big.zip", "type": "application/zip", "size": null, "kept": false },
      { "place": 3, "kind": "document", "name": "theirs.txt", "type": "text/plain", "size": null, "kept": false },
      { "place": 4, "kind": "document", "name": "linked.txt", "type": "text/plain", "size": null, "kept": false },
      { "place": 5, "kind": "video", "name": "gone.mp4", "type": "video/mp4", "size": null, "kept": false },
    ]),
  );

  let (status, refused) = get(&front, &format!("/rooms/{ROOM}/messages/99/files")).await;

  assert_eq!((status, refused["code"].as_str()), (StatusCode::NOT_FOUND, Some("not-found")));
}

#[tokio::test]
async fn a_file_is_served_whole_by_its_place_and_saved_under_its_own_name_when_downloaded() {
  let (_scratch, front) = bench().await;
  let answer = fetched(&front, &format!("/rooms/{ROOM}/messages/{MESSAGE}/files/0"), &[]).await;

  assert_eq!(answer.status(), StatusCode::OK);
  assert_eq!(header_of(&answer, header::CONTENT_TYPE).as_deref(), Some("audio/ogg"));
  assert_eq!(header_of(&answer, header::CONTENT_LENGTH).as_deref(), Some("10"));
  assert_eq!(header_of(&answer, header::ACCEPT_RANGES).as_deref(), Some("bytes"));
  assert_eq!(header_of(&answer, header::CONTENT_DISPOSITION), None);
  assert_eq!(bytes_of(answer).await, b"0123456789");

  let saved =
    fetched(&front, &format!("/rooms/{ROOM}/messages/{MESSAGE}/files/1?download=true"), &[]).await;

  assert_eq!(saved.status(), StatusCode::OK);
  assert_eq!(
    header_of(&saved, header::CONTENT_DISPOSITION).as_deref(),
    Some("attachment; filename*=UTF-8''report%20final.pdf"),
  );
  assert_eq!(bytes_of(saved).await, b"%PDF");
}

#[tokio::test]
async fn a_range_is_answered_partial_and_one_outside_the_file_is_refused() {
  let (_scratch, front) = bench().await;
  let path = format!("/rooms/{ROOM}/messages/{MESSAGE}/files/0");
  let part = fetched(&front, &path, &[("range", "bytes=2-5")]).await;

  assert_eq!(part.status(), StatusCode::PARTIAL_CONTENT);
  assert_eq!(header_of(&part, header::CONTENT_RANGE).as_deref(), Some("bytes 2-5/10"));
  assert_eq!(bytes_of(part).await, b"2345");

  let past = fetched(&front, &path, &[("range", "bytes=40-50")]).await;

  assert_eq!(past.status(), StatusCode::RANGE_NOT_SATISFIABLE);
}

#[tokio::test]
async fn nothing_held_a_place_past_the_files_and_a_path_outside_the_rooms_folder_are_refused() {
  let (_scratch, front) = bench().await;

  // Not kept, kept for another room, a link leading out of the store, gone from the disk, past the files, and a
  // message the record does not hold.
  for path in [
    format!("/rooms/{ROOM}/messages/{MESSAGE}/files/2"),
    format!("/rooms/{ROOM}/messages/{MESSAGE}/files/3"),
    format!("/rooms/{ROOM}/messages/{MESSAGE}/files/4"),
    format!("/rooms/{ROOM}/messages/{MESSAGE}/files/5"),
    format!("/rooms/{ROOM}/messages/{MESSAGE}/files/6"),
    format!("/rooms/{ROOM}/messages/99/files/0"),
  ] {
    let (status, refused) = get(&front, &path).await;

    assert_eq!(status, StatusCode::NOT_FOUND, "{path}: {refused}");
    assert_eq!(refused["code"], "not-found", "{path}");
  }
}

#[tokio::test]
async fn a_messages_picture_is_served_from_the_rooms_own_folder_alone() {
  let (scratch, front) = bench().await;
  let root = front.data.path().to_path_buf();
  let picture = written(&root, &format!("images/{ROOM}/{MESSAGE}.jpg"), b"\xFF\xD8\xFFpicture");
  let theirs = written(&root, &format!("images/{OTHER}/4.jpg"), b"\xFF\xD8\xFFtheirs");
  let set = r#"UPDATE "messages" SET "image_path" = $1 WHERE "chat_id" = $2 AND "message_id" = $3"#;

  scratch.run(set, vec![picture.into(), ROOM.into(), MESSAGE.into()]).await;
  scratch.message(ROOM, 4, OWNER).await;
  scratch.run(set, vec![theirs.into(), ROOM.into(), 4_i64.into()]).await;

  let answer = fetched(&front, &format!("/rooms/{ROOM}/messages/{MESSAGE}/picture"), &[]).await;

  assert_eq!(answer.status(), StatusCode::OK);
  assert_eq!(header_of(&answer, header::CONTENT_TYPE).as_deref(), Some("image/jpeg"));
  assert_eq!(bytes_of(answer).await, b"\xFF\xD8\xFFpicture");

  scratch.message(ROOM, 5, OWNER).await;

  // A picture kept for another room, a message that stored none, and one the record does not hold.
  for message in [4, 5, 99] {
    let (status, refused) = get(&front, &format!("/rooms/{ROOM}/messages/{message}/picture")).await;

    assert_eq!(status, StatusCode::NOT_FOUND, "{message}: {refused}");
  }
}

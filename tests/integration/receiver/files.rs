//! The files a message carried, kept as they are handed over (docs/room-files.md, 「Message files」; docs/pictures.md,
//! 「Pictures received」): a picture filed under its message, brought down and laid on white where it must be; a sound
//! or a video with no name named for what it is; the same file, by its message and id, kept once and two files of one
//! name kept apart; and a file past the cap or one that failed leaving its row for a later hand-over.

use std::io::Cursor;
use std::sync::Arc;

use bmo_chat_client::{GatheredPatch, MessageFile};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::{Context, Room};
use image::{ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};
use receiver_policy::roads::ROADS;

use super::inbound;
use crate::common::Scratch;
use crate::common::units::{CAP, StandInChat, Standing, standing};

const ROOM: i64 = -120;

async fn stood() -> (Scratch, tempfile::TempDir, Standing) {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let chat = Arc::new(StandInChat::new("road-a").holding(&[ROOM]));
  let standing = standing(scratch.db(), DataRoot::new(data.path()), vec![chat]);

  (scratch, data, standing)
}

fn file(id: &str, name: &str, mime: &str, bytes: Vec<u8>) -> MessageFile {
  MessageFile {
    id: id.into(),
    name: name.into(),
    mime: mime.into(),
    bytes,
    kind: None,
  }
}

fn carrying() -> Context {
  Context {
    carries_file: true,
    ..Context::default()
  }
}

async fn handed(standing: &Standing, message_id: i64, files: &[MessageFile]) {
  standing
    .receiver
    .attachment(&inbound(ROOM, message_id, 55, "[file]"), &carrying(), Room::default(), files)
    .await
    .unwrap();
}

fn encoded(picture: image::DynamicImage, format: ImageFormat) -> Vec<u8> {
  let mut out = Cursor::new(Vec::new());

  picture.write_to(&mut out, format).unwrap();
  out.into_inner()
}

// Pixels no encoder can fold away, so the picture stands past the threshold by its size alone.
fn noise(width: u32, height: u32) -> Vec<u8> {
  let mut state: u32 = 0x2545_f491;
  let picture = RgbImage::from_fn(width, height, |_, _| {
    state ^= state << 13;
    state ^= state >> 17;
    state ^= state << 5;

    let [red, green, blue, _] = state.to_le_bytes();

    Rgb([red, green, blue])
  });

  encoded(picture.into(), ImageFormat::Png)
}

async fn image_path(scratch: &Scratch, message_id: i64) -> String {
  scratch
    .texts(&format!(
      r#"SELECT "image_path" FROM "messages" WHERE "chat_id" = {ROOM} AND "message_id" = {message_id}"#
    ))
    .await
    .remove(0)
}

#[tokio::test]
async fn a_large_picture_is_brought_inside_its_longest_side_and_filed_as_a_jpeg_under_its_message() {
  let (scratch, data, standing) = stood().await;
  let large = noise(2000, 1100);

  assert!(
    large.len() > ROADS.pictures.kept_threshold_bytes,
    "the picture stands past the threshold"
  );

  handed(&standing, 1, &[file("1", "big.png", "image/png", large)]).await;

  let path = image_path(&scratch, 1).await;
  let kept = std::fs::read(&path).unwrap();
  let read = image::load_from_memory(&kept).unwrap();

  assert_eq!(path, data.path().join(format!("images/{ROOM}/1.jpg")).to_string_lossy());
  assert_eq!(image::guess_format(&kept).unwrap(), ImageFormat::Jpeg);
  assert_eq!((read.width(), read.height()), (ROADS.pictures.kept_max_dimension, 862));
}

#[tokio::test]
async fn a_transparent_picture_is_laid_on_white_and_filed_as_a_jpeg() {
  let (scratch, _data, standing) = stood().await;
  let clear = encoded(RgbaImage::from_pixel(8, 8, Rgba([0, 0, 0, 0])).into(), ImageFormat::Png);

  handed(&standing, 2, &[file("1", "clear.png", "image/png", clear)]).await;

  let kept = std::fs::read(image_path(&scratch, 2).await).unwrap();
  let read = image::load_from_memory(&kept).unwrap().to_rgb8();

  assert_eq!(image::guess_format(&kept).unwrap(), ImageFormat::Jpeg);
  assert!(read.pixels().all(|pixel| pixel.0.iter().all(|channel| *channel > 245)), "laid on white");
}

#[tokio::test]
async fn a_small_jpeg_is_kept_as_it_came_and_the_messages_first_picture_names_its_row() {
  let (scratch, data, standing) = stood().await;
  let small = encoded(RgbImage::from_pixel(8, 8, Rgb([10, 200, 30])).into(), ImageFormat::Jpeg);
  let opaque = encoded(RgbImage::from_pixel(8, 8, Rgb([200, 10, 30])).into(), ImageFormat::Png);

  handed(
    &standing,
    3,
    &[
      file("1", "a.jpg", "image/jpeg", small.clone()),
      file("2", "b.png", "image/png", opaque.clone()),
    ],
  )
  .await;

  let folder = data.path().join(format!("images/{ROOM}"));

  assert_eq!(image_path(&scratch, 3).await, folder.join("3.jpg").to_string_lossy());
  assert_eq!(std::fs::read(folder.join("3.jpg")).unwrap(), small);
  // An opaque picture under the threshold is kept as it came too.
  assert_eq!(std::fs::read(folder.join("3-1.jpg")).unwrap(), opaque);
}

#[tokio::test]
async fn a_sound_or_a_video_with_no_name_is_named_for_what_it_is() {
  let (scratch, _data, standing) = stood().await;

  handed(
    &standing,
    4,
    &[
      file("1", "", "video/webm", b"webm".to_vec()),
      file("2", "", "video/x-unknown", b"what".to_vec()),
      file("3", "", "audio/ogg; codecs=opus", b"ogg".to_vec()),
      file("4", "clip.mov", "video/quicktime", b"mov".to_vec()),
    ],
  )
  .await;

  assert_eq!(
    scratch
      .texts(&format!(
        r#"SELECT "source" || ' ' || "kind" FROM "collected_items" WHERE "chat_id" = {ROOM} AND "message_id" = 4 ORDER BY "source""#
      ))
      .await,
    ["clip.mov video", "video.mp4 video", "video.webm video", "voice.ogg audio"]
  );
}

#[tokio::test]
async fn the_same_file_handed_over_twice_is_kept_once() {
  let (scratch, _data, standing) = stood().await;

  handed(&standing, 5, &[file("1", "notes.txt", "text/plain", b"first".to_vec())]).await;

  let again = GatheredPatch::kept(vec![file("1", "notes.txt", "text/plain", b"second".to_vec())]);

  standing.receiver.gathered(ROOM, 5, "road-a", &again).await.unwrap();

  let rows = scratch
    .texts(&format!(
      r#"SELECT "stored_path" FROM "collected_items" WHERE "chat_id" = {ROOM} AND "message_id" = 5"#
    ))
    .await;

  assert_eq!(rows.len(), 1);
  assert_eq!(std::fs::read(&rows[0]).unwrap(), b"first");
}

#[tokio::test]
async fn two_files_of_one_name_in_a_message_are_two_files_filed_apart() {
  let (scratch, data, standing) = stood().await;

  handed(&standing, 7, &[file("a", "notes.txt", "text/plain", b"first".to_vec())]).await;

  let other = GatheredPatch::kept(vec![file("b", "notes.txt", "text/plain", b"second".to_vec())]);

  standing.receiver.gathered(ROOM, 7, "road-a", &other).await.unwrap();

  let folder = data.path().join(format!("collected/{ROOM}"));

  assert_eq!(
    scratch
      .texts(&format!(
        r#"SELECT "file_unique_id" || ' ' || "stored_path" FROM "collected_items" WHERE "chat_id" = {ROOM} AND "message_id" = 7 ORDER BY "file_unique_id""#
      ))
      .await,
    [
      format!("a {}", folder.join("7-notes.txt").to_string_lossy()),
      format!("b {}", folder.join("7-2-notes.txt").to_string_lossy()),
    ]
  );
  assert_eq!(std::fs::read(folder.join("7-notes.txt")).unwrap(), b"first");
  assert_eq!(std::fs::read(folder.join("7-2-notes.txt")).unwrap(), b"second");
}

#[tokio::test]
async fn a_row_that_names_no_id_is_taken_by_the_first_file_of_its_name() {
  let (scratch, data, standing) = stood().await;

  scratch
    .run(
      r#"INSERT INTO "collected_items" ("chat_id", "message_id", "sender_name", "kind", "source", "collected_at")
         VALUES ($1, 8, 'tester', 'document', 'notes.txt', 0)"#,
      vec![ROOM.into()],
    )
    .await;
  handed(&standing, 8, &[file("a", "notes.txt", "text/plain", b"kept".to_vec())]).await;

  let stored = data.path().join(format!("collected/{ROOM}/8-notes.txt"));

  assert_eq!(
    scratch
      .texts(&format!(
        r#"SELECT "file_unique_id" || ' ' || "stored_path" FROM "collected_items" WHERE "chat_id" = {ROOM} AND "message_id" = 8"#
      ))
      .await,
    [format!("a {}", stored.to_string_lossy())]
  );
}

#[tokio::test]
async fn a_file_past_the_cap_keeps_its_row_without_the_file_until_a_later_hand_over_keeps_it() {
  let (scratch, data, standing) = stood().await;
  let past = vec![b'x'; usize::try_from(CAP).unwrap() + 1];

  handed(&standing, 6, &[file("1", "big.bin", "application/octet-stream", past)]).await;

  let sql = format!(
    r#"SELECT COALESCE("stored_path", 'none') || ' | ' || COALESCE("summary", 'none') FROM "collected_items" WHERE "chat_id" = {ROOM} AND "message_id" = 6"#
  );
  let row = async || scratch.texts(&sql).await.remove(0);

  assert_eq!(row().await, format!("none | past the attachment size cap of {CAP} bytes"));
  assert!(!data.path().join(format!("collected/{ROOM}/6-big.bin")).exists());

  // The same file handed over again within the cap fills the row that was left.
  let smaller =
    GatheredPatch::kept(vec![file("1", "big.bin", "application/octet-stream", b"fits".to_vec())]);

  standing.receiver.gathered(ROOM, 6, "road-a", &smaller).await.unwrap();

  let stored = data.path().join(format!("collected/{ROOM}/6-big.bin"));

  assert_eq!(row().await, format!("{} | none", stored.to_string_lossy()));
  assert_eq!(std::fs::read(&stored).unwrap(), b"fits");
}

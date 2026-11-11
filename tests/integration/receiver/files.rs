//! The files a message carried, kept as they are handed over (docs/room-files.md, 「Message files」; docs/pictures.md,
//! 「Pictures received」): a picture filed under its message, brought down and laid on white where it must be; a sound
//! or a video with no name named for what it is; the same file, by its message and id, kept once and two files of one
//! name kept apart; and a file past the cap or one that failed leaving its row for a later hand-over.

use std::collections::HashSet;
use std::io::Cursor;
use std::sync::Arc;

use bmo_chat_client::{ChatFileKind, GatheredPatch, MessageFile};
use bmo_receiver::chat::shape::Reading;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::{Context, Room, rows};
use image::{ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::roads::ROADS;

use super::inbound;
use crate::integration::common::Scratch;
use crate::integration::common::units::{CAP, StandInChat, Standing, standing};

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
  handed_text(standing, message_id, "[file]", files).await;
}

async fn handed_text(standing: &Standing, message_id: i64, text: &str, files: &[MessageFile]) {
  standing
    .receiver
    .attachment(&inbound(ROOM, message_id, 55, text), &carrying(), Room::default(), files)
    .await
    .unwrap();
}

fn voice() -> MessageFile {
  MessageFile {
    id: "1".into(),
    name: "voice.oga".into(),
    mime: "audio/ogg".into(),
    bytes: vec![0x4f, 0x67, 0x67, 0x53, 0x00],
    kind: Some(ChatFileKind::VoiceNote),
  }
}

async fn voice_row(scratch: &Scratch, message_id: i64) -> String {
  scratch
    .texts(&format!(
      r#"SELECT COALESCE("transcript", '') || '|' || COALESCE("voice_seconds"::text, '') || '|' || COALESCE("voice_state", '')
         FROM "messages" WHERE "chat_id" = {ROOM} AND "message_id" = {message_id}"#
    ))
    .await
    .into_iter()
    .next()
    .unwrap_or_default()
}

async fn voice_line(standing: &Standing, message_id: i64) -> String {
  let message = rows::message(standing.receiver.db(), ROOM, message_id, "road-a")
    .await
    .unwrap()
    .unwrap();
  let shown = HashSet::new();

  Reading {
    receiver: &standing.receiver,
    owner: 55,
    fetch_on_demand: false,
    shown: &shown,
  }
  .lines(&[message])
  .await
  .unwrap()
  .remove(0)
}

async fn line(standing: &Standing, message_id: i64) -> String {
  let message = rows::message(standing.receiver.db(), ROOM, message_id, "road-a")
    .await
    .unwrap()
    .unwrap();
  let shown = HashSet::new();

  Reading {
    receiver: &standing.receiver,
    owner: 55,
    fetch_on_demand: false,
    shown: &shown,
  }
  .lines(&[message])
  .await
  .unwrap()
  .remove(0)
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
async fn a_sticker_and_an_animation_keep_their_own_kind_and_a_sticker_picture_stays_a_picture() {
  let (scratch, _data, standing) = stood().await;
  let kinded = |id: &str, name: &str, mime: &str, kind: ChatFileKind| MessageFile {
    kind: Some(kind),
    ..file(id, name, mime, b"bytes".to_vec())
  };

  handed(
    &standing,
    40,
    &[
      kinded("1", "sticker-1.tgs", "application/x-tgsticker", ChatFileKind::Sticker),
      kinded("2", "sticker-2.webm", "video/webm", ChatFileKind::Sticker),
      kinded("3", "", "video/mp4", ChatFileKind::Animation),
      kinded("4", "clip.mp4", "video/mp4", ChatFileKind::Video),
    ],
  )
  .await;

  assert_eq!(
    scratch
      .texts(&format!(
        r#"SELECT "source" || ' ' || "kind" FROM "collected_items" WHERE "chat_id" = {ROOM} AND "message_id" = 40 ORDER BY "source""#
      ))
      .await,
    ["clip.mp4 video", "sticker-1.tgs sticker", "sticker-2.webm sticker", "video.mp4 animation"]
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

#[tokio::test]
async fn a_voice_note_is_heard_before_its_turn_and_kept_with_its_transcript() {
  let (scratch, _data, standing) = stood().await;

  standing.hearing.saying("Good morning, the backup finished.", Some(2.5));
  handed_text(&standing, 1, &ROADS.marks.voice, &[voice()]).await;

  assert_eq!(voice_row(&scratch, 1).await, "Good morning, the backup finished.|2.5|heard");
  assert_eq!(standing.hearing.asked().len(), 1);
  assert!(standing.hearing.asked()[0].to_string_lossy().ends_with("voice.oga"));

  let line = voice_line(&standing, 1).await;

  assert!(line.contains("heard as: Good morning, the backup finished."), "{line}");

  let page = bmo_receiver::receiver::reads::latest(scratch.db(), ROOM, 1).await.unwrap();
  let shown = serde_json::to_value(&page[0]).unwrap();

  assert_eq!(shown["transcript"], "Good morning, the backup finished.");
  assert_eq!(shown["voiceSeconds"], 2.5);
  assert_eq!(shown["voiceState"], "heard");
}

#[tokio::test]
async fn a_voice_note_that_cannot_be_heard_is_one_line_saying_so() {
  let (scratch, _data, standing) = stood().await;

  standing.hearing.failing("the node is down");
  handed_text(&standing, 1, &ROADS.marks.voice, &[voice()]).await;

  assert_eq!(voice_row(&scratch, 1).await, "||failed");

  let line = voice_line(&standing, 1).await;

  assert!(line.contains(&CHAT_ROUND.shape.voice_failed), "{line}");
}

#[tokio::test]
async fn a_picture_with_no_caption_carries_its_note_and_path() {
  let (scratch, _data, standing) = stood().await;
  let picture = encoded(RgbImage::from_pixel(8, 8, Rgb([10, 200, 30])).into(), ImageFormat::Jpeg);

  scratch
    .run(r#"INSERT INTO "photo_chats" ("chat_id", "look") VALUES ($1, 1)"#, vec![ROOM.into()])
    .await;
  handed_text(&standing, 1, "", &[file("1", "photo.jpg", "image/jpeg", picture)]).await;

  let path = image_path(&scratch, 1).await;
  let line = line(&standing, 1).await;

  assert!(line.contains(&path), "{line}");
  assert!(
    line.contains(&receiver_policy::fill_marks(
      &CHAT_ROUND.shape.image_attached,
      &[("path", &path), ("look", &CHAT_ROUND.shape.image_look)]
    )),
    "the note stands with the path: {line}"
  );
}

#[tokio::test]
async fn a_picture_that_could_not_be_kept_says_it_was_not_stored() {
  let (scratch, data, standing) = stood().await;
  let path = data.path().join(format!("images/{ROOM}/2.jpg"));

  std::fs::create_dir_all(&path).unwrap();

  handed_text(&standing, 2, "", &[file("1", "photo.jpg", "image/jpeg", vec![0xff, 0xd8, 0xff, 0xd9])])
    .await;

  let line = line(&standing, 2).await;

  assert_eq!(image_path_or_none(&scratch, 2).await, String::new());
  assert!(line.contains(&CHAT_ROUND.shape.image_not_stored), "{line}");
  assert!(!line.trim_end().ends_with("):"), "the line is not empty words: {line}");
}

async fn image_path_or_none(scratch: &Scratch, message_id: i64) -> String {
  scratch
    .texts(&format!(
      r#"SELECT COALESCE("image_path", '') FROM "messages" WHERE "chat_id" = {ROOM} AND "message_id" = {message_id}"#
    ))
    .await
    .into_iter()
    .next()
    .unwrap_or_default()
}

#[tokio::test]
async fn a_file_and_a_voice_note_with_no_caption_carry_their_notes() {
  let (_scratch, _data, standing) = stood().await;

  handed_text(&standing, 3, "", &[file("1", "notes.txt", "text/plain", b"notes".to_vec())]).await;
  standing.hearing.saying("the backup finished", Some(2.5));
  handed_text(&standing, 4, "", &[voice()]).await;

  let file_line = line(&standing, 3).await;
  let voice_line = line(&standing, 4).await;

  assert!(file_line.contains("notes.txt") && file_line.contains("saved at"), "{file_line}");
  assert!(voice_line.contains("heard as: the backup finished"), "{voice_line}");
  assert!(!file_line.trim_end().ends_with("):") && !voice_line.trim_end().ends_with("):"));
}

#[tokio::test]
async fn a_text_message_is_unchanged() {
  let (_scratch, _data, standing) = stood().await;

  standing
    .receiver
    .message(&inbound(ROOM, 5, 55, "hello there"), &Context::default(), Room::default())
    .await
    .unwrap();

  let line = line(&standing, 5).await;

  assert!(line.ends_with("): hello there"), "{line}");
  assert!(!line.contains("[attached"), "{line}");
}

#[tokio::test]
async fn a_message_that_carried_nothing_stays_empty() {
  let (_scratch, _data, standing) = stood().await;

  standing
    .receiver
    .message(&inbound(ROOM, 6, 55, ""), &Context::default(), Room::default())
    .await
    .unwrap();

  let line = line(&standing, 6).await;

  assert!(line.ends_with("): "), "{line}");
  assert!(!line.contains("[attached"), "{line}");
}

#[tokio::test]
async fn a_file_past_the_cap_says_it_was_not_stored() {
  let (_scratch, _data, standing) = stood().await;
  let large = vec![b'x'; (CAP + 1) as usize];

  handed_text(&standing, 7, "", &[file("1", "big.bin", "application/octet-stream", large)]).await;

  let line = line(&standing, 7).await;

  assert!(
    line.contains(&receiver_policy::fill_marks(&CHAT_ROUND.shape.file_not_stored, &[("name", "big.bin")])),
    "{line}"
  );
  assert!(!line.trim_end().ends_with("):"), "{line}");
}

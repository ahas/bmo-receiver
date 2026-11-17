//! A room's placeholder rows filled later from its chat service (docs/room-files.md, 「Files filled in later」): a
//! placeholder row asked by its room, number and sent time and its files kept as a live line's are, one naming its
//! file after the head among them; each row asked once, its outcome recorded; a pass bounded, newest first, a dry
//! run asking nothing; a road that cannot be reached ending the pass with nothing recorded; and a room whose road
//! does not fetch files refused.

use std::io::Cursor;
use std::sync::Arc;

use bmo_chat_client::{ErrorCode, MessageFile};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::file_fills::{self, FillRefused};
use bmo_receiver::receiver::rows;
use bmo_receiver::{all, one};
use image::{ImageFormat, Rgb, RgbImage};
use receiver_client::{FileFill, FillOutcome};
use receiver_codes::FileFillRefusal;
use receiver_policy::roads::ROADS;

use super::inbound;
use crate::integration::common::Scratch;
use crate::integration::common::units::{FilesAnswer, StandInChat, Standing, standing};

fn png() -> Vec<u8> {
  let mut bytes = Cursor::new(Vec::new());

  RgbImage::from_pixel(4, 4, Rgb([10, 20, 30]))
    .write_to(&mut bytes, ImageFormat::Png)
    .unwrap();

  bytes.into_inner()
}

fn file(id: &str, name: &str, mime: &str, bytes: Vec<u8>) -> MessageFile {
  MessageFile {
    id: id.into(),
    name: name.into(),
    mime: mime.into(),
    bytes: bytes.into(),
    kind: None,
  }
}

async fn stood(room: i64, chat: StandInChat) -> (Scratch, tempfile::TempDir, Standing, Arc<StandInChat>) {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let chat = Arc::new(chat.holding(&[room]));
  let standing = standing(scratch.db(), DataRoot::new(data.path()), vec![chat.clone()]);

  (scratch, data, standing, chat)
}

// A row kept as the road handed it over, sent at its number's moment, so the newest is the highest number.
async fn kept(standing: &Standing, room: i64, message_id: i64, text: &str) {
  let mut row = inbound(room, message_id, 55, text);
  row.sent_at = 1_760_000_000_000 + message_id;

  rows::save_message(standing.receiver.db(), &row, None, None).await.unwrap();
}

fn pass(rows: Option<u32>, dry_run: bool) -> FileFill {
  FileFill { rows, dry_run }
}

async fn outcomes(standing: &Standing, room: i64) -> Vec<(i64, String)> {
  all(
    standing.receiver.db(),
    r#"SELECT "message_id", "outcome" FROM "message_file_asks" WHERE "chat_id" = $1 ORDER BY "message_id""#,
    vec![room.into()],
  )
  .await
  .unwrap()
  .iter()
  .map(|row| (row.try_get("", "message_id").unwrap(), row.try_get("", "outcome").unwrap()))
  .collect()
}

#[tokio::test]
async fn a_placeholder_row_is_filled_as_a_live_line_and_asked_once() {
  const ROOM: i64 = -131;

  let chat = StandInChat::new("road-a")
    .filing(11, FilesAnswer::Files(vec![file("0", "", "image/png", png())]))
    .filing(12, FilesAnswer::Files(vec![file("0", "notes.txt", "text/plain", b"notes".to_vec())]))
    .filing(13, FilesAnswer::Refused(404, ErrorCode::Unavailable, "the file expired"))
    .filing(14, FilesAnswer::Files(vec![]));
  let (_scratch, _data, standing, chat) = stood(ROOM, chat).await;

  kept(&standing, ROOM, 11, "[Photo] photo").await;
  kept(&standing, ROOM, 12, "[File: notes.txt]").await;
  kept(&standing, ROOM, 13, "[Video]").await;
  kept(&standing, ROOM, 14, "Emoticon").await;
  kept(&standing, ROOM, 15, "[Photo]graphy is a word, not a placeholder").await;
  kept(&standing, ROOM, 16, "words alone").await;

  let filled = file_fills::fill(&standing.receiver, ROOM, &pass(None, false)).await.unwrap();
  let reached: Vec<(i64, Option<FillOutcome>)> =
    filled.rows.iter().map(|row| (row.message, row.outcome)).collect();

  assert_eq!(
    reached,
    vec![
      (14, Some(FillOutcome::None)),
      (13, Some(FillOutcome::Unavailable)),
      (12, Some(FillOutcome::Filled)),
      (11, Some(FillOutcome::Filled)),
    ],
    "the placeholder rows alone are asked, newest first"
  );
  assert_eq!(filled.unreached, None);
  assert_eq!(filled.rows[1].reason.as_deref(), Some("the file expired"));
  assert_eq!(
    chat.files_asked.lock().unwrap().first().copied(),
    Some((14, Some(1_760_000_000_014))),
    "a row is asked by its sent time"
  );

  let photo = rows::message(standing.receiver.db(), ROOM, 11, "road-a").await.unwrap().unwrap();
  let path = photo.image_path.expect("the picture's path is on its row");
  assert!(path.ends_with(&format!("{ROOM}/11.jpg")) && std::path::Path::new(&path).is_file());
  assert_eq!(photo.image_state.as_deref(), Some("stored"));
  assert_eq!(photo.text, "[Photo] photo", "the row's words stand as they were");
  assert_eq!(filled.rows[3].files[0].kind, "image");
  assert!(filled.rows[3].files[0].bytes > 0);

  let document = one(
    standing.receiver.db(),
    r#"SELECT "kind", "stored_path" FROM "collected_items" WHERE "chat_id" = $1 AND "message_id" = 12"#,
    vec![ROOM.into()],
  )
  .await
  .unwrap()
  .expect("the document is kept with its row");
  assert_eq!(document.try_get::<String>("", "kind").unwrap(), "document");
  assert_eq!(filled.rows[2].files[0].bytes, 5);

  assert_eq!(
    outcomes(&standing, ROOM).await,
    vec![
      (11, "filled".to_owned()),
      (12, "filled".to_owned()),
      (13, "unavailable".to_owned()),
      (14, "none".to_owned())
    ]
  );

  let again = file_fills::fill(&standing.receiver, ROOM, &pass(None, false)).await.unwrap();

  assert!(again.rows.is_empty(), "no recorded row is asked again");
  assert_eq!(chat.asked_files().len(), 4);
}

#[tokio::test]
async fn a_placeholder_naming_its_file_after_the_head_is_asked() {
  const ROOM: i64 = -134;

  let chat = StandInChat::new("road-a")
    .filing(41, FilesAnswer::Files(vec![file("0", "plan.pdf", "application/pdf", b"%PDF".to_vec())]));
  let (_scratch, _data, standing, chat) = stood(ROOM, chat).await;

  kept(&standing, ROOM, 41, "[File plan.pdf]").await;
  kept(&standing, ROOM, 42, "[Filed under notes]").await;

  let filled = file_fills::fill(&standing.receiver, ROOM, &pass(None, false)).await.unwrap();

  assert_eq!(
    filled.rows.iter().map(|row| (row.message, row.outcome)).collect::<Vec<_>>(),
    vec![(41, Some(FillOutcome::Filled))],
    "a file named after the head is a placeholder; a word that only begins like the head is not"
  );
  assert_eq!(chat.asked_files(), vec![41]);
}

#[tokio::test]
async fn a_pass_is_bounded_and_a_dry_run_asks_nothing() {
  const ROOM: i64 = -132;

  let chat = StandInChat::new("road-a")
    .filing(21, FilesAnswer::Files(vec![]))
    .filing(22, FilesAnswer::Files(vec![]))
    .filing(23, FilesAnswer::Files(vec![]));
  let (_scratch, _data, standing, chat) = stood(ROOM, chat).await;

  for message in [21, 22, 23] {
    kept(&standing, ROOM, message, "[Photos]").await;
  }

  let listed = file_fills::fill(&standing.receiver, ROOM, &pass(Some(2), true)).await.unwrap();

  assert_eq!(
    listed.rows.iter().map(|row| (row.message, row.outcome)).collect::<Vec<_>>(),
    vec![(23, None), (22, None)]
  );
  assert!(chat.asked_files().is_empty() && outcomes(&standing, ROOM).await.is_empty());

  file_fills::fill(&standing.receiver, ROOM, &pass(Some(2), false)).await.unwrap();

  assert_eq!(chat.asked_files(), vec![23, 22], "a pass asks no more rows than it names");
  assert_eq!(file_fills::bounded(None), ROADS.filling.default_rows);
  assert_eq!(file_fills::bounded(Some(u32::MAX)), ROADS.filling.most_rows);
}

#[tokio::test]
async fn a_road_not_reached_ends_the_pass_with_nothing_recorded() {
  const ROOM: i64 = -133;

  let chat = StandInChat::new("road-a")
    .filing(31, FilesAnswer::Unreachable)
    .filing(32, FilesAnswer::Unreachable);
  let (_scratch, _data, standing, chat) = stood(ROOM, chat).await;

  kept(&standing, ROOM, 31, "[Voice]").await;
  kept(&standing, ROOM, 32, "[Voice]").await;

  let filled = file_fills::fill(&standing.receiver, ROOM, &pass(None, false)).await.unwrap();

  assert!(filled.rows.is_empty() && filled.unreached.is_some());
  assert_eq!(chat.asked_files(), vec![32], "the pass ends on the first road it cannot reach");
  assert!(outcomes(&standing, ROOM).await.is_empty(), "a later pass asks the same rows");
}

#[tokio::test]
async fn a_room_whose_road_fetches_no_files_is_refused() {
  const ROOM: i64 = -134;

  let (_scratch, _data, standing, _chat) = stood(ROOM, StandInChat::new("road-a")).await;

  kept(&standing, ROOM, 41, "[Photo]").await;

  let refused = file_fills::fill(&standing.receiver, ROOM, &pass(None, false)).await;

  assert!(matches!(
    refused,
    Err(FillRefused::Refused(FileFillRefusal::RoomFetchesNoFiles { room: ROOM }))
  ));
}

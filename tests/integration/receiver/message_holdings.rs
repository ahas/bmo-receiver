//! A room's messages asked for by what they hold (`docs/api.md`, 「Messages」; `docs/room-files.md`, 「A room's
//! files by kind」): media, files, voice notes, music and links, each page the newest first and walked by
//! `before`, a message holding several kinds standing under each, a document counted by the type its name implies,
//! and a kind the route does not know, or one asked together with a search, refused.

use axum::http::StatusCode;
use receiver_policy::roads::ROADS;
use serde_json::Value;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{Front, get, stood};
use crate::integration::common::units::OWNER;

const ROOM: i64 = -712;

// A seeded message: its number, its words, and the kind and the name of the file it kept, where it kept one.
type Seeded<'a> = (i64, &'a str, Option<(&'a str, &'a str)>);

// Fourteen messages, sent a second apart in the order of their numbers: the first two stored a picture, the second
// a sticker's, the fifth is a heard voice note, and eleven carry a kept file of the kind and the name beside them;
// the eleventh to thirteenth are documents a video, a sound and a file with no ending were sent as, and the last a
// recording neither heard nor marked, standing under the name the receiver gives a sound that came with none.
async fn bench() -> (Scratch, Front) {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;
  let voice_marked = format!("  {} 0:03", ROADS.marks.voice);
  let voice_named = format!("{}.ogg", ROADS.gathering.voice_name);
  let messages: [Seeded; 14] = [
    (1, "", None),
    (2, "", None),
    (3, "", Some(("video", "clip.mp4"))),
    (4, "", Some(("document", "notes.pdf"))),
    (5, "", Some(("audio", "voice.ogg"))),
    (6, &voice_marked, Some(("audio", "voice.ogg"))),
    (7, "a song", Some(("audio", "song.mp3"))),
    (8, "see HTTPS://example.org/page", None),
    (9, "no address here", None),
    (10, "http://example.com/loop", Some(("animation", "loop.mp4"))),
    (11, "", Some(("document", "clip.MOV"))),
    (12, "", Some(("document", "song.mp3"))),
    (13, "", Some(("document", "README"))),
    (14, "[Voice 0:17]", Some(("audio", &voice_named))),
  ];

  for (message, text, filed) in messages {
    scratch
      .run(
        r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text")
           VALUES ($1, $2, $3, 'someone', $4, $5)"#,
        vec![
          ROOM.into(),
          message.into(),
          OWNER.into(),
          ((1_760_000_000_i64 + message) * 1_000).into(),
          text.into(),
        ],
      )
      .await;

    if let Some((kind, source)) = filed {
      scratch
        .run(
          r#"INSERT INTO "collected_items" ("chat_id", "message_id", "sender_name", "kind", "source", "collected_at")
             VALUES ($1, $2, 'someone', $3, $4, 0)"#,
          vec![ROOM.into(), message.into(), kind.into(), source.into()],
        )
        .await;
    }
  }

  for (set, message) in [
    (r#""image_path" = 'picture.jpg'"#, 1_i64),
    (r#""image_path" = 'sticker.webp', "kind" = 'sticker'"#, 2),
    (r#""voice_state" = 'heard'"#, 5),
  ] {
    scratch
      .run(
        &format!(r#"UPDATE "messages" SET {set} WHERE "chat_id" = $1 AND "message_id" = $2"#),
        vec![ROOM.into(), message.into()],
      )
      .await;
  }

  (scratch, front)
}

// The ids of a page's messages, in its order, and whether older ones stand.
fn walked(page: &Value) -> (Vec<i64>, bool) {
  let ids = page["items"]
    .as_array()
    .unwrap()
    .iter()
    .map(|message| message["id"].as_i64().unwrap())
    .collect();

  (ids, page["more"].as_bool().unwrap())
}

async fn holding(front: &Front, query: &str) -> (Vec<i64>, bool) {
  let (status, page) = get(front, &format!("/rooms/{ROOM}/messages?{query}")).await;

  assert_eq!(status, StatusCode::OK, "{query}: {page}");

  walked(&page)
}

#[tokio::test]
async fn each_holding_answers_only_the_messages_holding_its_kind() {
  let (_scratch, front) = bench().await;

  assert_eq!(
    holding(&front, "holding=media").await,
    (vec![1, 3, 10, 11], false),
    "no sticker, a video's name"
  );
  assert_eq!(
    holding(&front, "holding=file").await,
    (vec![4, 13], false),
    "neither a video's nor a sound's name"
  );
  assert_eq!(
    holding(&front, "holding=voice").await,
    (vec![5, 6, 14], false),
    "heard, marked or nameless"
  );
  assert_eq!(holding(&front, "holding=music").await, (vec![7, 12], false), "a sound's name");
  assert_eq!(holding(&front, "holding=link").await, (vec![8, 10], false), "case set aside");
}

#[tokio::test]
async fn a_holding_page_is_the_newest_first_and_walked_by_before() {
  let (_scratch, front) = bench().await;

  assert_eq!(holding(&front, "holding=media&limit=2").await, (vec![10, 11], true));
  assert_eq!(holding(&front, "holding=media&limit=2&before=10").await, (vec![1, 3], false));
}

#[tokio::test]
async fn a_holding_the_route_does_not_know_or_one_asked_with_a_search_is_refused() {
  let (_scratch, front) = bench().await;

  for query in ["holding=pictures", "holding=media&q=song", "holding=link&after=1"] {
    let (status, answer) = get(&front, &format!("/rooms/{ROOM}/messages?{query}")).await;

    assert_eq!(
      (status, answer["code"].as_str()),
      (StatusCode::BAD_REQUEST, Some("bad-request")),
      "{query}"
    );
  }
}

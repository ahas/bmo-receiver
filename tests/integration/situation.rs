//! The room's situation note as the console reads it (docs/drafting.md, 「The situation note」): the note stands on
//! each message the round that wrote it judged, with whether that round changed it, and a room answers the note as it
//! stands with when it last changed.

use axum::http::StatusCode;
use serde_json::json;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{get, stood};
use bmo_receiver::chat::situation::{note_from_answer, update_prompt};

const ROOM: i64 = -5_200;
const OTHER: i64 = -5_201;

#[tokio::test]
async fn a_room_answers_its_note_and_when_it_last_changed() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, 1).await;

  let (status, none) = get(&front, &format!("/rooms/{ROOM}/situation")).await;

  assert_eq!(status, StatusCode::OK, "{none}");
  assert_eq!(none, json!({ "text": null, "updatedAt": null }));

  scratch
    .run(
      r#"INSERT INTO "room_situations" ("chat_id", "text", "updated_at") VALUES ($1, $2, $3)"#,
      vec![ROOM.into(), "Mina and Joon await the file.".into(), 1_700_000_000_000i64.into()],
    )
    .await;

  let (status, held) = get(&front, &format!("/rooms/{ROOM}/situation")).await;

  assert_eq!(status, StatusCode::OK, "{held}");
  assert_eq!(
    held,
    json!({ "text": "Mina and Joon await the file.", "updatedAt": 1_700_000_000_000i64 })
  );
  // Another room has no note of its own.
  let (_, other) = get(&front, &format!("/rooms/{OTHER}/situation")).await;

  assert!(other["text"].is_null(), "{other}");
}

#[tokio::test]
async fn a_message_carries_the_note_as_the_round_that_judged_it_read_it() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, 1).await;

  scratch.message(ROOM, 5, 1).await;
  scratch.message(ROOM, 7, 1).await;
  scratch
    .run(
      r#"UPDATE "messages" SET "situation" = $1, "situation_changed" = $2 WHERE "chat_id" = $3 AND "message_id" = $4"#,
      vec!["They wait on you.".into(), true.into(), ROOM.into(), 5i64.into()],
    )
    .await;

  let (status, held) = get(&front, &format!("/rooms/{ROOM}/messages/5")).await;

  assert_eq!(status, StatusCode::OK, "{held}");
  assert_eq!(held["situation"], json!({ "text": "They wait on you.", "changed": true }));

  // A message no round has judged under a note carries none.
  let (status, plain) = get(&front, &format!("/rooms/{ROOM}/messages/7")).await;

  assert_eq!(status, StatusCode::OK, "{plain}");
  assert!(plain["situation"].is_null(), "{plain}");
}

#[test]
fn a_note_summarizes_the_room_and_does_not_copy_its_latest_answer() {
  let latest = "[01-01 00:00Z] #7 agent-a: The correction was attached and sent.";
  let copied = "The correction was attached and sent.";

  assert_eq!(
    note_from_answer(copied, &[latest.to_owned()]),
    None,
    "the latest answer's sentence is not repeated"
  );
}

#[test]
fn a_note_may_not_hold_a_first_person_reply_claim() {
  assert_eq!(
    note_from_answer("I replied that the correction was sent.", &[]),
    None,
    "a first-person claim is refused even where nothing was copied"
  );
  let korean = std::fs::read_to_string("tests/fixtures/situation-claim-korean.txt")
    .unwrap()
    .trim()
    .to_owned();

  assert_eq!(note_from_answer(&korean, &[]), None);
}

#[test]
fn a_note_summarizing_the_same_exchange_passes() {
  assert_eq!(
    note_from_answer(
      "The owner asked for a correction to the answer; the correction is still to be sent.",
      &["[01-01 00:00Z] #7 agent-a: The correction was attached and sent.".to_owned()]
    )
    .as_deref(),
    Some("The owner asked for a correction to the answer; the correction is still to be sent.")
  );
}

#[test]
fn a_gap_past_the_policys_bound_is_read_as_its_newest_messages_alone() {
  use bmo_receiver::chat::situation::catch_up;

  assert!(!catch_up(200, 200), "a gap at the bound is read whole");
  assert!(catch_up(200, 201), "a gap past the bound is read as its newest messages");
  assert!(!catch_up(0, 100_000), "0 has no bound");
}

#[test]
fn the_note_prompt_bans_copying_and_first_person_claims() {
  let prompt = update_prompt(None, &[]);

  assert!(prompt.contains("nothing written here is sent to the room"), "{prompt}");
  assert!(prompt.contains("what this room is about"), "{prompt}");
  assert!(prompt.contains("how it is best answered"), "{prompt}");
  assert!(prompt.contains("Never copy a message"), "{prompt}");
  assert!(prompt.contains("never restate your own answer"), "{prompt}");
  assert!(prompt.contains("write no first-person claim"), "{prompt}");
}

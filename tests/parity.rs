//! The parity the news a writer outside the receiver holds to: the kinds its route takes and the emotions a
//! briefing may be said with, as another unit's own record of them has them, and a cursor read as the screens'
//! own code reads one.

use serde_json::{Value, json};

use bmo_receiver::routes::{TYPES, leading_whole};
use receiver_policy::chat_round::CHAT_ROUND;

#[test]
fn reads_a_cursor_as_the_screens_parse_their_whole_number() {
  assert_eq!(leading_whole("12"), Some(12));
  assert_eq!(leading_whole("12abc"), Some(12));
  assert_eq!(leading_whole(" -3"), Some(-3));
  assert_eq!(leading_whole("+7"), Some(7));
  assert_eq!(leading_whole("abc"), None);
  assert_eq!(leading_whole(""), None);
}

#[test]
fn holds_the_briefing_emotions_and_the_kinds_the_route_takes() {
  let written: Value = serde_json::from_str(
    &std::fs::read_to_string("tests/fixtures/event-parity.json").expect("the parity fixture"),
  )
  .expect("its JSON");

  assert_eq!(written["briefingEmotions"], json!(CHAT_ROUND.voice.emotions));
  assert_eq!(written["types"], json!(TYPES));
}

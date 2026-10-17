//! The room transcript's lines: an instant written as the transcript shows one, a line naming its sender and
//! tagging the owner, and a quote flattened and cut with its mark.

use bmo_receiver::chat::shape::{ShapeLine, clip_quote, iso_instant, shape_line};
use receiver_policy::chat_round::CHAT_ROUND;

#[test]
fn an_instant_is_written_as_the_bun_bot_writes_one() {
  assert_eq!(iso_instant(1_760_000_000), "2025-10-09T08:53:20.000Z");
}

#[test]
fn a_line_names_its_sender_and_tags_the_owner() {
  let notes = vec!["[a note]".to_owned()];
  let line = shape_line(&ShapeLine {
    sent_at: 0,
    message_id: 7,
    sender_name: "Owner",
    sender_id: Some(5),
    owner: true,
    text: "hello",
    notes: &notes,
  });

  assert_eq!(
    line,
    format!(
      "[1970-01-01T00:00:00.000Z] #7 Owner (id 5{}): hello [a note]",
      CHAT_ROUND.shape.owner_tag,
    ),
  );
}

#[test]
fn a_long_quote_is_flattened_and_cut_with_its_mark() {
  assert_eq!(clip_quote(" one\n two  "), "one two");
  assert!(clip_quote(&"a".repeat(300)).ends_with(&CHAT_ROUND.shape.quote_cut));
}

//! The room transcript's lines: an instant written as the transcript shows one, a line naming its sender and
//! tagging the owner, and a quote flattened and cut with its mark.

use bmo_receiver::chat::shape::{ShapeLine, clip_quote, clock, iso_instant, shape_line};
use receiver_policy::chat_round::CHAT_ROUND;

#[test]
fn an_instant_is_written_as_the_bun_bot_writes_one() {
  assert_eq!(iso_instant(1_760_000_000_000), "2025-10-09T08:53:20.000Z");
}

#[test]
fn a_message_s_instant_is_written_as_its_line_shows_it() {
  assert_eq!(clock(1_760_000_000_000), "10-09 08:53Z");
}

#[test]
fn a_line_names_its_sender_and_tags_the_owner() {
  let notes = vec!["[a note]".to_owned()];
  let line = shape_line(&ShapeLine {
    sent_at: 0,
    message_id: 7,
    sender_name: "Owner",
    owner: true,
    text: "hello",
    notes: &notes,
  });

  assert_eq!(line, format!("[01-01 00:00Z] #7 Owner{}: hello [a note]", CHAT_ROUND.shape.owner_tag),);
}

#[test]
fn a_long_quote_is_flattened_and_cut_with_its_mark() {
  assert_eq!(clip_quote(" one\n two  "), "one two");
  assert!(clip_quote(&"a".repeat(300)).ends_with(&CHAT_ROUND.shape.quote_cut));
}

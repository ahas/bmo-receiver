//! How a turn's quiet instructions are framed (docs/bmo-window.md, 「Quiet instruction」): the unit that
//! published each row is named beside its words, an unchanged repeat is one block saying how many arrived, and
//! the same words for different items name each item once.

use bmo_receiver::chat::instructions::{Notice, publisher_of, shown};
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::fill_marks;

const NOTE: &str = "the research unit has waited twenty minutes for its card";

fn notice(publisher: &str, subject: Option<(&str, &str)>) -> Notice {
  Notice {
    id: 0,
    text: NOTE.to_owned(),
    publisher: publisher.to_owned(),
    agent: "agent-a".to_owned(),
    room: -500,
    subject: subject.map(|(name, id)| (name.to_owned(), id.to_owned())),
  }
}

#[test]
fn an_unchanged_repeat_is_one_block_that_names_its_publisher_and_counts_them() {
  let shown = shown(&[notice("research", None), notice("research", None), notice("research", None)]);

  assert_eq!(shown.text.matches(NOTE).count(), 1, "the words stand once: {}", shown.text);
  assert!(
    shown
      .text
      .contains(&fill_marks(&CHAT_ROUND.instructions.source, &[("publisher", "research")])),
    "the publisher is named: {}",
    shown.text
  );
  assert!(
    shown
      .text
      .contains(&fill_marks(&CHAT_ROUND.instructions.agent, &[("agent", "agent-a")])),
    "the agent is named: {}",
    shown.text
  );
  assert!(
    shown.text.contains(&fill_marks(&CHAT_ROUND.instructions.room, &[("room", "-500")])),
    "the room is named: {}",
    shown.text
  );
  assert!(
    shown.text.contains(&fill_marks(&CHAT_ROUND.instructions.repeat, &[("count", "3")])),
    "how many arrived is said: {}",
    shown.text
  );
}

#[test]
fn the_same_words_for_two_items_keep_one_line_for_each_item() {
  let shown = shown(&[notice("job", Some(("job", "#11"))), notice("job", Some(("job", "#12")))]);

  assert_eq!(shown.text.matches(NOTE).count(), 1, "the words stand once: {}", shown.text);
  assert!(
    shown
      .text
      .contains(&fill_marks(&CHAT_ROUND.instructions.subject, &[("name", "job"), ("id", "#11")])),
    "item 11 is named: {}",
    shown.text
  );
  assert!(
    shown
      .text
      .contains(&fill_marks(&CHAT_ROUND.instructions.subject, &[("name", "job"), ("id", "#12")])),
    "item 12 is named: {}",
    shown.text
  );
  assert!(
    !shown.text.contains(&CHAT_ROUND.instructions.repeat),
    "a different item is no unchanged repeat: {}",
    shown.text
  );
}

#[test]
fn a_repeat_for_the_one_item_counts_on_that_item() {
  let shown = shown(&[
    notice("research", Some(("research", "#7"))),
    notice("research", Some(("research", "#7"))),
  ]);

  assert!(
    shown.text.contains(&fill_marks(
      &CHAT_ROUND.instructions.subject_times,
      &[("name", "research"), ("id", "#7"), ("count", "2")],
    )),
    "the item and its count stand on one line: {}",
    shown.text
  );
}

#[test]
fn the_same_words_from_two_publishers_are_two_blocks() {
  let shown = shown(&[notice("research", None), notice("job", None)]);

  assert_eq!(shown.text.matches(NOTE).count(), 2, "each publisher keeps its block: {}", shown.text);
}

#[test]
fn a_row_naming_no_publisher_takes_the_first_word_of_its_kind() {
  assert_eq!(publisher_of(Some("research-restore:flash-next")), "research");
  assert_eq!(publisher_of(Some("job-ended:12")), "job");
  assert_eq!(publisher_of(Some("road-changed:supervisor:a")), "road");
  assert_eq!(publisher_of(Some("overtime")), "overtime");
  assert_eq!(publisher_of(Some("")), CHAT_ROUND.instructions.unknown_publisher);
  assert_eq!(publisher_of(None), CHAT_ROUND.instructions.unknown_publisher);
}

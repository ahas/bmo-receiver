//! What opened a room's turns: the line a person reads an instruction by, and the rounds a room is handed, which
//! are only its own.

use bmo_receiver::routes::{SUMMARY_CHARS, rounds_of, summary_of};

#[test]
fn a_summary_is_the_producers_or_the_texts_first_line_cut_to_one_line() {
  assert_eq!(summary_of(Some("item #12 - queue-waiting"), "[header]\n{}"), "item #12 - queue-waiting");
  assert_eq!(
    summary_of(None, "\n  Documents you rely on were updated (2):\n- a\n- b"),
    "Documents you rely on were updated (2):",
  );
  assert_eq!(summary_of(Some("  "), "first\nsecond"), "first");
  assert_eq!(summary_of(None, ""), "");

  let long = "a word that a producer writes over and over again until one line is not enough".repeat(20);
  let cut = summary_of(None, &long);

  assert_eq!(cut.chars().count(), SUMMARY_CHARS + 1);
  assert!(cut.ends_with('…'));
}

#[test]
fn a_room_asks_only_its_own_rounds() {
  assert_eq!(
    rounds_of(7, "round:7:2:1, round:-7:1:1,round:70:1:1,round:7:1:1,round:7:1:1,"),
    vec!["round:7:1:1".to_owned(), "round:7:2:1".to_owned()],
  );
  assert_eq!(rounds_of(-7, "round:-7:1:1,round:7:1:1"), vec!["round:-7:1:1".to_owned()]);
  assert!(rounds_of(7, "round:7:1:1,round:7:2:1,round:7:3:1").len() <= 200);
}

//! The launcher's answer and the opening of a line: what a caller is told to let go of, and what is spoken for a
//! notice turn whose session launched no summary.

use bmo_receiver::chat::voice::{launched, line_opening};

#[test]
fn the_launchers_answer_tells_the_caller_to_let_go_of_it() {
  let said = launched("bmo-voice-1", 10_000_001);

  assert!(said.contains("bmo-voice-1") && said.contains("10000001"));
  // The two things a session most needs told: do not wait, and do not say it twice.
  assert!(said.contains("Do not wait for it") && said.contains("in text as well"));
}

#[test]
fn a_lines_opening_is_its_first_sentence_or_its_first_two_where_the_first_is_short() {
  assert_eq!(
    line_opening("The backup finished. Nothing else changed."),
    "The backup finished. Nothing else changed."
  );
  assert_eq!(
    line_opening(
      "The nightly backup of every disk on the machine finished without trouble. Nothing else changed."
    ),
    "The nightly backup of every disk on the machine finished without trouble."
  );
  assert_eq!(line_opening("Done!\n\nMore below"), "Done! More below");
  assert_eq!(line_opening(""), "");
}

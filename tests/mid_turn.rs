//! The mid-turn paragraph's opening: it names the push and what arrived as data, and carries no instruction
//! sentence but the line a push of quiet instructions alone ends on; the rule the marker stands for is said once in
//! the room's block.

use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::fill_marks;

use bmo_receiver::chat::mid_turn::note;

#[test]
fn the_opening_names_the_push_and_what_arrived() {
  let both = note(2, 1);
  let messages = note(3, 0);

  let joined = [
    fill_marks(&CHAT_ROUND.push.messages, &[("count", "2")]),
    fill_marks(&CHAT_ROUND.push.instructions, &[("count", "1")]),
  ]
  .join(&CHAT_ROUND.push.joined);

  assert!(both.contains(&CHAT_ROUND.push.marker), "{both}");
  assert!(both.contains(&joined), "{both}");
  assert!(messages.contains(&fill_marks(&CHAT_ROUND.push.messages, &[("count", "3")])));
  assert!(!messages.contains(&fill_marks(&CHAT_ROUND.push.instructions, &[("count", "0")])));
}

#[test]
fn the_opening_carries_no_instruction_sentence() {
  let push = &CHAT_ROUND.push;
  let both = note(1, 1);

  assert!(!both.contains(&push.rule), "the rule stands in the room's block: {both}");
  assert!(
    !both.contains("<<SEPARATE") && !both.contains("<<DONE"),
    "the marks are the block's to name: {both}"
  );
  assert!(push.rule.contains(&push.marker) || push.rule.contains("{marker}"), "{}", push.rule);
}

#[test]
fn a_push_of_quiet_instructions_alone_says_the_reply_written_before_it_stands() {
  let push = &CHAT_ROUND.push;
  let quiet = note(0, 2);

  assert!(quiet.starts_with(&push.marker), "{quiet}");
  assert!(quiet.ends_with(&push.kept), "the opening ends on the kept line: {quiet}");
  assert!(!note(1, 1).contains(&push.kept), "the room's words may change the reply");
  assert!(!note(2, 0).contains(&push.kept), "the room's words may change the reply");
}

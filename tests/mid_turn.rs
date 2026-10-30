//! The mid-turn paragraph's opening: it names only what arrived, and carries the rule each message is judged by
//! only where messages came into a push and how instructions are taken only where instructions came.

use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::fill_marks;

use bmo_receiver::chat::mid_turn::note;

#[test]
fn the_opening_names_only_what_arrived() {
  let both = note(2, 1, true);
  let messages = note(3, 0, true);

  let joined = [
    fill_marks(&CHAT_ROUND.push.messages, &[("count", "2")]),
    fill_marks(&CHAT_ROUND.push.instructions, &[("count", "1")]),
  ]
  .join(&CHAT_ROUND.push.joined);

  assert!(both.contains(&joined));
  assert!(messages.contains(&fill_marks(&CHAT_ROUND.push.messages, &[("count", "3")])));
  assert!(!messages.contains(&fill_marks(&CHAT_ROUND.push.instructions, &[("count", "0")])));
}

#[test]
fn the_rules_follow_what_arrived() {
  let push = &CHAT_ROUND.push;
  let both = note(1, 1, true);
  let messages = note(1, 0, true);
  let instructions = note(0, 1, true);

  assert!(both.contains(&push.judge) && both.contains(&push.heed), "{both}");
  assert!(messages.contains(&push.judge) && !messages.contains(&push.heed), "{messages}");
  assert!(
    !instructions.contains(&push.judge) && instructions.contains(&push.heed),
    "{instructions}"
  );
  assert!(
    !note(1, 0, false).contains(&push.judge),
    "a block under a question leans on the room's block"
  );
}

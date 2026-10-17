//! The mid-turn paragraph's opening: it names only what arrived.

use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::fill_marks;

use bmo_receiver::chat::mid_turn::note;

#[test]
fn the_opening_names_only_what_arrived() {
  let both = note(2, 1);
  let messages = note(3, 0);

  let joined = [
    fill_marks(&CHAT_ROUND.push.messages, &[("count", "2")]),
    fill_marks(&CHAT_ROUND.push.instructions, &[("count", "1")]),
  ]
  .join(&CHAT_ROUND.push.joined);

  assert!(both.contains(&joined));
  assert!(messages.contains(&fill_marks(&CHAT_ROUND.push.messages, &[("count", "3")])));
  assert!(!messages.contains(&fill_marks(&CHAT_ROUND.push.instructions, &[("count", "0")])));
}

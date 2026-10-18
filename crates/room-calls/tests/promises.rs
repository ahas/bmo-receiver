//! The sentences a call's own answer is made of, and what an answer in another room's language is (docs/chat-mcp.md).

use room_calls::promises::{answered_in_the_wrong_language, orders, own_sentences};

#[test]
fn sentences_end_after_their_marks_and_quotations_are_set_aside() {
  assert_eq!(own_sentences("One. Two!\nThree"), vec!["One.", " Two!", "\n", "Three"]);
  assert_eq!(own_sentences("said `code` and \"quoted\" here"), vec!["said   and   here"]);
}

#[test]
fn an_answer_in_the_room_s_language_or_in_a_room_that_speaks_none_is_never_turned_back() {
  let english = "This answer is written in English words only";

  assert!(!answered_in_the_wrong_language(english, ["hello there"]));
  assert!(!answered_in_the_wrong_language("see `the code` and /a/b/c", ["hello"]));
}

#[test]
fn a_question_is_no_order() {
  assert!(!orders("do it?"));
  assert!(!orders("   "));
}

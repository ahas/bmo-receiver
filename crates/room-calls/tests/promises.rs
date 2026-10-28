//! The sentences a call's own answer is made of, and what an answer in none of the languages of the people it
//! answers is (docs/answer-checks.md).

use room_calls::promises::{orders, own_sentences};

#[test]
fn sentences_end_after_their_marks_and_quotations_are_set_aside() {
  assert_eq!(own_sentences("One. Two!\nThree"), vec!["One.", " Two!", "\n", "Three"]);
  assert_eq!(own_sentences("said `code` and \"quoted\" here"), vec!["said   and   here"]);
}

#[test]
fn a_question_is_no_order() {
  assert!(!orders("do it?"));
  assert!(!orders("   "));
}

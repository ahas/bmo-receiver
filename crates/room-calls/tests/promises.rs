//! The sentences a call's own answer is made of, and what an answer in a language other than the turn's answer
//! language is (docs/answer-checks.md).

use room_calls::promises::{answered_in_the_wrong_language, orders, own_sentences};

#[test]
fn sentences_end_after_their_marks_and_quotations_are_set_aside() {
  assert_eq!(own_sentences("One. Two!\nThree"), vec!["One.", " Two!", "\n", "Three"]);
  assert_eq!(own_sentences("said `code` and \"quoted\" here"), vec!["said   and   here"]);
}

#[test]
fn an_answer_is_held_to_the_answer_language_and_its_code_never_turns_it_back() {
  let english = "This answer is written in English words only";

  assert!(!answered_in_the_wrong_language(english, "en"));
  assert!(answered_in_the_wrong_language(english, "ko"));
  assert!(!answered_in_the_wrong_language("see `the code` and /a/b/c", "ko"));
  assert!(!answered_in_the_wrong_language("v1.2.3 gpt-5 12:30", "ko"));
}

#[test]
fn a_question_is_no_order() {
  assert!(!orders("do it?"));
  assert!(!orders("   "));
}

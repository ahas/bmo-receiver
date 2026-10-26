//! The sentences a call's own answer is made of, and what an answer in none of the languages of the people it
//! answers is (docs/answer-checks.md).

use room_calls::promises::{answered_in_none_of, orders, own_sentences};

#[test]
fn sentences_end_after_their_marks_and_quotations_are_set_aside() {
  assert_eq!(own_sentences("One. Two!\nThree"), vec!["One.", " Two!", "\n", "Three"]);
  assert_eq!(own_sentences("said `code` and \"quoted\" here"), vec!["said   and   here"]);
}

#[test]
fn an_answer_is_held_to_the_languages_of_its_people_and_its_code_never_turns_it_back() {
  let english = "This answer is written in English words only";

  assert!(!answered_in_none_of(english, &["en"]));
  assert!(answered_in_none_of(english, &["ko"]));
  assert!(!answered_in_none_of(english, &["ko", "en"]));
  assert!(!answered_in_none_of("see `the code` and /a/b/c", &["ko"]));
  assert!(!answered_in_none_of("v1.2.3 gpt-5 12:30", &["ko"]));
}

#[test]
fn a_question_is_no_order() {
  assert!(!orders("do it?"));
  assert!(!orders("   "));
}

//! The readings an answer is held to: its own sentences cut with quotations set aside, an answer holding a
//! language of the people it answers never turned back and one holding none of theirs turned back, and a question
//! that is no order.

use bmo_receiver::chat::promises::{answered_in_none_of, orders, own_sentences, waits_on_happening};

// A sentence of the room's language, kept as data beside the tests that read it.
fn fixture(name: &str) -> String {
  std::fs::read_to_string(format!("tests/fixtures/{name}"))
    .unwrap_or_else(|why| panic!("the fixture {name} reads: {why}"))
    .trim()
    .to_owned()
}

#[test]
fn sentences_end_after_their_marks_and_quotations_are_set_aside() {
  assert_eq!(own_sentences("One. Two!\nThree"), vec!["One.", " Two!", "\n", "Three"]);
  assert_eq!(own_sentences("said `code` and \"quoted\" here"), vec!["said   and   here"]);
  assert_eq!(own_sentences("it's 'quoted' x"), vec!["it quoted' x"]);
}

#[test]
fn an_answer_holding_a_language_of_its_people_passes_and_one_holding_none_is_turned_back() {
  let english = "This answer is written in English words only";
  let korean = fixture("answer-korean.txt");

  assert!(!answered_in_none_of(english, &["en"]));
  assert!(answered_in_none_of(english, &["ko"]));
  assert!(!answered_in_none_of(&korean, &["ko"]));
  assert!(answered_in_none_of(&korean, &["en"]));
  assert!(
    !answered_in_none_of(&format!("{korean}\n\n{english}"), &["en", "ko"]),
    "each in their own passes"
  );
  assert!(!answered_in_none_of(&format!("{korean} `the code is here` /a/b/c"), &["ko"]));
  assert!(!answered_in_none_of("see `the code` and /a/b/c", &["ko"]));
}

#[test]
fn a_question_is_no_order() {
  assert!(!orders("do it?"));
  assert!(!orders("   "));
}

#[test]
fn a_promise_to_act_once_something_happened_waits_and_a_statement_of_the_day_does_not() {
  assert!(waits_on_happening(&fixture("promises-waiting.txt")));
  assert!(!waits_on_happening(&fixture("promises-not-waiting.txt")));
}

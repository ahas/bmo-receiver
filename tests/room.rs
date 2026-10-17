//! The room block's own readings: the names one agent answers to, and the order they are compared in.

use bmo_receiver::chat::room::{name_order, names_of};

#[test]
fn a_name_list_opens_on_the_owners_word() {
  assert_eq!(
    names_of("worker", Some("worker label"), Some("a, worker label, b")),
    "worker label, a, b"
  );
  assert_eq!(names_of("bare", None, None), "bare");
}

#[test]
fn names_compare_with_case_set_aside_first() {
  let mut names = vec!["b", "A", "a", "agent-w2", "agent-w10"];

  names.sort_by(|left, right| name_order(left, right));

  assert_eq!(names, vec!["a", "A", "b", "agent-w10", "agent-w2"]);
}

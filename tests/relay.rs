//! Who drives the receiver's news: the caller's own word flattened to one line and cut to what a journal line
//! carries, and the one line a windowful of calls is named by.

use bmo_receiver::events::relay::{RelayBurst, WHO_LIMIT, relay_caller};

#[test]
fn a_caller_is_one_line_cut_short_and_unnamed_where_it_said_nothing() {
  assert_eq!(relay_caller(Some("  todo.ts[1]\n forged  ")), "todo.ts[1] forged");
  assert_eq!(relay_caller(Some(" \n")), "unnamed");
  assert_eq!(relay_caller(None), "unnamed");
  let long = "a very long word that some caller writes about itself over and over again and again";

  assert_eq!(relay_caller(Some(long)).chars().count(), WHO_LIMIT);
}

#[test]
fn says_one_line_per_windowful_naming_the_commonest_first() {
  let burst = RelayBurst::default();

  for at in 0..19 {
    assert_eq!(burst.saw(if at < 15 { "flood" } else { "todo.ts" }, at * 100), None);
  }

  assert_eq!(
    burst.saw("other", 1_900),
    Some("relay: 20 events in 1.9s from flood x15, todo.ts x4, other".to_owned()),
  );
  assert_eq!(burst.saw("flood", 2_000), None);
  // Calls older than the window are no part of a burst with a later one.
  assert_eq!((0..19).filter_map(|at| burst.saw("late", 20_000 + at * 1_000)).count(), 0);
}

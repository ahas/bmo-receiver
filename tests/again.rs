//! The second asking's own words, the ending read off a failure's words, and the errands' forms.

use bmo_receiver::chat::again::{Ending, ending_of, record_errand, wake_errand, words};
use bmo_receiver::chat::conversation::Failure;
use bmo_receiver::chat::prompt::Scope;
use receiver_policy::second_asks::SECOND_ASKS;

#[test]
fn a_second_asking_closes_on_the_turn_s_scope_and_what_waits() {
  let scope = Scope {
    chat_id: -5,
    message_ids: vec![7],
    reference_message_ids: vec![],
    instructions: String::new(),
  };
  let said = words("Do it again.", &scope, &[8, 9]);

  assert!(said.starts_with("Do it again.\n"));
  assert!(said.contains(&format!("\n{}[8,9]\n", SECOND_ASKS.next_turn)));
  assert!(said.ends_with(&SECOND_ASKS.later));
}

#[test]
fn only_a_stop_is_a_stop_and_a_timeout_is_read_off_its_words() {
  assert_eq!(ending_of(&Failure::Stopped), Ending::Stopped);
  assert_eq!(ending_of(&Failure::Failed("the service timed out".to_owned())), Ending::Timeout);
  assert_eq!(ending_of(&Failure::Failed("broken".to_owned())), Ending::Error);
}

#[test]
fn an_errand_for_a_turn_with_its_tools_off_takes_the_promise_out() {
  assert!(record_errand(false).ends_with(&SECOND_ASKS.errands.record_off));
  assert!(wake_errand(true, -5).contains("chat -5"));
}

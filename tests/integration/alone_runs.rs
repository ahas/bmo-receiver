//! A test run alone (`common::alone`): its body runs in a process of its own, which sees the variable set
//! there and nowhere else.

use crate::integration::common::alone::alone;

const SET_ALONE: &str = "BMO_TEST_ALONE_SEEN";

#[test]
fn a_test_run_alone_runs_in_a_process_of_its_own() {
  let Some(_run) = alone(module_path!(), "a_test_run_alone_runs_in_a_process_of_its_own") else {
    assert!(std::env::var_os(SET_ALONE).is_none(), "the harness's process never set the variable");
    return;
  };

  // SAFETY: this process runs this one test alone, so no other thread reads the environment.
  unsafe { std::env::set_var(SET_ALONE, "1") };
  assert_eq!(std::env::var(SET_ALONE).as_deref(), Ok("1"));
}

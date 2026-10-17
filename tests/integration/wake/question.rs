//! The owner's words read against a job question's options (docs/bmo-window.md): a bare number, with or
//! without the suffix the policy allows, or a piece of text standing in exactly one option.

use receiver_policy::commands::COMMANDS;

use bmo_receiver::receiver::wake::question::chosen_option;

fn options() -> Vec<String> {
  ["moved to rust", "left on bun", "both stood"].map(str::to_owned).to_vec()
}

#[test]
fn a_number_or_a_piece_of_one_option_chooses_it() {
  assert_eq!(chosen_option("2", &options()), Some(1));
  let third = format!("3{}", COMMANDS.option_suffixes[0]);

  assert_eq!(chosen_option(&third, &options()), Some(2));
  assert_eq!(chosen_option("rust", &options()), Some(0));
}

#[test]
fn anything_else_is_talk() {
  assert_eq!(chosen_option("4", &options()), None);
  assert_eq!(chosen_option("02", &options()), None);
  assert_eq!(chosen_option("o", &options()), None);
  assert_eq!(chosen_option("1\n2", &options()), None);
  assert_eq!(chosen_option("  ", &options()), None);
}

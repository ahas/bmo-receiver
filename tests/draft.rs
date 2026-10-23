//! What the draft process reads without a model and the table it is matched by (docs/drafting.md): the skip in an
//! answer, a light turn, the checks on the final text, the row a turn reads and what a set may hold.

use bmo_receiver::chat::draft::checks::{Check, Reading, errand, failing, has_skip, light, own_lines};
use bmo_receiver::chat::draft::log::Answered;
use bmo_receiver::chat::draft::table::{
  ANY, PastCeiling, Process, Question, question_refusal, refusal, resolve,
};
use bmo_receiver::chat::draft::{Plan, question_words, read_answer, record_errand, wake_errand, words};
use bmo_receiver::chat::prompt::Scope;
use bmo_receiver::receiver::Inbound;
use receiver_policy::drafting::DRAFTING;
use receiver_policy::second_asks::SECOND_ASKS;

fn scope() -> Scope {
  Scope {
    chat_id: -5,
    message_ids: vec![7],
    reference_message_ids: vec![],
    instructions: String::new(),
  }
}

fn said(text: &str, picture: bool) -> Inbound {
  Inbound {
    chat_id: -5,
    message_id: 1,
    sender_id: Some(2),
    sender_name: "tester".into(),
    sent_at: 1,
    text: text.into(),
    image_path: picture.then(|| "a.jpg".to_owned()),
    reply_to_message_id: None,
    via: "road-a".into(),
    mode: None,
    via_reader: None,
    album: None,
  }
}

fn row(provider: &str, tier: &str, agent: &str, rounds: &[&str]) -> Process {
  Process {
    provider: provider.to_owned(),
    tier: tier.to_owned(),
    agent: agent.to_owned(),
    rounds: rounds.iter().map(|name| (*name).to_owned()).collect(),
    light: vec!["final".to_owned()],
    light_chars: 12,
    ceiling_seconds: 150,
    past_ceiling: PastCeiling::Send,
    set: false,
  }
}

fn catalogue() -> Vec<Question> {
  DRAFTING
    .questions
    .iter()
    .map(|(name, text)| Question {
      name: name.clone(),
      text: text.clone(),
      set: false,
    })
    .collect()
}

#[test]
fn an_item_closes_on_the_turn_s_scope_and_what_waits() {
  let said = words("Do it again.", &scope(), &[8, 9]);

  assert!(said.starts_with("Do it again.\n"));
  assert!(said.contains(&format!("\n{}[8,9]\n", SECOND_ASKS.next_turn)));
  assert!(said.ends_with(&SECOND_ASKS.later));
  assert!(record_errand(false).ends_with(&SECOND_ASKS.errands.record_off));
  assert!(wake_errand(true, -5).contains("chat -5"));
}

#[test]
fn a_question_names_its_place_and_the_last_says_it_is_the_message() {
  let first = question_words("intent", "why?", 1, 3, &scope());
  let last = question_words("final", "write it", 3, 3, &scope());

  assert!(first.contains("question 1 of 3 (intent)") && first.contains("\n\nwhy?\n"));
  assert!(
    first.contains("<<SEND>>") && first.contains("<<SKIP>>"),
    "the lead names the two markers"
  );
  assert!(last.contains("3 of 3 (final)") && last.contains("the message"));
}

#[test]
fn the_first_question_opens_on_the_opening_unless_it_is_the_last() {
  let plan = |names: &[&str]| Plan {
    questions: names.iter().map(|name| ((*name).to_owned(), format!("ask {name}"))).collect(),
    ..Plan::default()
  };
  let three = plan(&["intent", "information", "final"]);
  let one = plan(&["final"]);
  let head = DRAFTING.opening.split('{').next().unwrap_or_default();

  assert!(three.questioned() && !one.questioned() && !Plan::default().questioned());
  assert!(
    three
      .first_words()
      .is_some_and(|words| words.contains(head) && words.ends_with("\n\nask intent"))
  );
  assert!(
    one
      .first_words()
      .is_some_and(|words| words.contains("1 of 1 (final)") && !words.contains(head))
  );
  assert_eq!(Plan::default().first_words(), None);
}

#[test]
fn an_answer_opening_on_the_send_marker_is_the_message_and_a_skip_wins() {
  assert_eq!(read_answer("<<SEND>> hello there"), ("hello there".to_owned(), Answered::Send));
  assert_eq!(read_answer("  <<SEND>>\nthe answer"), ("the answer".to_owned(), Answered::Send));
  assert_eq!(read_answer("<<SEND>>").1, Answered::Skip);
  assert_eq!(read_answer("<<SEND>> then <<SKIP>>").1, Answered::Skip);
  assert_eq!(
    read_answer("notes that name <<SEND>> later"),
    ("notes that name <<SEND>> later".to_owned(), Answered::Answered)
  );
}

#[test]
fn a_skip_counts_outside_code_alone() {
  assert!(has_skip("nothing to add <<SKIP>>"));
  assert!(has_skip("<<SKIP>>"));
  assert!(!has_skip("the mark is written `<<SKIP>>` in code"));
  assert!(!has_skip("an answer"));
}

#[test]
fn a_turn_is_light_for_short_words_with_no_file_or_instructions_alone() {
  assert!(light(&[said("hi there", false)], 0, 12));
  assert!(!light(&[said("hi there", true)], 0, 12), "a file makes no turn light");
  assert!(!light(&[said("what does the plan for this week hold", false)], 0, 12));
  assert!(light(&[], 1, 12), "quiet instructions alone");
  assert!(!light(&[], 0, 12));
}

#[test]
fn the_checks_read_length_wording_and_the_process_s_own_lines() {
  let asked = vec!["why was it said?".to_owned()];
  let lines = own_lines(&asked);
  let reading = Reading {
    recent: &[],
    glossary: &[],
    own_lines: &lines,
    max_chars: 10,
  };

  assert_eq!(failing("short", &reading), vec![]);
  assert_eq!(failing("far too long a text", &reading), vec![Check::Length]);
  assert!(failing("why was it said?", &reading).contains(&Check::Format));
  assert!(errand(Check::Length, "far too long a text", &reading).contains("19 characters, past the 10"));
}

#[test]
fn the_most_specific_row_is_read() {
  let rows = vec![
    row(ANY, ANY, ANY, &["intent", "final"]),
    row("provider-a", ANY, ANY, &["final"]),
    row(ANY, "Supervisor", ANY, &["facts", "final"]),
    row(ANY, ANY, "agent-b", &["fit", "final"]),
  ];

  assert_eq!(resolve(&rows, Some("provider-b"), None, "agent-a").unwrap().rounds, ["intent", "final"]);
  assert_eq!(
    resolve(&rows, Some("provider-a"), Some("Supervisor"), "agent-a").unwrap().rounds,
    ["final"]
  );
  assert_eq!(
    resolve(&rows, Some("provider-b"), Some("Supervisor"), "agent-a").unwrap().rounds,
    ["facts", "final"]
  );
  assert_eq!(resolve(&rows, Some("provider-a"), None, "agent-b").unwrap().rounds, ["fit", "final"]);
}

#[test]
fn a_set_names_known_questions_ending_in_the_last_and_bounds_its_numbers() {
  let catalogue = catalogue();

  assert_eq!(refusal(&row(ANY, ANY, ANY, &["intent", "final"]), &catalogue), None);
  assert_eq!(refusal(&row(ANY, ANY, ANY, &[]), &catalogue), None, "an empty list asks nothing");
  assert!(refusal(&row(ANY, ANY, ANY, &["final", "intent"]), &catalogue).is_some());
  assert!(refusal(&row(ANY, ANY, ANY, &["final", "final"]), &catalogue).is_some());
  assert!(refusal(&row(ANY, ANY, ANY, &["unknown", "final"]), &catalogue).is_some());

  let mut long = row(ANY, ANY, ANY, &["final"]);

  long.ceiling_seconds = 0;
  assert!(refusal(&long, &catalogue).is_some());
  assert_eq!(question_refusal("tone-2", "a text"), None);
  assert!(question_refusal("Tone", "a text").is_some());
  assert!(question_refusal("tone", " ").is_some());
}

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
    image_state: None,
    transcript: None,
    voice_seconds: None,
    voice_state: None,
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
    send_from: String::new(),
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
  let plan = Plan {
    questions: [("information", "what?"), ("facts", "true?"), ("final", "write it")]
      .map(|(name, text)| (name.to_owned(), text.to_owned()))
      .to_vec(),
    send_from_at: 1,
    ..Plan::default()
  };
  let first = question_words("intent", "why?", 1, 3, &plan.send_line(0));
  let second = question_words("facts", "true?", 2, 3, &plan.send_line(1));
  let last = question_words("final", "write it", 3, 3, &plan.send_line(2));

  assert!(first.contains("question 1 of 3 (intent)") && first.ends_with("\n\nwhy?"));
  assert!(!first.contains("TURN SCOPE"), "a later question carries no turn scope: {first}");
  assert!(
    first.contains("question 2 (facts)") && first.contains("<<SEND>>") && !first.contains(&DRAFTING.voice),
    "a question before the one a send is read from may still carry a message, without the voice line: {first}"
  );
  assert!(
    second.contains("begin the answer with <<SEND>>"),
    "a send is read from facts on: {second}"
  );
  assert!(last.contains("3 of 3 (final)") && last.contains("the message"));
  assert!(!plan.send_open(0) && plan.send_open(1) && plan.send_open(2));
}

#[test]
fn every_question_whose_answer_may_be_the_message_asks_for_the_rooms_voice_and_notes_stay_notes() {
  let plan = Plan {
    questions: [("information", "what?"), ("facts", "true?"), ("final", "write it")]
      .map(|(name, text)| (name.to_owned(), text.to_owned()))
      .to_vec(),
    send_from_at: 1,
    ..Plan::default()
  };
  let notes = question_words("information", "what?", 1, 3, &plan.send_line(0));
  let facts = question_words("facts", &DRAFTING.questions["facts"], 2, 3, &plan.send_line(1));
  let last = question_words("final", &DRAFTING.questions["final"], 3, 3, &plan.send_line(2));

  assert!(DRAFTING.voice.contains("voice") && DRAFTING.voice.contains("never as notes"));
  assert!(
    facts.contains(&DRAFTING.voice),
    "the send-from question asks for the room's voice: {facts}"
  );
  assert!(last.contains(&DRAFTING.voice), "the last question asks for it: {last}");
  assert!(DRAFTING.questions["final"].contains("your persona sets"), "final's own text says so");
  assert!(DRAFTING.voice.contains("your persona sets") && DRAFTING.voice.contains("register"));
  assert!(!notes.contains(&DRAFTING.voice), "a notes question says nothing of the voice: {notes}");
  assert!(
    !plan.first_words().unwrap_or_default().contains(&DRAFTING.voice),
    "the first question, before the send-from one, names no voice; a message sent there is still the message"
  );
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
  assert!(three.first_words().is_some_and(|words| {
    words.contains(head) && words.contains(&DRAFTING.voice) && words.ends_with("\n\nask intent")
  }));
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
fn the_check_reads_the_process_s_own_lines_and_no_length() {
  let asked = vec!["why was it said?".to_owned()];
  let lines = own_lines(&asked);
  let reading = Reading { own_lines: &lines };

  assert_eq!(failing("short", &reading), vec![]);
  assert_eq!(failing(&"x".repeat(10_000), &reading), vec![], "no length bound");
  assert_eq!(failing("why was it said?", &reading), vec![Check::Format]);
  assert!(errand(Check::Format, "anything", &reading).contains("process's own lines"));
}

#[test]
fn the_most_specific_row_is_read() {
  let rows = vec![
    row(ANY, ANY, ANY, &["intent", "final"]),
    row("provider-a", ANY, ANY, &["final"]),
    row(ANY, "Supervisor", ANY, &["facts", "final"]),
    row(ANY, ANY, "agent-b", &["memory", "final"]),
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
  assert_eq!(resolve(&rows, Some("provider-a"), None, "agent-b").unwrap().rounds, ["memory", "final"]);
}

#[test]
fn a_set_names_known_questions_ending_in_the_last_and_bounds_its_numbers() {
  let catalogue = catalogue();

  assert_eq!(refusal(&row(ANY, ANY, ANY, &["intent", "final"]), &catalogue), None);
  assert_eq!(refusal(&row(ANY, ANY, ANY, &[]), &catalogue), None, "an empty list asks nothing");
  assert!(refusal(&row(ANY, ANY, ANY, &["final", "intent"]), &catalogue).is_some());
  assert!(refusal(&row(ANY, ANY, ANY, &["final", "final"]), &catalogue).is_some());
  assert!(refusal(&row(ANY, ANY, ANY, &["unknown", "final"]), &catalogue).is_some());

  let sending = |from: &str| Process {
    send_from: from.to_owned(),
    ..row(ANY, ANY, ANY, &["information", "facts", "final"])
  };

  assert_eq!(refusal(&sending("facts"), &catalogue), None);
  assert!(
    refusal(&sending("memory"), &catalogue).is_some(),
    "a send is read from a question of rounds"
  );

  assert_eq!(question_refusal("tone-2", "a text"), None);
  assert!(question_refusal("Tone", "a text").is_some());
  assert!(question_refusal("tone", " ").is_some());
}

#[test]
fn a_question_turn_carries_only_its_place_the_fixed_rules_stand_in_the_rooms_block() {
  let plan = |names: &[&str]| Plan {
    questions: names.iter().map(|name| ((*name).to_owned(), String::new())).collect(),
    ..Plan::default()
  };
  let one = plan(&["final"]).first_words().unwrap();
  let three = plan(&["intent", "information", "final"]).first_words().unwrap();
  let process = &receiver_policy::chat_line::CHAT_LINE.room.process;

  // The template a question adds to a turn is its place, its send line and, where its answer may be the message,
  // the room's voice: a few hundred characters at most.
  assert!(one.trim().chars().count() <= 350, "{} characters: {one}", one.chars().count());
  assert!(three.trim().chars().count() <= 650, "{} characters: {three}", three.chars().count());

  for rule in [
    "<<DONE #<number>>>",
    "<<SEPARATE #<number>>>",
    "validFrom",
    "live state",
    "ask nothing of you and change nothing",
  ] {
    assert!(!one.contains(rule) && !three.contains(rule), "{rule} stands in a question's turn");
    assert!(process.contains(rule), "{rule} stands in the room's block");
  }
}

#[test]
fn the_message_question_is_named_for_the_message_and_never_for_its_place_in_the_process() {
  let lead = &DRAFTING.last_lead;

  assert!(lead.contains("The message question"), "{lead}");
  assert!(!lead.contains("last question"), "{lead}");
  assert!(lead.contains("reviewed before it goes out"), "{lead}");
  assert!(!lead.contains("sent to the room as you write it"), "{lead}");
  assert!(!DRAFTING.send_open.contains("sent to this room as written"), "{}", DRAFTING.send_open);
  assert!(
    !DRAFTING.send_closed.contains("sent to this room as written"),
    "{}",
    DRAFTING.send_closed
  );
  assert!(
    receiver_policy::chat_line::CHAT_LINE
      .room
      .process
      .contains("returns it to be written again"),
    "the room's block carries the review's return"
  );
}

#[test]
fn the_closing_check_shows_the_message_as_it_stands_and_asks_for_it_whole() {
  use bmo_receiver::chat::draft::closing_words;

  let turn = closing_words("  the message as it stands  ", "#7: words that came");

  assert!(turn.contains(&format!("{}\nthe message as it stands\n", DRAFTING.standing)), "{turn}");
  assert!(turn.ends_with("#7: words that came"), "{turn}");
  assert!(DRAFTING.arrived.contains("whole") && !DRAFTING.arrived.contains("sent after it"));
}

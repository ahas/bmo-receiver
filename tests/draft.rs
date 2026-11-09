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
use receiver_policy::fill_marks;
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
    off: vec![],
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
  assert!(said.contains(&format!("\n{}[8,9]", SECOND_ASKS.next_turn)));
  assert!(!said.contains(&SECOND_ASKS.later), "the rule stands in the room's block: {said}");
  assert_eq!(record_errand(false), "ITEM: record.\nTools are off for this turn.");
  assert!(wake_errand(true, -5).contains("The room: -5."));
}

#[test]
fn a_round_names_its_place_and_whether_it_may_send() {
  let plan = Plan {
    questions: [("information", "what?"), ("facts", "true?"), ("final", "write it")]
      .map(|(name, text)| (name.to_owned(), text.to_owned()))
      .to_vec(),
    send_from_at: 1,
    ..Plan::default()
  };
  let first = question_words("intent", 1, 3, &plan.send_line(0));
  let second = question_words("facts", 2, 3, &plan.send_line(1));
  let last = question_words("final", 3, 3, &plan.send_line(2));

  assert_eq!(first, "Round 1/3 (intent): NOTES");
  assert_eq!(second, "Round 2/3 (facts): SEND");
  assert_eq!(last, "Round 3/3 (final): SEND");
  assert!(!first.contains("why?"), "a turn carries no question text: {first}");
  assert!(!first.contains("TURN SCOPE"), "a later round carries no turn scope: {first}");
  assert!(!plan.send_open(0) && plan.send_open(1) && plan.send_open(2));
  assert!(DRAFTING.send.contains("<<SEND>>") && DRAFTING.send.contains("reviewed before it goes out"));
  assert!(
    fill_marks(&DRAFTING.send, &[("voice", &DRAFTING.voice)]).contains(&DRAFTING.voice),
    "what a send does and the voice it carries stand in the block"
  );
}

#[test]
fn the_rooms_voice_stands_in_the_block_and_notes_stay_notes() {
  let plan = Plan {
    questions: [("information", "what?"), ("facts", "true?"), ("final", "write it")]
      .map(|(name, text)| (name.to_owned(), text.to_owned()))
      .to_vec(),
    send_from_at: 1,
    ..Plan::default()
  };
  let notes = question_words("information", 1, 3, &plan.send_line(0));
  let facts = question_words("facts", 2, 3, &plan.send_line(1));
  let last = question_words("final", 3, 3, &plan.send_line(2));

  assert!(DRAFTING.voice.contains("voice") && DRAFTING.voice.contains("never as notes"));
  assert!(DRAFTING.voice.contains("your persona sets") && DRAFTING.voice.contains("register"));
  assert!(
    fill_marks(&DRAFTING.send, &[("voice", &DRAFTING.voice)]).contains(&DRAFTING.voice),
    "the block says the voice once"
  );
  assert!(DRAFTING.questions["final"].contains("your persona sets"), "final's own text says so");

  for words in [notes, facts, last, plan.first_words().unwrap_or_default()] {
    assert!(!words.contains(&DRAFTING.voice), "no turn repeats the voice: {words}");
  }
}

#[test]
fn the_first_round_and_the_single_light_round_are_one_word_apart_from_the_block() {
  let plan = |names: &[&str]| Plan {
    questions: names.iter().map(|name| ((*name).to_owned(), format!("ask {name}"))).collect(),
    ..Plan::default()
  };
  let three = plan(&["intent", "information", "final"]);
  let one = plan(&["final"]);

  assert!(three.questioned() && !one.questioned() && !Plan::default().questioned());
  assert_eq!(three.first_words().as_deref(), Some("Round 1/3 (intent): SEND"));
  assert_eq!(one.first_words().as_deref(), Some("Round 1/1 (final): SEND"));
  assert!(
    !three.first_words().unwrap_or_default().contains("ask intent"),
    "the question text stands in the block"
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
  assert_eq!(errand(Check::Format, "anything", &reading), "CHECK: format.");
  assert!(SECOND_ASKS.errands.format.contains("process's own lines"));
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
fn a_question_switched_off_keeps_its_place_and_is_not_asked() {
  let named = |list: &[&str]| list.iter().map(|name| (*name).to_owned()).collect::<Vec<String>>();
  let switched = |off: &[&str], from: &str| {
    Process {
      off: named(off),
      send_from: from.to_owned(),
      ..row(ANY, ANY, ANY, &["information", "facts", "memory", "final"])
    }
    .settled()
  };
  let process = switched(&["memory", "intent", "facts", "facts"], "");

  assert_eq!(
    process.off,
    ["facts", "memory"],
    "only questions of the lists, once, in the lists' order"
  );
  assert_eq!(process.rounds, ["information", "facts", "memory", "final"], "each keeps its place");
  assert_eq!(process.asked(&process.rounds), ["information", "final"]);
  assert_eq!(process.asked(&process.light), ["final"]);
  assert_eq!(refusal(&process, &catalogue()), None);
  assert!(
    refusal(&switched(&["final"], ""), &catalogue()).is_some(),
    "the question that writes the message is never switched off"
  );

  // A send is asked for from the question named, or from the first asked after it where that one is off.
  assert_eq!(switched(&[], "memory").send_from_at(), 2);
  assert_eq!(switched(&["facts"], "memory").send_from_at(), 1);
  assert_eq!(switched(&["memory"], "memory").send_from_at(), 2, "final, the first asked after it");
  assert_eq!(switched(&["facts", "memory"], "facts").send_from_at(), 1);
  assert_eq!(switched(&["facts"], "").send_from_at(), 0);
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
fn a_round_turn_carries_only_its_place_never_the_stage_s_instruction_text() {
  let plan = |names: &[&str]| Plan {
    questions: names
      .iter()
      .map(|name| ((*name).to_owned(), DRAFTING.questions[*name].clone()))
      .collect(),
    ..Plan::default()
  };
  let one = plan(&["final"]).first_words().unwrap();
  let three = plan(&["intent", "information", "final"]).first_words().unwrap();
  let process = &receiver_policy::chat_line::CHAT_LINE.room.process;

  // The text a round adds to a turn is its place, its name and one word of send permission: a few dozen
  // characters at most. The question's own text, what a send does and the voice never ride it.
  assert!(one.trim().chars().count() <= 60, "{} characters: {one}", one.chars().count());
  assert!(three.trim().chars().count() <= 60, "{} characters: {three}", three.chars().count());
  assert!(!one.contains(&DRAFTING.questions["final"]), "{one}");
  assert!(!three.contains(&DRAFTING.questions["intent"]), "{three}");
  assert!(!one.contains(&DRAFTING.voice) && !three.contains(&DRAFTING.voice));
  assert_eq!(three, "Round 1/3 (intent): SEND");
  assert_eq!(one, "Round 1/1 (final): SEND");

  for rule in [
    "<<DONE #<number>>>",
    "<<SEPARATE #<number>>>",
    "validFrom",
    "live state",
    "ask nothing of you and change nothing",
    "shown to the room while you work",
  ] {
    assert!(!one.contains(rule) && !three.contains(rule), "{rule} stands in a round's turn");
    assert!(process.contains(rule), "{rule} stands in the room's block");
  }
}

#[test]
fn a_later_round_carries_its_place_and_permission_never_the_question_text() {
  let plan = Plan {
    questions: ["facts", "final"]
      .map(|name| (name.to_owned(), DRAFTING.questions[name].clone()))
      .to_vec(),
    send_from_at: 0,
    ..Plan::default()
  };
  let notes = question_words("facts", 1, 2, &plan.send_line(0));
  let message = question_words("final", 2, 2, &plan.send_line(1));

  assert_eq!(notes, "Round 1/2 (facts): SEND");
  assert_eq!(message, "Round 2/2 (final): SEND");
  assert!(!notes.contains(&DRAFTING.questions["facts"]), "{notes}");
  assert!(!message.contains(&DRAFTING.questions["final"]));
  assert!(!notes.contains("TURN SCOPE") && !message.contains("TURN SCOPE"));

  let sending = Plan {
    send_from_at: 1,
    ..plan
  };
  let notes_only = question_words("facts", 1, 2, &sending.send_line(0));

  assert_eq!(notes_only, "Round 1/2 (facts): NOTES");
}

#[test]
fn a_message_round_is_named_by_its_stage_and_the_block_says_what_a_send_does() {
  assert_eq!(DRAFTING.last_lead, "Round {number}/{count} ({name}): {send}");
  assert!(!DRAFTING.last_lead.contains("The message question"), "{}", DRAFTING.last_lead);
  assert!(DRAFTING.send.contains("reviewed before it goes out"), "{}", DRAFTING.send);
  assert!(DRAFTING.send.contains("<<SEND>>"), "{}", DRAFTING.send);
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

  assert!(turn.starts_with(&DRAFTING.arrived_marker), "{turn}");
  assert!(turn.contains(&format!("{}\nthe message as it stands\n", DRAFTING.standing)), "{turn}");
  assert!(turn.ends_with("#7: words that came"), "{turn}");
  assert!(
    !turn.contains("Answer with the message as it should now go out"),
    "the instruction stands in the room's block: {turn}"
  );
  assert!(DRAFTING.arrived.contains("whole") && !DRAFTING.arrived.contains("sent after it"));
}

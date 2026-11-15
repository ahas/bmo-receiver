//! What the draft process reads without a model and the table it is matched by (docs/drafting.md): the skip in an
//! answer, the checks on the final text, the row a turn reads and what a set may hold.

use bmo_receiver::chat::draft::checks::{Check, Reading, errand, failing, has_skip, own_lines};
use bmo_receiver::chat::draft::log::Answered;
use bmo_receiver::chat::draft::table::{
  ANY, PastCeiling, Process, Question, question_refusal, refusal, resolve,
};
use bmo_receiver::chat::draft::{
  Plan, Spanned, read_answer, record_errand, spanned_answer, wake_errand, words,
};
use bmo_receiver::chat::prompt::Scope;
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

fn row(provider: &str, tier: &str, agent: &str, rounds: &[&str]) -> Process {
  Process {
    provider: provider.to_owned(),
    tier: tier.to_owned(),
    agent: agent.to_owned(),
    rounds: rounds.iter().map(|name| (*name).to_owned()).collect(),
    send_from: String::new(),
    off: vec![],
    stages_off: vec![],
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
fn a_round_s_line_names_the_questions_it_follows_in_order() {
  let plan = |names: &[&str]| Plan {
    questions: names.iter().map(|name| ((*name).to_owned(), format!("ask {name}"))).collect(),
    ..Plan::default()
  };
  let three = plan(&["intent", "information", "final"]);
  let one = plan(&["final"]);

  assert_eq!(three.names().as_deref(), Some("intent+information+final"));
  assert_eq!(three.first_words().as_deref(), Some("DRAFT ROUND (intent+information+final)."));
  assert_eq!(one.first_words().as_deref(), Some("DRAFT ROUND (final)."));
  assert!(
    !three.first_words().unwrap_or_default().contains("ask intent"),
    "the question text stands in the block"
  );
  assert_eq!(Plan::default().names(), None, "a round that follows no question names none");
  assert_eq!(Plan::default().first_words(), None);
}

#[test]
fn the_rooms_voice_stands_in_the_block_and_no_turn_repeats_it() {
  let plan = Plan {
    questions: [("information", "what?"), ("facts", "true?"), ("final", "write it")]
      .map(|(name, text)| (name.to_owned(), text.to_owned()))
      .to_vec(),
    ..Plan::default()
  };

  assert!(DRAFTING.voice.contains("voice") && DRAFTING.voice.contains("never as notes"));
  assert!(DRAFTING.voice.contains("your persona sets") && DRAFTING.voice.contains("register"));
  assert!(
    fill_marks(&DRAFTING.stages_head, &[("voice", &DRAFTING.voice)]).contains(&DRAFTING.voice),
    "the block says the voice once"
  );
  assert!(DRAFTING.questions["final"].contains("your persona sets"), "final's own text says so");
  assert!(
    !plan.first_words().unwrap_or_default().contains(&DRAFTING.voice),
    "the round's line repeats no voice"
  );
}

#[test]
fn an_answer_opening_on_the_send_marker_is_the_message_and_a_skip_wins() {
  assert_eq!(read_answer("<<SEND>> hello there"), ("hello there".to_owned(), Answered::Send, None));
  assert_eq!(read_answer("  <<SEND>>\nthe answer"), ("the answer".to_owned(), Answered::Send, None));
  assert_eq!(
    read_answer("<<SEND>>").1,
    Answered::NoVerdict,
    "a marker with nothing after it names no reason"
  );
  assert_eq!(read_answer("<<SEND>> then <<SKIP>>").1, Answered::NoVerdict);
  assert_eq!(
    read_answer("notes that name <<SEND>> later"),
    ("notes that name <<SEND>> later".to_owned(), Answered::Answered, None)
  );
}

#[test]
fn a_skip_names_its_reason_and_a_bare_one_is_no_verdict() {
  let (said, ended, reason) = read_answer("<<SKIP: the words were not mine>>");

  assert_eq!(ended, Answered::Skip);
  assert_eq!(reason.as_deref(), Some("the words were not mine"), "the reason the round log keeps");
  assert!(said.contains("SKIP"));
  assert_eq!(read_answer("<<SKIP>>").1, Answered::NoVerdict, "a skip naming no reason is no verdict");
  assert_eq!(
    read_answer("<<SKIP:  >>").1,
    Answered::NoVerdict,
    "the form with nothing in it names no reason"
  );
  assert_eq!(read_answer("").1, Answered::NoVerdict, "no words name no reason");
  assert_eq!(read_answer("<<FOLD>>").1, Answered::NoVerdict, "a fold asked of no words is nothing");
  assert_eq!(
    read_answer("nothing to add <<SKIP: the picture asked nothing>>").1,
    Answered::Skip,
    "a skip anywhere in an answer wins"
  );
  assert_eq!(
    read_answer("<<SKIP>> but <<SKIP: the owner has it already>>").2.as_deref(),
    Some("the owner has it already"),
    "where an answer skips both ways, the reason is the verdict"
  );
  assert_eq!(
    read_answer("its form is `<<SKIP: why>>` in code").1,
    Answered::Answered,
    "a skip quoted in code names nothing"
  );
  assert_eq!(
    read_answer("<<SKIP:   the   picture   asked   nothing  >>").2.as_deref(),
    Some("the picture asked nothing"),
    "the reason is one line, its white space collapsed"
  );
}

#[test]
fn a_skip_counts_outside_code_alone() {
  assert!(has_skip("nothing to add <<SKIP>>"));
  assert!(has_skip("<<SKIP>>"));
  assert!(has_skip("nothing to add <<SKIP: nothing was asked>>"));
  assert!(!has_skip("the mark is written `<<SKIP>>` in code"));
  assert!(!has_skip("the form is written `<<SKIP: why>>` in code"));
  assert!(!has_skip("an answer"));
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
fn a_supervisor_reads_its_own_row_and_the_policy_s_where_it_has_none() {
  let rows = vec![row(ANY, ANY, ANY, &["intent", "final"]), row(ANY, ANY, "agent-b", &["memory", "final"])];

  assert_eq!(resolve(&rows, "agent-a").unwrap().rounds, ["intent", "final"]);
  assert_eq!(resolve(&rows, "agent-b").unwrap().rounds, ["memory", "final"]);
  assert_eq!(resolve(&rows[1..], "agent-a"), None, "another supervisor's row is no one else's");
}

#[test]
fn a_row_is_set_for_one_supervisor_alone() {
  let catalogue = [Question {
    name: "final".to_owned(),
    text: "write it".to_owned(),
    set: false,
  }];

  for (provider, tier, agent) in
    [(ANY, ANY, ANY), ("provider-a", ANY, "agent-a"), (ANY, "tier-a", "agent-a")]
  {
    assert!(
      refusal(&row(provider, tier, agent, &["final"]), &catalogue)
        .is_some_and(|why| why.contains("supervisor")),
      "{provider}/{tier}/{agent}"
    );
  }

  assert_eq!(refusal(&row(ANY, ANY, "agent-a", &["final"]), &catalogue), None);
}

#[test]
fn a_question_switched_off_keeps_its_place_and_is_not_asked() {
  let named = |list: &[&str]| list.iter().map(|name| (*name).to_owned()).collect::<Vec<String>>();
  let switched = |off: &[&str]| {
    Process {
      off: named(off),
      stages_off: vec![],
      ..row(ANY, ANY, "agent-a", &["information", "facts", "memory", "final"])
    }
    .settled()
  };
  let process = switched(&["memory", "intent", "facts", "facts"]);

  assert_eq!(
    process.off,
    ["facts", "memory"],
    "only questions of the lists, once, in the lists' order"
  );
  assert_eq!(process.rounds, ["information", "facts", "memory", "final"], "each keeps its place");
  assert_eq!(process.asked(&process.rounds), ["information", "final"]);
  assert_eq!(refusal(&process, &catalogue()), None);
  assert!(
    refusal(&switched(&["final"]), &catalogue()).is_some(),
    "the question that writes the message is never switched off"
  );
}

#[test]
fn a_set_names_known_questions_ending_in_the_last_and_bounds_its_numbers() {
  let catalogue = catalogue();

  assert_eq!(refusal(&row(ANY, ANY, "agent-a", &["intent", "final"]), &catalogue), None);
  assert_eq!(refusal(&row(ANY, ANY, "agent-a", &[]), &catalogue), None, "an empty list asks nothing");
  assert!(refusal(&row(ANY, ANY, "agent-a", &["final", "intent"]), &catalogue).is_some());
  assert!(refusal(&row(ANY, ANY, "agent-a", &["final", "final"]), &catalogue).is_some());
  assert!(refusal(&row(ANY, ANY, "agent-a", &["unknown", "final"]), &catalogue).is_some());

  let sending = Process {
    send_from: "memory".to_owned(),
    ..row(ANY, ANY, "agent-a", &["information", "facts", "final"])
  };

  assert_eq!(refusal(&sending, &catalogue), None, "a row's old send question is read by nothing");

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

  // The text a round adds to a turn is the names of the questions it follows: a few dozen characters at most.
  // The question's own text and the voice never ride it.
  assert!(one.trim().chars().count() <= 60, "{} characters: {one}", one.chars().count());
  assert!(three.trim().chars().count() <= 60, "{} characters: {three}", three.chars().count());
  assert!(!one.contains(&DRAFTING.questions["final"]), "{one}");
  assert!(!three.contains(&DRAFTING.questions["intent"]), "{three}");
  assert!(!one.contains(&DRAFTING.voice) && !three.contains(&DRAFTING.voice));
  assert_eq!(three, "DRAFT ROUND (intent+information+final).");
  assert_eq!(one, "DRAFT ROUND (final).");

  for rule in [
    "<<DONE #<number>>>",
    "<<SEPARATE #<number>>>",
    "<<SKIP: <why>>>",
    "validFrom",
    "live state",
    "ask nothing of you and change nothing",
  ] {
    assert!(!one.contains(rule) && !three.contains(rule), "{rule} stands in a round's turn");
    assert!(process.contains(rule), "{rule} stands in the room's block");
  }
}

#[test]
fn the_block_says_the_draft_round_is_one_turn_and_that_its_answer_is_reviewed() {
  let process = &receiver_policy::chat_line::CHAT_LINE.room.process;

  assert_eq!(DRAFTING.round, "DRAFT ROUND ({names}).");
  assert!(DRAFTING.stages_head.contains("in the order named"), "{}", DRAFTING.stages_head);
  assert!(DRAFTING.stages_head.contains("in that one turn"), "{}", DRAFTING.stages_head);
  assert!(
    DRAFTING.stages_head.contains("no question is asked again as a turn of its own"),
    "{}",
    DRAFTING.stages_head
  );
  assert!(process.contains("reviewed before it goes out"), "the room's block carries the review");
  assert!(
    process.contains("returns it to be done and written again"),
    "the room's block carries the review's return"
  );
  // The review audits whether the round's questions were carried out, and a revision does what was not done.
  assert!(
    DRAFTING.review.instruction.contains("carried it out"),
    "{}",
    DRAFTING.review.instruction
  );
  assert!(DRAFTING.review.revise.contains("with your tools"), "{}", DRAFTING.review.revise);
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

fn handed(text: &str) -> Spanned {
  Spanned {
    text: text.to_owned(),
    handed: true,
  }
}

fn program(text: &str) -> Spanned {
  Spanned {
    text: text.to_owned(),
    handed: false,
  }
}

#[test]
fn a_later_turn_that_says_nothing_leaves_the_answer_written_before_it() {
  let sent = "<<SEND>> the reply";
  let skipped = "<<SKIP: the instruction was handled; no room words>>";

  assert_eq!(spanned_answer(skipped, &[handed(sent), handed(skipped)]), sent);
  assert_eq!(spanned_answer("<<WAIT>>", &[handed(sent), handed("<<WAIT>>")]), sent);
  assert_eq!(spanned_answer("x", &[handed(sent), program("<<SKIP>>")]), sent);
  assert_eq!(
    spanned_answer("x", &[handed(sent), handed("<<SEND>> the reply, written again")]),
    "<<SEND>> the reply, written again"
  );
  assert_eq!(
    spanned_answer("x", &[handed(sent), program("the task ended")]),
    "<<SEND>> the reply\n\nthe task ended"
  );
  assert_eq!(spanned_answer("x", &[handed(skipped), handed(sent)]), sent);
  assert_eq!(spanned_answer("x", &[handed("<<SKIP>>"), handed(skipped)]), skipped);
  assert_eq!(spanned_answer("as built", &[]), "as built");

  let (text, ended, _) = read_answer(&spanned_answer(skipped, &[handed(sent), handed(skipped)]));

  assert_eq!((text.as_str(), ended), ("the reply", Answered::Send));
}

#[test]
fn the_answers_of_a_turns_end_are_read_with_what_opened_each() {
  let ended = serde_json::json!({
    "answer": "b",
    "answers": [{ "text": "a", "opened": "handed" }, { "text": "b", "opened": "program" }],
  });

  assert_eq!(Spanned::of(&ended), [handed("a"), program("b")]);
  assert!(Spanned::of(&serde_json::json!({ "answer": "b" })).is_empty());
}

//! What the instruction stages read with no model and the chain they are set by (docs/instruction.md): the answer
//! marks of a round's turn, a stage's verdict, a turn's spanned answer, the turn showing the words that arrived
//! meanwhile, and what a set may hold of the stage chain and its texts.

use bmo_receiver::chat::stages::log::Answered;
use bmo_receiver::chat::stages::table::{Chain, chain_refusal, followed, policy_holds, text_refusal};
use bmo_receiver::chat::stages::{Spanned, closing_words, read_answer, spanned_answer, stage_verdict};
use receiver_policy::instruction::INSTRUCTION;

#[test]
fn the_closing_check_shows_the_message_as_it_stands_and_asks_for_it_whole() {
  let turn = closing_words("  the message as it stands  ", "#7: words that came");

  assert!(turn.starts_with(&INSTRUCTION.arrived_marker), "{turn}");
  assert!(
    turn.contains(&format!("{}\nthe message as it stands\n", INSTRUCTION.standing)),
    "{turn}"
  );
  assert!(turn.ends_with("#7: words that came"), "{turn}");
  assert!(
    !turn.contains("Answer with the message as it should now go out"),
    "the instruction stands in the room's block: {turn}"
  );
}

#[test]
fn a_supervisor_follows_the_policy_s_chain_until_it_sets_its_own() {
  let policy = Chain::policy();
  let own = Chain {
    agent: "agent-a".to_owned(),
    stages: vec!["tone".to_owned()],
    set: true,
  };
  let rows = [policy.clone(), own.clone()];

  assert_eq!((policy.agent.as_str(), &policy.stages, policy.set), ("*", &INSTRUCTION.chain, false));
  assert_eq!(followed(&rows, "agent-a"), Some(&own));
  assert_eq!(
    followed(&rows, "agent-b"),
    Some(&policy),
    "a supervisor with no chain follows the policy's"
  );
}

#[test]
fn a_stage_chain_names_only_stages_whose_text_the_catalogue_holds() {
  let catalogue = INSTRUCTION.stages.keys().cloned().collect::<Vec<String>>();

  assert_eq!(chain_refusal(&INSTRUCTION.chain, &catalogue), None);
  let empty: Vec<String> = vec![];

  assert_eq!(chain_refusal(&empty, &catalogue), None, "an empty chain runs no stage");

  assert!(
    chain_refusal(&["unknown".to_owned()], &catalogue)
      .is_some_and(|why| why.contains("the catalogue does not hold")),
    "a stage with no text is refused"
  );

  let dup = Chain {
    agent: "agent-a".to_owned(),
    stages: ["review".to_owned(), "review".to_owned()].to_vec(),
    set: true,
  }
  .dedup();

  assert_eq!(dup.stages, ["review".to_owned()], "a chain names each stage once");
  assert!(policy_holds("review") && !policy_holds("unknown"));
}

#[test]
fn a_stage_text_is_a_lowercase_name_and_not_empty() {
  assert_eq!(text_refusal("tone-2", "a text"), None);
  assert!(text_refusal("Tone", "a text").is_some(), "a name starts lowercase");
  assert!(text_refusal("tone", " ").is_some(), "an empty text is refused");
}

#[test]
fn a_stage_passes_on_pass_alone_or_the_message_unchanged_and_revises_on_any_other_message() {
  let standing = "Hello, Mina.\nThe build is green.";

  for (said, ended) in [
    ("PASS", Answered::Answered),
    ("  pass \n", Answered::Answered),
    ("Hello, Mina. The build is green.", Answered::Answered),
    ("", Answered::Answered),
    ("<<WAIT>>", Answered::Answered),
    ("<<SKIP: nothing to change>>", Answered::Skip),
  ] {
    assert_eq!(
      stage_verdict(said, ended, standing),
      Answered::Passed,
      "{said:?} leaves the message as it stands"
    );
  }

  assert_eq!(
    stage_verdict("Hello, Ms. Mina.\nThe build is green.", Answered::Answered, standing),
    Answered::Revised,
    "a reprint that changes the message revises it"
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

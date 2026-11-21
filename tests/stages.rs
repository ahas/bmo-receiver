//! What the instruction stages read with no model and the chain they are set by (docs/instruction.md): the answer
//! marks of a round's turn, a stage's verdict, a turn's spanned answer, the turn showing the words that arrived
//! meanwhile, and what a set may hold of the stage chain and its texts.

use bmo_receiver::chat::stages::log::Answered;
use bmo_receiver::chat::stages::table::{Chain, chain_refusal, followed, policy_holds, text_refusal};
use bmo_receiver::chat::stages::{
  Spanned, closing_words, read_answer, room_line, spanned_answer, stage_verdict,
};
use bmo_receiver::receiver::Inbound;
use receiver_policy::instruction::INSTRUCTION;

#[test]
fn the_closing_check_shows_what_arrived_and_carries_no_message() {
  let turn = closing_words("#7: words that came");

  assert_eq!(turn, format!("{}\n\n#7: words that came", INSTRUCTION.arrived_marker));
}

fn said_by(sender_id: Option<i64>) -> Inbound {
  Inbound {
    chat_id: -100,
    message_id: 7,
    sender_id,
    sender_name: "tester".into(),
    sent_at: 1_760_000_000,
    text: "is the build green".into(),
    image_path: None,
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

#[test]
fn a_stage_shows_a_room_line_with_its_sender_s_id_plainly_and_without_one_where_none_came() {
  assert_eq!(room_line(&said_by(Some(42))), "#7 tester (id 42): is the build green");
  assert_eq!(room_line(&said_by(None)), "#7 tester: is the build green");
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
  assert!(text_refusal("2nd", "a text").is_some(), "a name opens on a letter");

  for name in ["message", "arrived", INSTRUCTION.audit.name.as_str()] {
    assert!(text_refusal(name, "a text").is_some(), "{name} is a fixed turn's own");
  }
}

#[test]
fn a_stage_passes_on_nothing_or_the_message_unchanged_and_revises_on_any_other_message() {
  let standing = "Hello, Mina.\nThe build is green.";

  for (said, ended) in [
    ("", Answered::Passed),
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

  for said in ["PASS", "  pass \n"] {
    assert_eq!(
      stage_verdict(said, Answered::Answered, standing),
      Answered::Revised,
      "{said:?} is message text, no verdict"
    );
  }
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
  assert_eq!(
    read_answer("PASS"),
    ("PASS".to_owned(), Answered::Answered, None),
    "PASS is the message's words"
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
  assert_eq!(
    spanned_answer("x", &[handed(sent), handed("")]),
    sent,
    "writing nothing keeps the reply"
  );
  assert_eq!(
    spanned_answer("x", &[handed(sent), handed("<<DONE #7>>")]),
    "<<SEND>> the reply\n\n<<DONE #7>>",
    "marks alone keep the reply and add their marks"
  );
  assert_eq!(
    spanned_answer("x", &[handed(sent), handed("PASS")]),
    "PASS",
    "any words are the new reply"
  );

  let (text, ended, _) = read_answer(&spanned_answer(skipped, &[handed(sent), handed(skipped)]));

  assert_eq!((text.as_str(), ended), ("the reply", Answered::Send));
}

// Words pushed into a working turn keep the reply written before them with the keep mark, and add to it with the
// append mark, so the reply is never written again for them (docs/instruction.md, 「What it is」).
#[test]
fn a_pushed_turn_keeps_the_reply_with_the_keep_mark_and_adds_to_it_with_the_append_mark() {
  let sent = "The reply to the first words.";

  let kept = spanned_answer("x", &[handed(sent), handed("<<KEEP>>")]);
  let (text, ended, _) = read_answer(&bmo_receiver::chat::sentinels::without_round_marks(&kept));

  assert_eq!((text.as_str(), ended), (sent, Answered::Answered), "{kept:?}");
  assert_eq!(
    spanned_answer("x", &[handed(sent), handed("<<APPEND>> And the answer to the new words.")]),
    "The reply to the first words.\n\nAnd the answer to the new words."
  );
  assert_eq!(
    spanned_answer("x", &[handed("<<SKIP: nothing yet>>"), handed("<<APPEND>> The first words answered.")]),
    "<<APPEND>> The first words answered.",
    "with no reply standing, an appending answer is the answer, its mark taken out as every mark is"
  );
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

#[test]
fn a_sheet_is_told_by_the_person_key_in_its_path_and_owed_by_what_the_brain_holds() {
  use bmo_receiver::chat::stages::audit::{Owed, owed, sheet_of};
  use receiver_ports::brain::Known;

  assert_eq!(sheet_of("rooms/42/people/7/sheet.md").as_deref(), Some("7"));
  assert_eq!(sheet_of("people/7"), None, "a key with nothing under it names no sheet");
  assert_eq!(sheet_of("memories/42/memory-9"), None);

  let known = |held, named| Known { held, named };

  assert_eq!(owed(known(true, true), false), Owed::Read);
  assert_eq!(owed(known(true, false), false), Owed::ReadAndAsk);
  assert_eq!(owed(known(true, false), true), Owed::Read, "a person asked once is not asked again");
  assert_eq!(owed(known(false, false), false), Owed::Ask);
  assert_eq!(owed(known(false, false), true), Owed::Nothing);
}

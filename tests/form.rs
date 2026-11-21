//! The words' form: the scaffolding cut where it begins, the reply sentinel taken off and the markers that stay
//! only where they are quoted.

use bmo_receiver::form::{
  ReplyTarget, read_reply_request, strip_prompt_scaffolding, without_control_markers,
};

#[test]
fn the_harness_s_scaffolding_is_cut_where_it_begins() {
  assert_eq!(
    strip_prompt_scaffolding("The answer.\n\nNew messages (1):\n[10-09 10:00Z] someone"),
    "The answer.",
  );
  assert_eq!(
    strip_prompt_scaffolding("The answer.\n\nChat -500 (Lab) · a group chat · RESTRICTED · owner 7"),
    "The answer.",
  );
  assert_eq!(strip_prompt_scaffolding("BMO ROOM CONTEXT all of it"), "");
  assert_eq!(strip_prompt_scaffolding("# Context Summary\nold\n---\nThe answer."), "The answer.");
}

#[test]
fn a_tool_call_written_as_text_never_reaches_a_room() {
  let call = "<tool_call>\n<function=mcp__job__job_reports_probe>\n<parameter=itemId>\n694\n</parameter>\n</function>\n</tool_call>";

  assert_eq!(strip_prompt_scaffolding(call), "", "a call alone is no message");
  assert_eq!(
    strip_prompt_scaffolding(&format!("Here is what I found.\n\n{call}\n\nThe rest of the answer.")),
    "Here is what I found.\n\n\n\nThe rest of the answer."
  );
  assert_eq!(
    strip_prompt_scaffolding(&format!("```\n{call}\n```")),
    format!("```\n{call}\n```"),
    "a call quoted in a fenced block stays"
  );
  assert_eq!(
    strip_prompt_scaffolding("<|tool_call|>probe<|/tool_call|>"),
    "",
    "the other wrapper a model writes goes too"
  );
}

#[test]
fn the_reply_sentinel_is_taken_off_and_names_its_message() {
  assert_eq!(read_reply_request("  just words "), ("just words".to_owned(), ReplyTarget::Plain));
  assert_eq!(read_reply_request("<<REPLY>> yes"), ("yes".to_owned(), ReplyTarget::Last));
  assert_eq!(read_reply_request("<<REPLY:42>>\nyes"), ("yes".to_owned(), ReplyTarget::Named(42)));
  assert_eq!(read_reply_request("<<REPLY:0>> yes"), ("yes".to_owned(), ReplyTarget::Last));
  assert_eq!(read_reply_request("<<REPLY:99999999999999999999>> yes").1, ReplyTarget::Last);
}

#[test]
fn a_marker_standing_as_text_goes_and_a_quoted_one_stays() {
  assert_eq!(without_control_markers("<<REPLY:12>> Hello <<SKIP>>"), "Hello");
  assert_eq!(without_control_markers("say `<<WAIT>>` there"), "say `<<WAIT>>` there");
  assert_eq!(
    without_control_markers("```\n<<SKIP>>\n```\nafter <<WAIT>>"),
    "```\n<<SKIP>>\n```\nafter",
  );
}

#[test]
fn a_round_mark_counts_outside_code_and_never_reaches_the_room() {
  use bmo_receiver::chat::sentinels::{Mark, round_marks, without_round_marks};

  let answer = "<<DONE #12>> The message. <<SEPARATE #13>> Quoted: `<<DONE #14>>`";

  assert_eq!(round_marks(answer), [(Mark::Done, 12), (Mark::Separate, 13)]);
  assert_eq!(without_control_markers("Hello <<DONE #12>>"), "Hello");
  assert_eq!(without_round_marks("<<SEPARATE #3>>"), "");
}

#[test]
fn a_mark_names_a_message_whose_number_is_negative() {
  use bmo_receiver::chat::sentinels::{Mark, round_marks, without_round_marks};

  let answer = "<<DONE #-998966603>> The message. <<SEPARATE #-998966604>> <<REPLY #-998966605>>";

  assert_eq!(
    round_marks(answer),
    [(Mark::Done, -998966603), (Mark::Separate, -998966604), (Mark::Reply, -998966605)]
  );
  assert_eq!(round_marks("<<REPLY:-998966605>>"), [(Mark::Reply, -998966605)]);
  assert_eq!(round_marks("Quoted: `<<DONE #-998966603>>`"), []);
  assert_eq!(without_round_marks(answer), "The message.");
  assert_eq!(
    without_control_markers("<<REPLY:-12>> Hello <<DONE #-13>> <<SEPARATE #-14>> <<REPLY #-15>>"),
    "Hello"
  );
  assert_eq!(
    read_reply_request("<<REPLY:-998966603>> yes"),
    ("yes".to_owned(), ReplyTarget::Named(-998966603))
  );
  assert_eq!(read_reply_request("<<REPLY:-99999999999999999999>> yes").1, ReplyTarget::Last);
  assert_eq!(read_reply_request("<<REPLY:-0>> yes").1, ReplyTarget::Last);
}

#[test]
fn a_reply_mark_naming_no_message_names_the_newest_the_round_answers() {
  use bmo_receiver::chat::sentinels::{Mark, round_marks, without_round_marks};

  assert_eq!(round_marks("<<REPLY>> The message."), [(Mark::Reply, 0)]);
  assert_eq!(round_marks("<<REPLY:0>> The message."), [(Mark::Reply, 0)]);
  assert_eq!(round_marks("<<REPLY #99999999999999999999>> The message."), [(Mark::Reply, 0)]);
  assert_eq!(round_marks("<<REPLY:-9007199254740992>> The message."), [(Mark::Reply, 0)]);
  assert_eq!(round_marks("<<DONE #99999999999999999999>> The message."), []);
  assert_eq!(round_marks("Quoted: `<<REPLY>>`"), []);
  assert_eq!(without_round_marks("<<REPLY>> The message."), "The message.");
}

#[test]
fn the_fold_mark_asks_a_fold_outside_code_and_never_reaches_the_room() {
  use bmo_receiver::chat::sentinels::{asks_fold, is_skip, is_wait, round_marks, without_round_marks};

  assert!(asks_fold("The lead.\n\n- one\n- two\n\n<<FOLD>>"));
  assert!(!asks_fold("A short answer."), "an unmarked message never asks a fold");
  assert!(!asks_fold("Quoted: `<<FOLD>>`"));
  assert!(!asks_fold("```\n<<FOLD>>\n```"));
  assert_eq!(without_control_markers("The lead. <<FOLD>>"), "The lead.");
  assert_eq!(without_control_markers("say `<<FOLD>>` there"), "say `<<FOLD>>` there");
  assert_eq!(round_marks("<<FOLD>> The message."), [], "the fold is no mark on a message shown");
  assert_eq!(
    without_round_marks("<<REPLY #4>> The message. <<FOLD>>"),
    "The message. <<FOLD>>",
    "the fold stays with the message's words"
  );
  assert!(is_wait("<<WAIT>> <<FOLD>>"), "a fold of no words is nothing beside a wait");
  assert!(is_skip("<<FOLD>><<SKIP>>"));
}

#[test]
fn the_keep_and_append_marks_count_outside_code_and_never_reach_the_room() {
  use bmo_receiver::chat::sentinels::{appended, appends, keeps, round_marks, without_round_marks};

  assert!(keeps("<<KEEP>>"));
  assert!(!keeps("Quoted: `<<KEEP>>`"));
  assert!(appends("<<APPEND>> One more line."));
  assert!(!appends("```\n<<APPEND>>\n```"));
  assert_eq!(round_marks("<<KEEP>>"), [], "the keep mark is no mark on a message shown");
  assert_eq!(without_round_marks("<<KEEP>> <<DONE #4>>"), "");
  assert_eq!(without_round_marks("<<APPEND>> One more line."), "One more line.");
  assert_eq!(without_control_markers("Hello <<KEEP>> <<APPEND>>"), "Hello");
  assert_eq!(
    appended("The reply.\n", "<<APPEND>> One more line. <<DONE #4>>"),
    "The reply.\n\nOne more line.",
    "an appending answer's words follow the message that stands, its marks taken out"
  );
  assert_eq!(appended("The reply.", "<<APPEND>>"), "The reply.", "an append of no words adds nothing");
}

#[test]
fn a_round_s_skip_names_its_reason_and_a_bare_one_names_none() {
  use bmo_receiver::chat::sentinels::{Skip, is_skip, skip, skip_alone};

  assert_eq!(
    skip("<<SKIP: nobody here was asked>>"),
    Some(Skip::Reasoned("nobody here was asked".to_owned()))
  );
  assert_eq!(skip("<<SKIP>>"), Some(Skip::Bare), "the bare marker names no reason");
  assert_eq!(skip("<<SKIP:>>"), Some(Skip::Bare), "the form with nothing after its colon names none");
  assert_eq!(skip("<<SKIP:    >>"), Some(Skip::Bare), "white space names no reason");
  assert_eq!(skip("words that name nothing"), None);
  assert_eq!(skip("quoted `<<SKIP: why>>` here"), None, "a skip quoted in code names nothing");
  assert_eq!(
    skip("<<SKIP>> then <<SKIP: one reason>>"),
    Some(Skip::Reasoned("one reason".to_owned())),
    "where the answer skips both ways, a reason is the verdict"
  );
  assert_eq!(skip("```\n<<SKIP: fenced>>\n```"), None, "a skip inside a fence is quoted");
  assert_eq!(
    skip_alone("<<SKIP: the picture asked nothing>>").as_ref().and_then(Skip::reason),
    Some("the picture asked nothing"),
    "an answer that is nothing but a skip carries its reason"
  );
  assert_eq!(skip_alone("<<SKIP>>"), Some(Skip::Bare));
  assert_eq!(skip_alone(""), Some(Skip::Bare), "no words name no reason");
  assert_eq!(
    skip_alone("<<REPLY>> <<SKIP: with a reason>>"),
    None,
    "a mark is not nothing but a skip"
  );
  assert_eq!(skip_alone("here is the message"), None);
  assert!(is_skip("<<SKIP: done here>>"), "a skip naming its reason says nothing either");
  assert!(is_skip("<<FOLD>><<SKIP>>"));
  assert_eq!(without_control_markers("<<SKIP: a reason>> Hello"), "Hello");
  assert_eq!(
    without_control_markers("say `<<SKIP: a reason>>` there"),
    "say `<<SKIP: a reason>>` there",
  );
}

#[test]
fn a_verdict_carries_the_words_without_their_markers() {
  use bmo_receiver::chat::turnover::verdict_of;

  assert_eq!(verdict_of("<<SEND>>Right, it is done."), "REPLY: Right, it is done.");
  assert_eq!(verdict_of("<<SEND>>"), "SKIP");
  assert_eq!(verdict_of("<<SKIP>>"), "SKIP");
}

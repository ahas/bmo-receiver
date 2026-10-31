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
fn a_verdict_carries_the_words_without_their_markers() {
  use bmo_receiver::chat::turnover::verdict_of;

  assert_eq!(verdict_of("<<SEND>>Right, it is done."), "REPLY: Right, it is done.");
  assert_eq!(verdict_of("<<SEND>>"), "SKIP");
  assert_eq!(verdict_of("<<SKIP>>"), "SKIP");
}

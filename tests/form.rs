//! The words' form: the scaffolding cut where it begins, the reply sentinel taken off and the markers that stay
//! only where they are quoted.

use bmo_receiver::form::{
  ReplyTarget, read_reply_request, strip_prompt_scaffolding, without_control_markers,
};

#[test]
fn the_harness_s_scaffolding_is_cut_where_it_begins() {
  assert_eq!(
    strip_prompt_scaffolding("The answer.\n\nNew messages (1):\n[2026-10-09T10:00:00Z] someone"),
    "The answer.",
  );
  assert_eq!(strip_prompt_scaffolding("BMO ROOM CONTEXT all of it"), "");
  assert_eq!(strip_prompt_scaffolding("# Context Summary\nold\n---\nThe answer."), "The answer.");
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

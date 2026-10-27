//! The turn's prompt: a document the session already holds goes in as its heading alone.

use std::collections::HashMap;

use bmo_receiver::chat::context::{State, core_names, opened, states};
use bmo_receiver::chat::window::once_block;
use bmo_receiver::js::bun_hash;

#[test]
fn a_document_the_session_holds_goes_in_as_its_heading() {
  let block = once_block(-5, "docs/x.md", "abc", None);
  let version = bun_hash("abc").to_string();
  let prompt = format!("{block}\n\nwhat the turn is for");
  let held = HashMap::from([("docs/x.md".to_owned(), version)]);

  assert_eq!(opened(&prompt, &held), "[docs/x.md] (already loaded)\n\nwhat the turn is for");
  assert_eq!(opened(&prompt, &HashMap::new()), "[docs/x.md]\nabc\n\nwhat the turn is for");
  assert_eq!(states(&prompt, &held).get("docs/x.md"), Some(&State::Held));
}

#[test]
fn a_wrapped_document_named_by_its_maker_keeps_its_name() {
  let named = once_block(-5, "threads/user-1", "words", Some("a thread"));
  let unnamed = once_block(-5, "docs/x.md", "abc", None);
  let prompt = format!("{named}\n{unnamed}\n\nwhat the turn is for");

  assert_eq!(
    core_names(&prompt),
    HashMap::from([("threads/user-1".to_owned(), "a thread".to_owned())])
  );
}

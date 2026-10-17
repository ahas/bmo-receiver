//! The turn's prompt: a document the session already holds goes in as its heading alone.

use std::collections::HashMap;

use bmo_receiver::chat::context::{State, opened, states};
use bmo_receiver::chat::window::once_block;
use bmo_receiver::js::bun_hash;

#[test]
fn a_document_the_session_holds_goes_in_as_its_heading() {
  let block = once_block(-5, "docs/x.md", "abc", "cleaned");
  let version = bun_hash("abc").to_string();
  let prompt = format!("{block}\n\nwhat the turn is for");
  let held = HashMap::from([("docs/x.md".to_owned(), version)]);

  assert_eq!(opened(&prompt, &held), "[docs/x.md] (already loaded)\n\nwhat the turn is for");
  assert_eq!(opened(&prompt, &HashMap::new()), "[docs/x.md]\ncleaned\n\nwhat the turn is for");
  assert_eq!(states(&prompt, &held).get("docs/x.md"), Some(&State::Held));
}

//! The window's own readings: a person-book document naming its person, and a document wrapped with its version.

use bmo_receiver::chat::window::{BLOCK_CLOSE, BLOCK_OPEN, once_block, person_book_owner};

#[test]
fn a_person_book_document_names_its_person_and_any_other_nobody() {
  assert_eq!(person_book_owner("people/55/sheet.md"), Some(55));
  assert_eq!(person_book_owner("rooms/-9/people/chunks/p7-work.md"), Some(7));
  assert_eq!(person_book_owner("knowledge/a.md"), None);
  assert_eq!(person_book_owner("/absolute/people/1/sheet.md"), None);
}

#[test]
fn a_wrapped_document_heads_itself_with_its_version() {
  let block = once_block(-5, "docs/x.md", "abc", None);

  assert_eq!(
    block,
    format!(
      "{BLOCK_OPEN}{{\"path\":\"docs/x.md\",\"chat\":-5,\"person\":null,\"version\":\"190542993387777138\"}}\n[docs/x.md]\nabc\n{BLOCK_CLOSE}",
    ),
  );

  let named = once_block(-5, "docs/x.md", "abc", Some("a reference document"));

  assert!(named.contains("\"holds\":\"a reference document\""), "{named}");
}

#[test]
fn the_due_line_follows_the_index_only_where_a_consolidation_is_due() {
  use bmo_receiver::chat::window::due_block;
  use receiver_policy::chat_round::CHAT_ROUND;

  assert_eq!(due_block("the index".to_owned(), false), "the index");
  assert_eq!(due_block("the index".to_owned(), true), format!("the index\n{}", CHAT_ROUND.index.due));
}

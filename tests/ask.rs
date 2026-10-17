//! What a turn is handed beside its words: the room's stretch first, then each document as it went in.

use std::collections::HashMap;

use bmo_receiver::chat::ask::handed_of;
use bmo_receiver::chat::context::{RoomSend, State};

#[test]
fn a_turn_is_handed_the_rooms_stretch_first_then_each_document_as_it_went_in() {
  let built = vec![
    ("knowledge".to_owned(), "knowledge/sample.md".to_owned()),
    ("person".to_owned(), "people/tester.md".to_owned()),
    ("rule".to_owned(), "rules/plain.md".to_owned()),
  ];
  let states = HashMap::from([
    ("knowledge/sample.md".to_owned(), State::Held),
    ("people/tester.md".to_owned(), State::Attached),
  ]);
  let handed = handed_of(-1003, Some(RoomSend::Mark), &built, &states);

  assert_eq!(
    handed
      .iter()
      .map(|(source, text, state)| (source.as_str(), text.as_str(), *state))
      .collect::<Vec<_>>(),
    vec![
      ("room", "chat -1003", Some("held")),
      ("knowledge", "knowledge/sample.md", Some("held")),
      ("person", "people/tester.md", Some("attached")),
      ("rule", "rules/plain.md", None),
    ],
  );
  assert_eq!(handed_of(-1003, Some(RoomSend::None), &[], &states), vec![]);
}

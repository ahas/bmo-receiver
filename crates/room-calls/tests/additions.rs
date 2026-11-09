//! A chat-service room added on the owner's word: the one mode the road sets and the one choice a call names
//! (docs/chat-mcp.md, 「The rooms tool」).

use std::collections::BTreeMap;

use room_calls::additions::{self, Chosen, MODE, NOT_OWNER};

#[test]
fn only_the_owner_mark_admits_a_room() {
  let owner = BTreeMap::from([("BMO_TURN_PRIVILEGED".to_owned(), "1".to_owned())]);
  let member = BTreeMap::from([("BMO_TURN_PRIVILEGED".to_owned(), "0".to_owned())]);

  assert_eq!(additions::admitted(&owner), Ok(()));
  assert_eq!(additions::admitted(&member), Err(NOT_OWNER.to_owned()));
  assert_eq!(additions::admitted(&BTreeMap::new()), Err(NOT_OWNER.to_owned()));
}

#[test]
fn the_road_sets_record_only_and_no_other_mode() {
  assert_eq!(MODE.as_str(), "record-only");
  assert!(additions::NOT_OWNER.contains("no grant in force names it"));
}

#[test]
fn one_choice_is_named_among_the_room_a_contact_and_the_contacts() {
  assert_eq!(additions::chosen(None, None, None, None), Ok(None));
  assert_eq!(additions::chosen(Some(7), None, None, Some("a room")), Ok(Some(Chosen::Room(7))));
  assert_eq!(additions::chosen(None, Some(9), None, None), Ok(Some(Chosen::Contact(9))));
  assert_eq!(
    additions::chosen(None, None, Some(vec![1, 2]), Some(" a group ")),
    Ok(Some(Chosen::Group {
      contact_ids: vec![1, 2],
      title: "a group".to_owned(),
    }))
  );
}

#[test]
fn more_than_one_choice_and_a_group_with_no_title_are_refused() {
  assert_eq!(
    additions::chosen(Some(7), Some(9), None, None),
    Err("A room to add names one of roomId, contactId or contactIds.".to_owned())
  );
  assert_eq!(
    additions::chosen(None, None, Some(vec![1]), None),
    Err("A group to start needs a title.".to_owned())
  );
}

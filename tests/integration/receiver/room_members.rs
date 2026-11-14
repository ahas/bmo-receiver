//! A room's members (docs/api.md, 「Rooms」): asked of the room's own chat service, the first its chosen list names,
//! answered as that service lists them, each person by the sender number the room's messages carry, and refused
//! with the service's own reason where it lists none, or where no chat service stands behind the room.

use std::sync::Arc;

use bmo_chat_client::{ChatCapability, ChatMember};
use bmo_receiver::roads::providers;
use serde_json::json;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{Front, get, stood_on};
use crate::integration::common::units::{OWNER, StandInChat};

const ROOM: i64 = -500;

fn member(person: Option<i64>, name: &str, status: Option<&str>) -> ChatMember {
  ChatMember {
    person,
    name: name.to_owned(),
    status_message: status.map(str::to_owned),
  }
}

// A front on the chat services given, the room titled so the receiver holds it.
async fn front(scratch: &Scratch, roads: Vec<Arc<StandInChat>>) -> Front {
  let front = stood_on(scratch, OWNER, roads).await;

  scratch
    .run(
      r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES (-500, 'Members Room')"#,
      vec![],
    )
    .await;

  front
}

#[tokio::test]
async fn a_rooms_members_are_answered_as_its_own_service_lists_them() {
  let scratch = Scratch::new().await;
  let own = Arc::new(
    StandInChat::new("road-b")
      .holding(&[ROOM])
      .canning(&[ChatCapability::MemberList])
      .membering(
        ROOM,
        vec![
          member(Some(9_000_000_000_001), "Person One", Some("Away")),
          member(None, "Person Two", None),
        ],
      ),
  );
  let other = Arc::new(
    StandInChat::new("road-a")
      .holding(&[ROOM])
      .canning(&[ChatCapability::MemberList])
      .membering(ROOM, vec![member(Some(5), "Someone Else", None)]),
  );
  let front = front(&scratch, vec![other.clone(), own.clone()]).await;

  providers::choose(scratch.db(), ROOM, &["road-b".to_owned(), "road-a".to_owned()])
    .await
    .unwrap();

  let (status, answer) = get(&front, &format!("/rooms/{ROOM}/members")).await;

  assert_eq!(status.as_u16(), 200, "{answer}");
  assert_eq!(
    answer,
    json!({ "members": [
      { "person": 9_000_000_000_001_i64, "name": "Person One", "statusMessage": "Away" },
      { "person": null, "name": "Person Two", "statusMessage": null },
    ] }),
    "the first provider the chosen list names answers, and a person it gave no number stands with none"
  );
  assert!(
    own.sends.lock().unwrap().is_empty() && other.sends.lock().unwrap().is_empty(),
    "nothing is sent"
  );
}

#[tokio::test]
async fn a_service_that_lists_no_members_of_the_room_is_refused_with_its_reason() {
  let scratch = Scratch::new().await;
  let unlisting = Arc::new(StandInChat::new("road-a").holding(&[ROOM]));
  let routeless = Arc::new(StandInChat::new("road-d").holding(&[ROOM]).routeless_members());
  let unfilled =
    Arc::new(StandInChat::new("road-b").holding(&[ROOM]).canning(&[ChatCapability::MemberList]));
  let down = Arc::new(StandInChat::new("road-c").holding(&[ROOM]).down());
  let front = front(&scratch, vec![unlisting, unfilled, down, routeless]).await;
  let path = format!("/rooms/{ROOM}/members");

  for (road, status, code, said) in [
    ("road-a", 409, "members-unlisted", "the road-a road does not list the members of a room"),
    ("road-d", 409, "members-unlisted", "the road-d road does not list the members of a room"),
    (
      "road-b",
      409,
      "members-unheld",
      "the road-b road gave no members of the room: The stand-in has read no members of room -500.",
    ),
    ("road-c", 502, "upstream-failed", "The road-c road did not answer"),
  ] {
    providers::choose(scratch.db(), ROOM, &[road.to_owned()]).await.unwrap();

    let (answered, body) = get(&front, &path).await;

    assert_eq!((answered.as_u16(), body["code"].as_str()), (status, Some(code)), "{road}: {body}");
    assert!(body["error"].as_str().is_some_and(|error| error.starts_with(said)), "{road}: {body}");
  }
}

#[tokio::test]
async fn a_room_with_no_chat_service_behind_it_is_refused_and_a_room_not_held_is_not_found() {
  let scratch = Scratch::new().await;
  let road = Arc::new(StandInChat::new("road-a").holding(&[ROOM]).canning(&[ChatCapability::MemberList]));
  let front = front(&scratch, vec![road]).await;

  providers::choose(scratch.db(), ROOM, &["record".to_owned()]).await.unwrap();

  let (status, body) = get(&front, &format!("/rooms/{ROOM}/members")).await;

  assert_eq!((status.as_u16(), body["code"].as_str()), (409, Some("record-only-room")), "{body}");

  let (status, body) = get(&front, "/rooms/-501/members").await;

  assert_eq!((status.as_u16(), body["code"].as_str()), (404, Some("not-found")), "{body}");
}

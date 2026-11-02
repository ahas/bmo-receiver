//! What the receiver keeps for a moment (`docs/receiver.md`, 「What is kept for a moment」): a chat service's facts
//! of a room and the agency's list of agents are asked once within their span and again once it has passed, the
//! room list reads the agents once for all its rooms, and a page of a room's line reads the room's facts kept.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::http::StatusCode;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::wake::agents;
use bmo_receiver::roads::Roads;
use receiver_ports::chat::{Chat, ChatServices};

use crate::integration::common::Scratch;
use crate::integration::common::queue::{get, stood_on};
use crate::integration::common::units::{AGENT, OWNER, StandInChat, registration, standing};

// The span the tests keep an answer for: long enough that a read straight after another falls inside it.
const SPAN: Duration = Duration::from_millis(1500);

// A room the chat service holds, as the owner's private chat, and one it does not.
const HELD: i64 = -701;
const NOT_HELD: i64 = -702;

fn service() -> Arc<StandInChat> {
  Arc::new(StandInChat::new("chat-a").owners(HELD).agent_in(HELD, 7))
}

fn asked(chat: &StandInChat) -> usize {
  chat.rooms_asked.load(Ordering::SeqCst)
}

#[tokio::test]
async fn a_rooms_facts_are_asked_once_within_the_span_and_again_after_it() {
  let chat = service();
  let roads =
    Roads::keeping_for(ChatServices::of([("chat-a".to_owned(), chat.clone() as Arc<dyn Chat>)]), SPAN);

  assert!(roads.room_facts(HELD).await.owner_room);
  assert_eq!(roads.identity_in(HELD).await, Some(7));
  assert!(roads.is_owner_room(HELD).await);
  assert_eq!(roads.room_road(HELD).await.as_deref(), Some("chat-a"));
  assert_eq!(asked(&chat), 1, "every fact of the room within the span is the one answer kept");

  assert_eq!(roads.room_road(NOT_HELD).await, None);
  assert!(!roads.is_owner_room(NOT_HELD).await);
  assert_eq!(asked(&chat), 2, "a room the service does not hold is kept as not held");

  roads.forget_room(HELD);
  assert!(roads.is_owner_room(HELD).await);
  assert_eq!(asked(&chat), 3, "a room let go is asked again");

  tokio::time::sleep(SPAN + Duration::from_millis(100)).await;

  assert!(roads.is_owner_room(HELD).await);
  assert!(!roads.is_owner_room(NOT_HELD).await);
  assert_eq!(asked(&chat), 5, "past the span each room is asked again");
}

#[tokio::test]
async fn the_agents_are_asked_once_within_the_span_and_again_after_it() {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let standing = standing(scratch.db(), DataRoot::new(data.path()), vec![]);
  let receiver = standing.receiver.clone().keeping_agents_for(SPAN);
  let asked = || standing.agency.agents_asked.load(Ordering::SeqCst);

  standing.agency.registering(vec![registration(AGENT, "Agent A", vec![], None, true)]);

  assert!(agents::registered(&receiver, AGENT).await.unwrap().is_some());
  assert_eq!(agents::default_designated(&receiver).await.unwrap().name, AGENT);
  assert!(receiver.agents().await.is_ok());
  assert_eq!(asked(), 1, "every read within the span is the one list kept");

  receiver.forget_agents();
  assert!(agents::registered(&receiver, AGENT).await.unwrap().is_some());
  assert_eq!(asked(), 2, "a list let go is asked again");

  tokio::time::sleep(SPAN + Duration::from_millis(100)).await;

  assert!(agents::registered(&receiver, AGENT).await.unwrap().is_some());
  assert_eq!(asked(), 3, "past the span the agency is asked again");
}

#[tokio::test]
async fn the_room_list_reads_the_agents_once_and_a_page_reads_the_rooms_facts_kept() {
  let scratch = Scratch::new().await;
  let chat = service();
  let front = stood_on(&scratch, OWNER, vec![chat.clone()]).await;
  let agency = front.standing.agency.clone();

  for (room, message) in [(HELD, 1), (HELD, 2), (NOT_HELD, 1)] {
    scratch.message(room, message, OWNER).await;
  }

  let (status, listed) = get(&front, "/rooms").await;

  assert_eq!(status, StatusCode::OK, "{listed}");
  assert_eq!(listed["items"].as_array().unwrap().len(), 2);
  assert_eq!(agency.agents_asked.load(Ordering::SeqCst), 1, "one read of the agents for every room");
  assert_eq!(asked(&chat), 2, "each room's facts asked once");

  assert_eq!(get(&front, "/rooms").await.0, StatusCode::OK);
  assert_eq!(asked(&chat), 2, "the second list reads the facts kept");

  for path in [
    format!("/rooms/{HELD}/messages?limit=1"),
    format!("/rooms/{NOT_HELD}/messages?limit=100"),
  ] {
    let (status, page) = get(&front, &path).await;

    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(page["items"].as_array().unwrap().len(), 1, "{path}: {page}");
  }

  assert_eq!(asked(&chat), 2, "a page reads the room's facts kept");
}

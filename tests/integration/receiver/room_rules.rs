//! A room's delegations (docs/unified-bmo.md, 「Delegations」): the room's block carries the owner's room rules, the
//! live `rule` memories of the agent's brain about the room, whole and every time it is composed, so a rule stands
//! however often it is read; a room holding none carries the empty set, and a brain that does not answer carries
//! the line saying nothing is delegated until it does.

use std::sync::Arc;

use bmo_receiver::chat::room::room_context;
use receiver_policy::chat_line::CHAT_LINE;
use receiver_policy::fill_marks;
use receiver_ports::brain::RoomRule;
use serde_json::json;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{Front, post, stood_on};
use crate::integration::common::units::{AGENT, OWNER, StandInChat};

const ROOM: i64 = -100;
const OTHER: i64 = -101;

// A front reaching one chat service that holds both rooms, its data root holding the chat's documents.
async fn front(scratch: &Scratch) -> Front {
  let service = Arc::new(StandInChat::new("road-a").holding(&[ROOM, OTHER]));
  let front = stood_on(scratch, OWNER, vec![service]).await;
  let chat = front.data.path().join("prompts").join("chat");

  std::fs::create_dir_all(&chat).unwrap();
  std::fs::write(chat.join(&CHAT_LINE.room.owner_document), "The owner's own chat.").unwrap();
  std::fs::write(chat.join(&CHAT_LINE.room.group_document), "A group room.").unwrap();
  std::fs::write(chat.join(&CHAT_LINE.capabilities.document), "What the agent can do.").unwrap();

  for room in [ROOM, OTHER] {
    let (status, added) =
      post(&front, "/rooms", json!({ "provider": "road-a", "roomId": room, "mode": "record-only" })).await;

    assert_eq!(status.as_u16(), 201, "{added}");
  }

  front
}

async fn block(front: &Front, room: i64) -> String {
  room_context(&front.receiver(), AGENT, room, None).await.unwrap().text
}

fn rules_block(rules: &[RoomRule]) -> String {
  fill_marks(&CHAT_LINE.rules.room_rules, &[("rows", &serde_json::to_string(rules).unwrap())])
}

#[tokio::test]
async fn the_rooms_block_carries_its_owner_room_rules_whole_every_time_it_is_composed() {
  let scratch = Scratch::new().await;
  let front = front(&scratch).await;
  let system = RoomRule {
    id: 31,
    persons: vec![],
    text: "Every member of this room may ask about the system.".to_owned(),
  };
  let launch = RoomRule {
    id: 32,
    persons: vec!["42".to_owned()],
    text: "Member 42's own equipment work is approved and run at once.".to_owned(),
  };

  assert!(
    block(&front, ROOM).await.contains(&rules_block(&[])),
    "a room holding no rule carries the empty set"
  );

  front.standing.brain.ruling(ROOM, system.clone());
  front.standing.brain.ruling(ROOM, launch.clone());

  let first = block(&front, ROOM).await;
  let second = block(&front, ROOM).await;
  let both = rules_block(&[system, launch]);

  assert!(first.contains(&both), "{first}");
  assert!(second.contains(&both), "a rule read once stands on the next block: {second}");
  assert!(
    block(&front, OTHER).await.contains(&rules_block(&[])),
    "another room's block carries none of this room's rules"
  );
}

#[tokio::test]
async fn a_brain_that_does_not_answer_leaves_nothing_delegated_in_the_room() {
  let scratch = Scratch::new().await;
  let front = front(&scratch).await;

  front.standing.brain.ruling(
    ROOM,
    RoomRule {
      id: 31,
      persons: vec![],
      text: "Every member of this room may ask about the system.".to_owned(),
    },
  );
  *front.standing.brain.rules_down.lock().unwrap() = true;

  let text = block(&front, ROOM).await;

  assert!(text.contains(&CHAT_LINE.rules.rules_down), "{text}");
  assert!(!text.contains("may ask about the system"), "{text}");
}

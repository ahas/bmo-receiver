//! Which rooms the receiver listens to (docs/wake.md, 「Who reaches the session」; docs/store.md, `allowed_chats`):
//! the rooms the console's room list holds, the owner's private room always among them. A room it listens to wakes
//! its agents and is answered to a chat service as listened; every other room is kept, wakes nobody and is
//! answered as not listened.

use std::sync::Arc;

use bmo_chat_client::{ChatReceiver, RoomQuery};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::{Context, pinned, rows};
use bmo_receiver::roads::added::{self, RoomMode};

use super::{inbound, room};
use crate::integration::common::Scratch;
use crate::integration::common::units::{
  ADDRESS, AGENT, OWNER, StandInChat, Standing, registration, standing, woken,
};

/// A room the owner allowed, one added, one naming a supervisor, one pinned, and one on no list.
const ALLOWED: i64 = -171;
const ADDED: i64 = -172;
const SUPERVISED: i64 = -173;
const PINNED: i64 = -174;
const UNLISTED: i64 = -175;

// A receiver with the one rule every message is judged by, reaching one chat service that holds every room, and the
// default answerer awake.
async fn stood() -> (Scratch, tempfile::TempDir, Standing) {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let chat = Arc::new(StandInChat::new("road-a").holding(&[ALLOWED, ADDED, SUPERVISED, PINNED, UNLISTED]));
  let standing = woken(standing(scratch.db(), DataRoot::new(data.path()), vec![chat]));

  standing.agency.registering(vec![registration(AGENT, "Agent A", vec![ADDRESS], None)]);

  (scratch, data, standing)
}

// What the receiver tells a chat service of a room: whether it listens to it, where it holds anything of it.
async fn told(standing: &Standing, chat_id: i64) -> Option<bool> {
  let query = RoomQuery {
    via: "road-a".to_owned(),
  };

  ChatReceiver::room(&standing.receiver, chat_id, query)
    .await
    .unwrap()
    .map(|room| room.listened)
}

#[tokio::test]
async fn a_room_the_console_lists_wakes_and_is_answered_as_listened() {
  let (scratch, _data, standing) = stood().await;
  let db = scratch.db();

  rows::allow_chat(db, ALLOWED).await.unwrap();
  added::add(db, ADDED, "road-a", RoomMode::Avatar, None).await.unwrap();
  scratch
    .run(
      r#"INSERT INTO "room_supervisors" ("chat_id", "supervisor") VALUES ($1, $2)"#,
      vec![SUPERVISED.into(), AGENT.into()],
    )
    .await;
  pinned::add(db, PINNED).await.unwrap();

  standing
    .receiver
    .message(
      &inbound(ALLOWED, 1, 9501, &format!("{ADDRESS}, sum this up")),
      &Context::default(),
      room(),
    )
    .await
    .unwrap();

  let woken = standing.taken.handed();

  assert_eq!(woken.len(), 1, "{woken:?}");
  assert_eq!((woken[0].chat_id, woken[0].message_id), (ALLOWED, 1));

  for chat_id in [ALLOWED, ADDED, SUPERVISED, PINNED] {
    assert!(standing.receiver.listens(chat_id).await.unwrap(), "{chat_id}");
  }
  assert_eq!(told(&standing, ALLOWED).await, Some(true));
}

#[tokio::test]
async fn a_room_on_no_list_is_kept_wakes_nobody_and_is_answered_as_not_listened() {
  let (scratch, _data, standing) = stood().await;

  standing
    .receiver
    .message(
      &inbound(UNLISTED, 1, 9501, &format!("{ADDRESS}, sum this up")),
      &Context::default(),
      room(),
    )
    .await
    .unwrap();

  assert!(standing.taken.handed().is_empty(), "{:?}", standing.taken.handed());
  assert_eq!(
    scratch
      .texts(&format!(r#"SELECT "text" FROM "messages" WHERE "chat_id" = {UNLISTED}"#))
      .await,
    [format!("{ADDRESS}, sum this up")],
    "the room's words are kept all the same"
  );
  assert!(!standing.receiver.listens(UNLISTED).await.unwrap());
  assert_eq!(told(&standing, UNLISTED).await, Some(false));
}

#[tokio::test]
async fn the_owners_private_room_is_listened_to_with_nothing_on_a_list_naming_it() {
  let (scratch, _data, standing) = stood().await;
  let own = standing.receiver.owner_room();

  assert!(
    scratch.texts(r#"SELECT CAST("chat_id" AS TEXT) FROM "allowed_chats""#).await.is_empty(),
    "the listen list names no room"
  );

  standing
    .receiver
    .message(
      &inbound(own, 1, OWNER, "what is on today"),
      &Context::default(),
      bmo_receiver::receiver::Room {
        owner_room: true,
        ..room()
      },
    )
    .await
    .unwrap();

  let woken = standing.taken.handed();

  assert_eq!(woken.len(), 1, "his words wake the default answerer with no name: {woken:?}");
  assert!(standing.receiver.listens(own).await.unwrap());
  assert_eq!(told(&standing, own).await, Some(true));
}

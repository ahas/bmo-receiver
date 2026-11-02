//! The owner's private room the configuration names (docs/receiver.md, 「Identity, and reaching the owner」;
//! docs/boundary-api.md, `owner-room`): a room on the record alone is the owner's private chat, a chat service's
//! own one-to-one with him is then an ordinary room, the notices go to it where no notices room is named, and the
//! command stands it up once.

use std::sync::Arc;

use tempfile::tempdir;

use bmo_receiver::data_root::DataRoot;
use bmo_receiver::owner_room::stand_up;
use bmo_receiver::store::settings;

use crate::integration::common::Scratch;
use crate::integration::common::units::{AGENT, OWNER, StandInChat, deployment, unrun};

const PRIVATE: i64 = -9_000_000_000_010;

#[tokio::test]
async fn the_configured_room_is_the_owners_private_chat_and_a_services_owner_kind_is_read_for_no_other() {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let road = || vec![Arc::new(StandInChat::new("road-a").holding(&[OWNER]).owners(OWNER))];
  let mut given = deployment();

  given.owner_room = Some(PRIVATE);

  let named = unrun(scratch.db(), DataRoot::new(data.path()), road(), given);
  let receiver = named.receiver.clone();

  assert!(receiver.roads().is_owner_room(PRIVATE).await, "the configured room is the owner's");
  assert!(
    !receiver.roads().is_owner_room(OWNER).await,
    "the service's one-to-one with the owner is an ordinary room"
  );
  assert_eq!(
    (receiver.owner_room(), receiver.owner_id()),
    (PRIVATE, OWNER),
    "the room and the sender stand apart"
  );
  assert_eq!(receiver.notices_room(), PRIVATE, "the notices go to the owner's private room");

  let other = Scratch::new().await;
  let unnamed = unrun(other.db(), DataRoot::new(data.path()), road(), deployment());

  assert!(unnamed.receiver.roads().is_owner_room(OWNER).await, "without one, the service says");
  assert_eq!(unnamed.receiver.owner_room(), OWNER);
}

#[tokio::test]
async fn the_command_stands_the_room_up_once_on_the_record_alone() {
  let scratch = Scratch::new().await;
  let (room, made) = stand_up(scratch.db(), AGENT, "the owner's room").await.unwrap();

  assert!(made && room < 0, "a room numbered below every room: {room}");

  let providers = scratch
    .texts(&format!(r#"SELECT "provider" FROM "room_providers" WHERE "chat_id" = {room}"#))
    .await;
  let answerers = scratch
    .texts(&format!(r#"SELECT "agent" FROM "room_answerers" WHERE "chat_id" = {room}"#))
    .await;
  let title = scratch
    .texts(&format!(r#"SELECT "title" FROM "chat_titles" WHERE "chat_id" = {room}"#))
    .await;
  let numbered = scratch
    .texts(&format!(
      r#"SELECT "sender_id"::text FROM "provider_users" WHERE "provider" = 'record' AND "user_id" = '{AGENT}'"#
    ))
    .await;

  assert_eq!(providers, vec!["record".to_owned()], "the record is its one provider");
  assert_eq!(answerers, vec![AGENT.to_owned()]);
  assert_eq!(title, vec!["the owner's room".to_owned()]);
  assert_eq!(numbered.len(), 1, "the chat agent holds a number of the console's own");
  assert!(numbered[0].parse::<i64>().unwrap() < 0);

  let (again, made) = stand_up(scratch.db(), AGENT, "another title").await.unwrap();

  assert_eq!((again, made), (room, false), "a second run stands up nothing");
  assert_eq!(settings::get(scratch.db(), "owner_room_made").await.unwrap(), Some(room.to_string()));
}

#[tokio::test]
async fn a_room_number_once_given_is_never_given_again_and_stays_below_every_room_the_store_holds() {
  use bmo_receiver::roads::record::next_room;

  let scratch = Scratch::new().await;

  // A room another provider numbered stands only in the messages: the next number goes below it.
  scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via")
         VALUES (-9000000000004, 1, 5, 'someone', 1, 'hello', 'road-a')"#,
      vec![],
    )
    .await;

  let first = next_room(scratch.db()).await.unwrap();

  assert_eq!(first, -9_000_000_000_005);

  // The room given that number is retired, every row of it gone: its number stays retired.
  scratch.run(r#"DELETE FROM "messages""#, vec![]).await;

  assert_eq!(
    next_room(scratch.db()).await.unwrap(),
    first - 1,
    "a retired room's number is never given again"
  );

  // A room another chat service numbered in its own range moves nothing of the record's.
  scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via")
         VALUES (-9001000000000, 1, 5, 'someone', 1, 'hello', 'road-a')"#,
      vec![],
    )
    .await;
  assert_eq!(
    next_room(scratch.db()).await.unwrap(),
    first - 2,
    "the record numbers within its own range"
  );
}

#[tokio::test]
async fn the_records_first_room_is_the_top_of_its_range() {
  let scratch = Scratch::new().await;

  assert_eq!(bmo_receiver::roads::record::next_room(scratch.db()).await.unwrap(), -9_000_000_000_001);
}

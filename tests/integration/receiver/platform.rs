//! What a chat service says happened to a message on its platform (docs/messages.md): its reactions, each listed
//! person's taking what the change gives them and, where the change is the whole set, the rest taken off; and its
//! deletion there, which marks the message and keeps its words.

use std::sync::Arc;

use bmo_chat_client::{ChatReceiver, Deletion, ErrorCode, PersonReactions, ReactionChange};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::Room;

use super::inbound;
use crate::integration::common::Scratch;
use crate::integration::common::units::{StandInChat, Standing, standing};

const ROOM: i64 = -121;

async fn stood() -> (Scratch, tempfile::TempDir, Standing) {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let chat = Arc::new(StandInChat::new("road-a").holding(&[ROOM]));
  let standing = standing(scratch.db(), DataRoot::new(data.path()), vec![chat]);

  (scratch, data, standing)
}

async fn said(standing: &Standing, message_id: i64, text: &str) {
  standing
    .receiver
    .message(&inbound(ROOM, message_id, 55, text), &Default::default(), Room::default())
    .await
    .unwrap();
}

fn change(message_id: i64, reactions: &[(Option<i64>, &[&str])], whole: bool) -> ReactionChange {
  ReactionChange {
    via: "road-a".into(),
    room_id: ROOM,
    message_id,
    stamp: None,
    reactions: reactions
      .iter()
      .map(|(person, emojis)| PersonReactions {
        person_id: *person,
        emojis: emojis.iter().map(|emoji| (*emoji).to_owned()).collect(),
      })
      .collect(),
    whole,
  }
}

async fn reactions(scratch: &Scratch, message_id: i64) -> Vec<String> {
  scratch
    .texts(&format!(
      r#"SELECT "by" || ' ' || "emoji" FROM "message_reactions" WHERE "chat_id" = {ROOM} AND "message_id" = {message_id} ORDER BY "by""#
    ))
    .await
}

#[tokio::test]
async fn a_reaction_change_sets_each_listed_persons_reaction_and_a_whole_one_takes_the_rest_off() {
  let (scratch, _data, standing) = stood().await;

  said(&standing, 1, "words").await;
  standing
    .receiver
    .reactions_changed(change(1, &[(Some(7), &["👍"]), (Some(8), &["❤"])], false))
    .await
    .unwrap();

  assert_eq!(reactions(&scratch, 1).await, ["7 👍", "8 ❤"]);

  // A person taking theirs off, and the account itself reacting under the provider's key.
  standing
    .receiver
    .reactions_changed(change(1, &[(Some(8), &[]), (None, &["🔥"])], false))
    .await
    .unwrap();

  assert_eq!(reactions(&scratch, 1).await, ["7 👍", "road-a 🔥"]);

  // The whole set: whoever it does not list has none.
  standing
    .receiver
    .reactions_changed(change(1, &[(Some(9), &["😂"])], true))
    .await
    .unwrap();

  assert_eq!(reactions(&scratch, 1).await, ["9 😂"]);

  let refused = standing
    .receiver
    .reactions_changed(change(99, &[(Some(7), &["👍"])], false))
    .await
    .unwrap_err();

  assert_eq!(refused.code, ErrorCode::NotFound);
}

#[tokio::test]
async fn a_message_deleted_on_the_platform_is_marked_and_keeps_its_words() {
  let (scratch, _data, standing) = stood().await;

  said(&standing, 2, "the words that went").await;
  standing
    .receiver
    .deleted(Deletion {
      via: "road-a".into(),
      room_id: ROOM,
      message_ids: vec![2, 404],
      stamps: vec![],
    })
    .await
    .unwrap();

  assert_eq!(
    scratch
      .texts(&format!(
        r#"SELECT "text" || ' ' || ("deleted_at" IS NOT NULL)::text FROM "messages" WHERE "chat_id" = {ROOM} AND "message_id" = 2"#
      ))
      .await,
    ["the words that went true"]
  );
}

#[tokio::test]
async fn a_message_known_by_another_number_is_found_by_who_sent_it_and_when() {
  let (scratch, _data, standing) = stood().await;
  let sent = inbound(ROOM, 3, 55, "said once");

  said(&standing, 3, "said once").await;
  standing
    .receiver
    .deleted(Deletion {
      via: "road-a".into(),
      room_id: ROOM,
      message_ids: vec![],
      stamps: vec![bmo_chat_client::MessageStamp {
        sender_id: 55,
        sent_at: sent.sent_at,
      }],
    })
    .await
    .unwrap();

  assert_eq!(
    scratch
      .texts(&format!(
        r#"SELECT ("deleted_at" IS NOT NULL)::text FROM "messages" WHERE "chat_id" = {ROOM} AND "message_id" = 3"#
      ))
      .await,
    ["true"]
  );
}

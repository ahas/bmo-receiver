//! A person's own thread: what of their rooms is carried into the prompt of their private one, and what is
//! kept out of it — the words of a companion they share a room with, and a member who is not them. The account
//! model stands in force, so the thread is followed (`docs/store.md`, the `private_thread` setting).

use bmo_receiver::chat::thread::thread_entry;
use bmo_receiver::data_root::DataRoot;

use std::sync::Arc;

use crate::common::Scratch;
use crate::common::units::{ADDRESS, AGENT, OWNER, SENDER, StandInChat, Standing, registration, standing};

const MEMBER: i64 = 4344;
const COMPANION_SENDER: i64 = 4345;
// The person's own private chat; a room naming nobody, so the chat agent's; one only the companion answers;
// one both answer.
const PRIVATE: i64 = -4300;
const SHARED: i64 = -4301;
const COMPANIONS: i64 = -4302;
const MIXED: i64 = -4303;
// The companion these rooms share the person with.
const COMPANION: &str = "agent-b";

async fn stood(rooms: &[i64]) -> (Scratch, tempfile::TempDir, Standing) {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let chat = Arc::new(StandInChat::new("road-a").holding(rooms).privating(PRIVATE, OWNER));

  let stood = standing(scratch.db(), DataRoot::new(data.path()), vec![chat]);

  (scratch, data, stood)
}

#[tokio::test]
async fn the_thread_carries_only_what_the_person_and_the_chat_agent_said_in_rooms_the_agent_answers() {
  let (scratch, _data, stood) = stood(&[SHARED, COMPANIONS, MIXED]).await;

  stood.agency.registering(
    [
      registration(AGENT, "Agent A", vec![ADDRESS], None, true),
      registration(COMPANION, "Agent B", vec![], None, false),
    ]
    .to_vec(),
  );
  stood.web.modelling();
  stood.web.linking(OWNER, "tester".into());
  scratch
    .run(
      r#"INSERT INTO "room_answerers" ("chat_id", "agent") VALUES ($1, 'agent-b'), ($2, 'agent-b'), ($2, $3)"#,
      vec![COMPANIONS.into(), MIXED.into(), AGENT.into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "provider_users" ("provider", "user_id", "sender_id") VALUES ('road-a', $1, $2)"#,
      vec![COMPANION.into(), COMPANION_SENDER.into()],
    )
    .await;

  let at = bmo_receiver::now_ms() / 1000 - 600;

  // In the room both answer, what the person says to the companion, by its name, by a reply to its line or as
  // the words the companion answers first, stays out with its answers.
  for (room, id, sender, name, reply, text) in [
    (SHARED, 1, OWNER, "tester", 0, "shared question"),
    (SHARED, 2, 0, SENDER, 0, "shared answer"),
    (SHARED, 3, MEMBER, "member", 0, "another member speaks"),
    (COMPANIONS, 1, OWNER, "tester", 0, "words for the companion"),
    (COMPANIONS, 2, COMPANION_SENDER, "Agent B", 0, "the companion answers"),
    (MIXED, 1, OWNER, "tester", 0, "mixed question"),
    (MIXED, 2, 0, SENDER, 0, "mixed answer"),
    (MIXED, 3, OWNER, "tester", 0, "Agent B, a call to the companion"),
    (MIXED, 4, COMPANION_SENDER, "Agent B", 0, "the companion answers the call"),
    (MIXED, 5, OWNER, "tester", 0, "talk going on with the companion"),
    (MIXED, 6, COMPANION_SENDER, "Agent B", 0, "the companion answers again"),
    (MIXED, 7, OWNER, "tester", 6, "a reply to the companion"),
    (MIXED, 8, 0, SENDER, 7, "a word from the side"),
    (MIXED, 9, OWNER, "tester", 0, "Agent A, a call to the chat agent"),
    (MIXED, 10, 0, SENDER, 0, "the chat agent answers the call"),
    (MIXED, 11, OWNER, "tester", 10, "a reply to the chat agent"),
    (MIXED, 12, 0, SENDER, 11, "the chat agent answers the reply"),
  ] {
    scratch
      .run(
        r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "reply_to_message_id")
           VALUES ($1, $2, NULLIF($3, 0), $4, $5, $6, NULLIF($7, 0))"#,
        vec![
          room.into(),
          id.into(),
          sender.into(),
          name.into(),
          (at + id).into(),
          text.into(),
          reply.into(),
        ],
      )
      .await;
  }

  let thread = thread_entry(&stood.receiver, PRIVATE).await.unwrap().unwrap();
  let carried = thread.text;

  for kept in [
    "shared question",
    "shared answer",
    "mixed question",
    "mixed answer",
    "a call to the chat agent",
    "the chat agent answers the call",
    "a reply to the chat agent",
    "the chat agent answers the reply",
  ] {
    assert!(carried.contains(kept), "{kept} missing from {carried}");
  }

  for left in [
    "another member speaks",
    "words for the companion",
    "the companion answers",
    "a call to the companion",
    "talk going on with the companion",
    "a reply to the companion",
    "a word from the side",
  ] {
    assert!(!carried.contains(left), "{left} carried into {carried}");
  }
}

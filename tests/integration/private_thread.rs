//! A person's own thread: what of their rooms is carried into the prompt of their private one, and what is
//! kept out of it — the words of another agent they share a room with, and a member who is not them. The account
//! model stands in force, so the thread is followed (`docs/store.md`, the `private_thread` setting).

use bmo_receiver::chat::thread::{remember_mark, thread_entry};
use bmo_receiver::data_root::DataRoot;

use std::sync::Arc;

use crate::integration::common::Scratch;
use crate::integration::common::units::{
  ADDRESS, AGENT, OWNER, SENDER, StandInChat, Standing, registration, standing,
};

const MEMBER: i64 = 4344;
const OTHER_SENDER: i64 = 4345;
// The person's own private chat; a room naming nobody, so the chat agent's; one only the other agent answers;
// one both answer.
const PRIVATE: i64 = -4300;
const SHARED: i64 = -4301;
const OTHERS: i64 = -4302;
const MIXED: i64 = -4303;
// The other agent these rooms share the person with.
const OTHER: &str = "agent-b";

async fn stood(rooms: &[i64]) -> (Scratch, tempfile::TempDir, Standing) {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let chat = Arc::new(StandInChat::new("road-a").holding(rooms).privating(PRIVATE, OWNER));

  let stood = standing(scratch.db(), DataRoot::new(data.path()), vec![chat]);

  (scratch, data, stood)
}

#[tokio::test]
async fn the_thread_carries_only_what_the_person_and_the_chat_agent_said_in_rooms_the_agent_answers() {
  let (scratch, _data, stood) = stood(&[SHARED, OTHERS, MIXED]).await;

  stood.agency.registering(
    [
      registration(AGENT, "Agent A", vec![ADDRESS], None, true),
      registration(OTHER, "Agent B", vec![], None, false),
    ]
    .to_vec(),
  );
  stood.web.modelling();
  stood.web.linking(OWNER, "tester".into());
  scratch
    .run(
      r#"INSERT INTO "room_answerers" ("chat_id", "agent") VALUES ($1, 'agent-b'), ($2, 'agent-b'), ($2, $3)"#,
      vec![OTHERS.into(), MIXED.into(), AGENT.into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "provider_users" ("provider", "user_id", "sender_id") VALUES ('road-a', $1, $2)"#,
      vec![OTHER.into(), OTHER_SENDER.into()],
    )
    .await;

  let at = bmo_receiver::now_ms() - 600_000;

  // In the room both answer, what the person says to the other agent, by its name, by a reply to its line or as
  // the words the other agent answers first, stays out with its answers.
  for (room, id, sender, name, reply, text) in [
    (SHARED, 1, OWNER, "tester", 0, "shared question"),
    (SHARED, 2, 0, SENDER, 0, "shared answer"),
    (SHARED, 3, MEMBER, "member", 0, "another member speaks"),
    (OTHERS, 1, OWNER, "tester", 0, "words for the other agent"),
    (OTHERS, 2, OTHER_SENDER, "Agent B", 0, "the other agent answers"),
    (MIXED, 1, OWNER, "tester", 0, "mixed question"),
    (MIXED, 2, 0, SENDER, 0, "mixed answer"),
    (MIXED, 3, OWNER, "tester", 0, "Agent B, a call to the other agent"),
    (MIXED, 4, OTHER_SENDER, "Agent B", 0, "the other agent answers the call"),
    (MIXED, 5, OWNER, "tester", 0, "talk going on with the other agent"),
    (MIXED, 6, OTHER_SENDER, "Agent B", 0, "the other agent answers again"),
    (MIXED, 7, OWNER, "tester", 6, "a reply to the other agent"),
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
  let carried = thread.entry.unwrap().text;

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
    "words for the other agent",
    "the other agent answers",
    "a call to the other agent",
    "talk going on with the other agent",
    "a reply to the other agent",
    "a word from the side",
  ] {
    assert!(!carried.contains(left), "{left} carried into {carried}");
  }

  // The turn that handed it writes the thread's mark.
  let path = thread.path.clone();

  remember_mark(stood.receiver.db(), thread.mark.unwrap()).await.unwrap();

  // A restart keeps the thread's mark: the next turn is still handed growth alone.
  bmo_receiver::service_start::settle(&stood.receiver, "test-consumer", "127.0.0.1:1", "token").await;

  // A later line is handed as growth alone, not with the thread it follows.
  for (id, sender, name, text) in [(20, OWNER, "tester", "a later word"), (21, 0, SENDER, "a later answer")]
  {
    scratch
      .run(
        r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text")
           VALUES ($1, $2, NULLIF($3, 0), $4, $5, $6)"#,
        vec![
          SHARED.into(),
          id.into(),
          sender.into(),
          name.into(),
          (at + 1_000 + id).into(),
          text.into(),
        ],
      )
      .await;
  }

  let grown = thread_entry(&stood.receiver, PRIVATE).await.unwrap().unwrap();
  let grown_mark = grown.mark.unwrap();
  let grown = grown.entry.unwrap().text;

  assert!(grown.contains("since this person's thread was last handed"), "{grown}");
  assert!(grown.contains("a later word") && grown.contains("a later answer"), "{grown}");
  assert!(!grown.contains("mixed question"), "{grown}");
  assert!(!grown.contains("shared question"), "{grown}");
  remember_mark(stood.receiver.db(), grown_mark).await.unwrap();

  // With nothing new, no block goes and the piece stands held.
  let still = thread_entry(&stood.receiver, PRIVATE).await.unwrap().unwrap();

  assert!(still.entry.is_none());
  assert!(still.path.starts_with("threads/user-"), "{}", still.path);
  assert!(still.holds.contains("this person's thread"), "{}", still.holds);
  assert!(still.mark.is_none());

  // The window hands that held piece to the round's log as held, with no block of its own.
  let selection = bmo_receiver::chat::window::select(
    &stood.receiver,
    &bmo_receiver::chat::window::Material {
      agent: AGENT,
      chat_id: PRIVATE,
      room: bmo_receiver::receiver::Room {
        owner_room: false,
        identity: None,
        private_to: Some(OWNER),
      },
      messages: &[],
      instructions: &[],
      said: &[],
      events: &[],
      recent: &[],
      holds: None,
      round: None,
      session: Some("session-a"),
    },
  )
  .await
  .unwrap();

  assert!(
    selection.held.iter().any(|(path, holds)| path.starts_with("threads/user-")
      && holds.as_deref().is_some_and(|holds| holds.contains("this person's thread"))),
    "{:?}",
    selection.held
  );
  assert!(selection.thread_mark.is_none());

  let reads = bmo_receiver::chat::draft::held_reads(&selection.held);

  assert!(
    reads.iter().any(|read| read.path.starts_with("threads/user-") && read.how == "held"),
    "{reads:?}"
  );

  // A session that folds keeps the thread's mark: nothing is handed again to the next session.
  bmo_receiver::chat::context::forget_documents(&stood.receiver, Some("session-a"))
    .await
    .unwrap();

  let folded = thread_entry(&stood.receiver, PRIVATE).await.unwrap().unwrap();

  assert!(folded.entry.is_none(), "the thread's mark stands after a fold: {folded:?}");

  // Only a thread with no mark is handed whole, within its days.
  bmo_receiver::store::settings::forget(stood.receiver.db(), &format!("thread_mark:{path}"))
    .await
    .unwrap();

  let fresh = thread_entry(&stood.receiver, PRIVATE).await.unwrap().unwrap();
  let fresh = fresh.entry.unwrap().text;

  assert!(fresh.contains("over the last seven days"), "{fresh}");
  assert!(fresh.contains("mixed question") && fresh.contains("a later word"), "{fresh}");
  assert!(!fresh.contains("since this person's thread was last handed"), "{fresh}");
}

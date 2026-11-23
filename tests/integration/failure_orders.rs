//! The order a broken chat turn raises (docs/voiced-notices.md, 「What each path says」): for the default answerer, in
//! the room the turn broke in where it answers that room, else in the owner's private room naming the agent and the
//! room, under a kind of that room's own, and never with the room's words.

use tempfile::tempdir;

use bmo_receiver::chat::health::raise_order;
use bmo_receiver::data_root::DataRoot;

use crate::integration::common::Scratch;
use crate::integration::common::units::{AGENT, OWNER, deployment, registration, unrun};

// A person's room their own supervisor answers, which the default answerer does not.
const PERSONS_ROOM: i64 = -9_000_000_000_124;
const PERSONS_AGENT: &str = "agent-b";
// A room nobody names, which the default answerer answers.
const OPEN_ROOM: i64 = 7;

// The failure orders standing: each its room, kind, agent, text and summary.
async fn orders(scratch: &Scratch) -> Vec<String> {
  scratch
    .texts(
      r#"SELECT "chat_id"::text || ' | ' || "kind" || ' | ' || "agent" || ' | ' || "text" || ' | ' || COALESCE("summary", '')
         FROM "instructions" WHERE "kind" LIKE 'failure-order%' ORDER BY "id""#,
    )
    .await
}

#[tokio::test]
async fn a_turn_broken_in_a_room_the_default_answerer_does_not_answer_is_ordered_in_the_owners_room() {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let standing = unrun(scratch.db(), DataRoot::new(data.path()), vec![], deployment());
  let receiver = standing.receiver.clone();

  standing.agency.registering(vec![
    registration(AGENT, "Agent A", vec!["a"], None),
    registration(PERSONS_AGENT, PERSONS_AGENT, vec![PERSONS_AGENT], None),
  ]);
  scratch
    .run(
      r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES ($1, 'a person''s room')"#,
      vec![PERSONS_ROOM.into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "room_supervisors" ("chat_id", "supervisor") VALUES ($1, $2)"#,
      vec![PERSONS_ROOM.into(), PERSONS_AGENT.into()],
    )
    .await;

  raise_order(&receiver, AGENT, PERSONS_AGENT, PERSONS_ROOM, Some("no supply is left for this run"))
    .await
    .unwrap();
  raise_order(&receiver, AGENT, AGENT, OPEN_ROOM, Some("the engine failed"))
    .await
    .unwrap();

  assert_eq!(
    orders(&scratch).await,
    [
      format!(
        "{OWNER} | failure-order:{PERSONS_ROOM} | {AGENT} | a chat turn of {PERSONS_AGENT} in a person's room | no \
         supply is left for this run"
      ),
      format!(
        "{OPEN_ROOM} | failure-order | {AGENT} | a chat turn of your own in {OPEN_ROOM} | the engine failed"
      ),
    ],
    "the person's room holds no order the default answerer is never handed; the owner's room names it instead",
  );

  // A second such room's order stands beside the first in the owner's room rather than replacing it.
  scratch
    .run(
      r#"INSERT INTO "room_supervisors" ("chat_id", "supervisor") VALUES ($1, $2)"#,
      vec![(PERSONS_ROOM - 1).into(), PERSONS_AGENT.into()],
    )
    .await;
  raise_order(&receiver, AGENT, PERSONS_AGENT, PERSONS_ROOM - 1, None).await.unwrap();
  assert_eq!(
    orders(&scratch)
      .await
      .iter()
      .filter(|order| order.starts_with(&format!("{OWNER} |")))
      .count(),
    2
  );
}

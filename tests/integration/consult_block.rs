//! What the receiver makes of another agent's own work: the block of the others' work it shows her where she
//! answers in the consult room (docs/consult.md, 「What an agent is shown」). The tools she files and keeps
//! things with are tested in their own areas.

use crate::common::Scratch;
use crate::common::units::standing;
use bmo_receiver::chat::consult::{consult_block, window_states};
use bmo_receiver::data_root::DataRoot;
use receiver_ports::agency::Agent;
use receiver_ports::job::ItemRow;
use serde_json::{Value, json};
use tempfile::tempdir;

const ROOM: i64 = -121;
const OTHER: &str = "agent-d";
const WORKER: &str = "agent-d-worker";

// Another agent that declares a worker of its own, beside the chat agent that declares none.
fn registered() -> Vec<Agent> {
  let row = |worker: &str| {
    let mut held = serde_json::Map::<String, Value>::new();

    held.insert("defaultAnswerer".to_owned(), json!(false));
    held.insert("windowSeconds".to_owned(), json!(60));
    held.insert(
      "itemWorker".to_owned(),
      worker.is_empty().then_some(json!(null)).unwrap_or(json!(worker)),
    );

    held
  };

  [
    Agent {
      name: "chat-a".into(),
      label: Some("Chat A".into()),
      awake: true,
      brain_folder: None,
      row: row(""),
    },
    Agent {
      name: OTHER.into(),
      label: Some("Agent D".into()),
      awake: true,
      brain_folder: None,
      row: row(WORKER),
    },
  ]
  .to_vec()
}

#[tokio::test]
async fn an_agents_consult_block_shows_the_others_work_and_not_her_own() {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let standing = standing(scratch.db(), DataRoot::new(data.path()), vec![]);

  standing.agency.registering(registered());
  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value") VALUES ($1, $2)"#,
      vec!["consult_room".into(), ROOM.to_string().into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "room_answerers" ("chat_id", "agent") VALUES ($1, $2), ($1, $3)"#,
      vec![ROOM.into(), OTHER.into(), "chat-a".into()],
    )
    .await;
  standing.job.filing(
    [
      ItemRow {
        id: 900,
        approved: true,
        status: "waiting".into(),
        title: "work of the chat agent".into(),
        worker: None,
      },
      ItemRow {
        id: 901,
        approved: true,
        status: "waiting".into(),
        title: "catnip order".into(),
        worker: Some(WORKER.into()),
      },
    ]
    .to_vec(),
  );

  let block = consult_block(&standing.receiver, OTHER, 1_790_000_100_000).await.unwrap().unwrap();

  assert!(block.contains("#900"), "{block}");
  assert!(!block.contains("- #901 "), "{block}");
}

#[tokio::test]
async fn the_windows_of_a_room_more_than_one_agent_answers_stand_in_the_block() {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let standing = standing(scratch.db(), DataRoot::new(data.path()), vec![]);

  standing.agency.registering(registered());
  scratch
    .run(
      r#"INSERT INTO "room_answerers" ("chat_id", "agent") VALUES ($1, $2), ($1, $3)"#,
      vec![ROOM.into(), OTHER.into(), "chat-a".into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "room_windows" ("chat_id", "agent", "open_until") VALUES ($1, $2, $3)"#,
      vec![ROOM.into(), "chat-a".into(), 1_790_000_160_000_i64.into()],
    )
    .await;

  let states = window_states(&standing.receiver, ROOM).await;

  assert_eq!(states.len(), 2);
  assert!(states.iter().any(|state| state.agent == OTHER && state.open_until.unwrap_or(0) == 0));
  assert!(
    states
      .iter()
      .any(|state| state.agent == "chat-a" && state.open_until == Some(1_790_000_160_000))
  );
}

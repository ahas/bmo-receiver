//! The consult verbs over a scratch archive (docs/consult.md, 「The consult tool」): the consult room read or
//! set by number or title, refused where fewer than two agents answer there, and the block an agent outside it
//! is shown.

use crate::common::Scratch;
use crate::common::units::{Standing, standing};
use bmo_receiver::companion::consult::window_states;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::verbs::consult::{room, state};
use receiver_policy::consult::CONSULT;
use receiver_ports::agency::Agent;
use tempfile::tempdir;

const SHARED: i64 = -121;
const ALONE: i64 = -122;

fn window(seconds: i64) -> serde_json::Map<String, serde_json::Value> {
  let mut held = serde_json::Map::<String, serde_json::Value>::new();

  held.insert("windowSeconds".to_owned(), serde_json::json!(seconds));

  held
}

// Two companions answering together in one titled room, and one alone in another.
async fn rooms() -> (Scratch, tempfile::TempDir, Standing) {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let standing = standing(scratch.db(), DataRoot::new(data.path()), vec![]);

  standing.agency.registering(
    [
      Agent {
        name: "agent-a".into(),
        label: Some("Agent A".into()),
        awake: true,
        brain_folder: None,
        row: window(60),
      },
      Agent {
        name: "agent-b".into(),
        label: Some("Agent B".into()),
        awake: true,
        brain_folder: None,
        row: window(60),
      },
    ]
    .to_vec(),
  );
  scratch
    .run(
      r#"INSERT INTO "room_answerers" ("chat_id", "agent") VALUES ($1, $2), ($1, $3), ($4, $2)"#,
      vec![SHARED.into(), "agent-a".into(), "agent-b".into(), ALONE.into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES ($1, $2), ($3, $4)"#,
      vec![SHARED.into(), "Lab".into(), ALONE.into(), "Alone".into()],
    )
    .await;

  (scratch, data, standing)
}

#[tokio::test]
async fn the_consult_room_is_set_by_title_to_a_room_two_agents_answer_and_refused_elsewhere() {
  let (_scratch, _data, standing) = rooms().await;
  let receiver = &standing.receiver;
  let alone = ALONE.to_string();
  let shared = SHARED.to_string();

  assert_eq!(room(receiver, None).await.unwrap(), vec![CONSULT.room_unset.clone()]);
  assert_eq!(
    room(receiver, Some(&alone)).await.unwrap_err(),
    CONSULT.not_shared.replace("{room}", &alone)
  );
  assert_eq!(
    room(receiver, Some("Nowhere")).await.unwrap_err(),
    CONSULT.not_answered.replace("{room}", "Nowhere")
  );
  assert_eq!(
    room(receiver, Some(" Lab ")).await.unwrap(),
    vec![CONSULT.room_chosen.replace("{title}", "Lab").replace("{chat}", &shared)]
  );
  assert_eq!(
    room(receiver, None).await.unwrap(),
    vec![CONSULT.room_shown.replace("{title}", "Lab").replace("{chat}", &shared)]
  );
}

#[tokio::test]
async fn an_agent_that_does_not_answer_in_the_consult_room_is_shown_nothing() {
  let (_scratch, _data, standing) = rooms().await;
  let receiver = &standing.receiver;

  room(receiver, Some("Lab")).await.unwrap();

  assert_eq!(
    state(receiver, "agent-c").await,
    vec![CONSULT.nothing_shown.replace("{agent}", "agent-c")]
  );
  assert!(state(receiver, "agent-a").await[0].contains("Agent B"));
  assert_eq!(window_states(receiver, SHARED).await.len(), 2);
}

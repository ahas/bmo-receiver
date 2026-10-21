//! A companion added on the console and removed there (docs/boundary-api.md, 「Companions with a room of their
//! own」): the agency writes the agent's half and the gate copies the chat agent's options before the receiver
//! writes the room half; the companion's name goes to the agency's removal; and a field the agency refuses comes
//! back as the receiver's code with nothing written.

use axum::http::StatusCode;
use serde_json::{Value, json};

use receiver_ports::agency::{Agent, FieldRefusal};

use crate::common::Scratch;
use crate::common::queue::{Front, post, remove, stood};
use crate::common::units::{AGENT, OWNER};

const COMPANION: &str = "companion-a";
const WORKER: &str = "worker-a";

// The front, with the agency registering one worker a companion's items may go to.
async fn bench(scratch: &Scratch) -> Front {
  let front = stood(scratch, OWNER).await;
  let mut row = serde_json::Map::<String, Value>::new();

  row.insert("kind".to_owned(), json!("worker"));
  front.standing.agency.registering(vec![Agent {
    name: WORKER.to_owned(),
    label: Some("Worker A".to_owned()),
    awake: true,
    brain_folder: None,
    row,
  }]);

  front
}

fn added() -> Value {
  json!({
    "name": COMPANION,
    "label": "Companion A",
    "provider": "provider-a",
    "model": "model-a",
    "worker": WORKER,
    "room": "A room of its own",
  })
}

async fn answerers(scratch: &Scratch) -> i64 {
  scratch
    .count(r#"SELECT COUNT(*) FROM "room_answerers" WHERE "agent" = $1"#, vec![COMPANION.into()])
    .await
}

#[tokio::test]
async fn a_companion_stands_with_its_gate_options_copied_and_its_name_goes_to_the_agencys_removal() {
  let scratch = Scratch::new().await;
  let front = bench(&scratch).await;
  let (status, answer) = post(&front, "/companions", added()).await;

  assert_eq!(status, StatusCode::OK, "{answer}");

  assert_eq!(
    *front.standing.gate.copied.lock().unwrap(),
    [(COMPANION.to_owned(), AGENT.to_owned())],
    "the gate copies the chat agent's options for the companion"
  );
  assert_eq!(answerers(&scratch).await, 1);

  let (status, answer) = remove(&front, &format!("/companions/{COMPANION}?by=tester")).await;

  assert_eq!(status, StatusCode::OK, "{answer}");
  assert_eq!(
    *front.standing.agency.removals.lock().unwrap(),
    [COMPANION.to_owned()],
    "the companion is removed by the name the agency registers it under"
  );
  assert_eq!(answerers(&scratch).await, 0);
}

#[tokio::test]
async fn a_field_the_agency_refuses_comes_back_as_the_receivers_code_and_nothing_is_written() {
  let scratch = Scratch::new().await;
  let front = bench(&scratch).await;

  *front.standing.agency.refusing.lock().unwrap() = Some(FieldRefusal {
    field: "provider".to_owned(),
    error: "The provider carries no companion.".to_owned(),
  });

  let (status, answer) = post(&front, "/companions", added()).await;

  assert_eq!((status, answer["code"].clone()), (StatusCode::BAD_REQUEST, json!("provider")));
  assert_eq!(answer["error"], "the provider provider-a cannot carry a companion");
  assert_eq!(answerers(&scratch).await, 0);
  assert_eq!(
    scratch
      .count(r#"SELECT COUNT(*) FROM "provider_rooms" WHERE "room_id" = $1"#, vec![COMPANION.into()])
      .await,
    0
  );
  assert!(
    front.standing.gate.copied.lock().unwrap().is_empty(),
    "no options are copied for a refused row"
  );
}

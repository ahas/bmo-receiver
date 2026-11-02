//! An agent's reset point as a send or a consult line is judged by it (docs/wake.md, 「An agent's reset」): the
//! agency's point, read afresh off the agent's supervisor and kept, overtakes a turn even where no cut-off was
//! heard; the later of it and the kept point stands; and the kept point alone stands where the agency does not
//! answer.

use bmo_receiver::chat::cut_off;
use room_calls::sends::Asker;
use serde_json::json;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{Front, stood};
use crate::integration::common::units::{AGENT, OWNER, registration};

const POINT: i64 = 1_700_000_000_000;

fn asked_at(began: i64) -> Asker {
  Asker {
    agent_name: Some(AGENT.to_owned()),
    began: Some(began),
    ..Asker::default()
  }
}

// The front, with the agency naming the agent's reset point on its supervisor, or none.
async fn naming(scratch: &Scratch, point: Option<i64>) -> Front {
  let front = stood(scratch, OWNER).await;
  let mut agent = registration(AGENT, "Agent A", vec![], None, true);

  agent.row.insert("resetPoint".to_owned(), json!(point));
  front.standing.agency.registering(vec![agent]);

  front
}

#[tokio::test]
async fn the_agencys_point_overtakes_a_turn_no_cut_off_was_heard_for_and_is_kept() {
  let scratch = Scratch::new().await;
  let front = naming(&scratch, Some(POINT)).await;
  let receiver = &front.standing.receiver;

  assert!(
    cut_off::overtaken(receiver, &asked_at(POINT)).await.is_some(),
    "a turn begun at the point is overtaken though the receiver heard no cut-off"
  );
  assert!(cut_off::overtaken(receiver, &asked_at(POINT - 1)).await.is_some());
  assert_eq!(
    cut_off::overtaken(receiver, &asked_at(POINT + 1)).await,
    None,
    "a turn begun after it goes on"
  );
  assert_eq!(
    cut_off::point(receiver.db(), AGENT).await.unwrap(),
    Some(POINT),
    "the point read is kept"
  );
  assert_eq!(
    *front.standing.agency.supervisors_read.lock().unwrap(),
    [AGENT, AGENT, AGENT],
    "each ask reads the supervisor afresh"
  );
}

#[tokio::test]
async fn the_later_of_the_agencys_point_and_the_kept_one_stands() {
  let scratch = Scratch::new().await;
  let front = naming(&scratch, Some(POINT)).await;
  let receiver = &front.standing.receiver;

  cut_off::keep_point(receiver.db(), AGENT, POINT + 50).await.unwrap();

  assert_eq!(cut_off::reset_point(receiver, AGENT).await, Some(POINT + 50));
  assert!(cut_off::overtaken(receiver, &asked_at(POINT + 20)).await.is_some());

  let front = naming(&scratch, Some(POINT + 90)).await;

  assert_eq!(cut_off::reset_point(&front.standing.receiver, AGENT).await, Some(POINT + 90));
  assert_eq!(
    cut_off::point(front.standing.receiver.db(), AGENT).await.unwrap(),
    Some(POINT + 90),
    "a later point read replaces the one kept"
  );
}

#[tokio::test]
async fn the_kept_point_stands_alone_where_the_agency_does_not_answer_or_names_none() {
  let scratch = Scratch::new().await;
  let front = naming(&scratch, None).await;
  let receiver = &front.standing.receiver;

  assert_eq!(cut_off::reset_point(receiver, AGENT).await, None, "an agent never reset has no point");

  cut_off::keep_point(receiver.db(), AGENT, POINT).await.unwrap();

  assert_eq!(cut_off::reset_point(receiver, AGENT).await, Some(POINT));

  front.standing.agency.going_down();

  assert!(
    cut_off::overtaken(receiver, &asked_at(POINT)).await.is_some(),
    "the kept point judges the turn while the agency does not answer"
  );
  assert_eq!(cut_off::overtaken(receiver, &asked_at(POINT + 1)).await, None);
}

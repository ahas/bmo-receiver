//! The bmo-job port against a stand-in of bmo-job's routes (bmo-job's `docs/api.md`, "A room's holdings").

use receiver_ports::job::{HttpJob, Job, RoomHolding};
use serde_json::json;

use crate::stand_in::{StandIn, call};

#[tokio::test]
async fn a_rooms_holdings_are_read_by_their_codes() {
  let unit = StandIn::start().await;

  unit.answer(
    "GET /rooms/-7/holdings",
    200,
    json!([{ "code": "room_open_items", "count": 2 }, { "code": "room_waiting_questions", "count": 1 }]),
  );

  let holdings = HttpJob::new(&unit.peer("t")).holdings(-7).await.unwrap();

  assert_eq!(
    holdings,
    [RoomHolding::RoomOpenItems { count: 2 }, RoomHolding::RoomWaitingQuestions { count: 1 }]
  );
  assert_eq!(unit.took(), [call("GET /rooms/-7/holdings", "", None)]);
}

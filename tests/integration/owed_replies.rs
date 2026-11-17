//! A room reply is an obligation the receiver keeps until a round has taken it (docs/wake.md, 「Quiet instructions」
//! and 「What the agency's events wake」; docs/bmo-window.md, 「Quiet instruction」): a row is settled only once a turn
//! carried it, a failed round gives back every row it took and counts only a failure the agency answered, no row is
//! ever deleted, rows handed to no conversation that stands are made pending again as the agency's stream opens and
//! handed by the next pass, and the stream resumes after the last event heard.

use std::sync::{Arc, Mutex};

use bmo_receiver::chat::instructions::{
  Cause, Queued, Rooms, carried_by, deliver_pending, give_back, queue, reconcile, requeue_interrupted,
  settle,
};
use bmo_receiver::hearing::{HeardSetting, Hearer, Hearing, Mark, Marks};
use bmo_receiver::now_ms;
use futures_util::future::BoxFuture;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_ports::agency::Event;
use receiver_ports::gate::TurnEvent;
use serde_json::json;

use crate::integration::common::Scratch;
use crate::integration::common::units::{
  AGENT, OWNER, StandInAgency, StandInChat, deployment, registration, unrun, woken,
};
use bmo_receiver::data_root::DataRoot;

/// Rooms that record each row handed to them and stand a conversation only where a test says one stands.
#[derive(Default)]
struct Recorded {
  handed: Mutex<Vec<i64>>,
  active: Mutex<Vec<i64>>,
}

impl Rooms for Recorded {
  fn instruct(
    &self,
    _agent: &str,
    _chat_id: i64,
    _text: String,
    id: i64,
    _via: Option<String>,
    _events: Vec<TurnEvent>,
  ) -> BoxFuture<'_, ()> {
    self.handed.lock().unwrap().push(id);
    Box::pin(async {})
  }

  fn is_active(&self, _agent: &str, chat_id: i64) -> bool {
    self.active.lock().unwrap().contains(&chat_id)
  }

  fn standing(&self, _agent: &str) -> bool {
    true
  }

  fn road_rests(&self, _agent: &str) -> BoxFuture<'_, bool> {
    Box::pin(async { false })
  }

  fn busy(&self, _agent: &str) -> bool {
    false
  }
}

// The state a row stands in: whether it is handed, whether it is settled, how often its text reached a turn that
// did not end answered, how often a round the agency answered failed with it, and why it was let go.
async fn stood(scratch: &Scratch, id: i64) -> (bool, bool, i64, i64, Option<String>) {
  let read = |column: &'static str| {
    format!(r#"SELECT count(*) FROM "instructions" WHERE "id" = $1 AND "{column}" IS NOT NULL"#)
  };
  let handed = scratch.count(&read("handed_at"), vec![id.into()]).await;
  let settled = scratch.count(&read("delivered_at"), vec![id.into()]).await;
  let restarts = scratch
    .count(r#"SELECT "restarts" FROM "instructions" WHERE "id" = $1"#, vec![id.into()])
    .await;
  let failures = scratch
    .count(r#"SELECT "failures" FROM "instructions" WHERE "id" = $1"#, vec![id.into()])
    .await;
  let let_go = scratch
    .texts(&format!(r#"SELECT COALESCE("let_go", '') FROM "instructions" WHERE "id" = {id}"#))
    .await
    .into_iter()
    .find(|why| !why.is_empty());

  (handed > 0, settled > 0, restarts, failures, let_go)
}

async fn handed(scratch: &Scratch, id: i64) {
  scratch
    .run(r#"UPDATE "instructions" SET "handed_at" = 1 WHERE "id" = $1"#, vec![id.into()])
    .await;
}

#[tokio::test]
async fn a_failed_round_gives_back_every_row_and_counts_only_a_failure_the_agency_answered() {
  let scratch = Scratch::new().await;
  let reply = queue(scratch.db(), AGENT, OWNER, "a delegate's answer", &Queued::default())
    .await
    .unwrap();
  let chores = Queued {
    cause: Cause::Housekeeping,
    ..Queued::default()
  };
  let chore = queue(scratch.db(), AGENT, OWNER, "a record", &chores).await.unwrap();
  let rows = [reply, chore];

  for id in rows {
    handed(&scratch, id).await;
  }

  // The round's turn carried the reply's text; the record never reached a turn.
  carried_by(scratch.db(), &[reply], "session-a", 7).await.unwrap();

  // A failure where the agency was unreachable counts nothing, however often it comes.
  for _ in 0..CHAT_ROUND.instructions.failure_limit + 1 {
    assert_eq!(give_back(scratch.db(), "round-a", &rows, false).await.unwrap(), rows);

    for id in rows {
      handed(&scratch, id).await;
    }
  }

  assert_eq!(
    stood(&scratch, reply).await,
    (true, false, 1, 0, None),
    "its text reached a session once"
  );
  assert_eq!(
    stood(&scratch, chore).await,
    (true, false, 0, 0, None),
    "housekeeping is given back too"
  );

  // A failure the agency answered is counted, and the last the limit allows lets the rows go, kept as failed.
  for _ in 1..CHAT_ROUND.instructions.failure_limit {
    assert_eq!(give_back(scratch.db(), "round-b", &rows, true).await.unwrap(), rows);

    for id in rows {
      handed(&scratch, id).await;
    }
  }

  assert_eq!(give_back(scratch.db(), "round-c", &rows, true).await.unwrap(), Vec::<i64>::new());

  let limit = CHAT_ROUND.instructions.failure_limit;

  assert_eq!(stood(&scratch, reply).await, (true, true, 1, limit, Some("failed".to_owned())));
  assert_eq!(stood(&scratch, chore).await, (true, true, 0, limit, Some("failed".to_owned())));
}

#[tokio::test]
async fn a_round_settles_only_the_rows_a_turn_carried_and_a_restart_deletes_none() {
  let scratch = Scratch::new().await;
  let carried = queue(scratch.db(), AGENT, OWNER, "a job's ending", &Queued::default()).await.unwrap();
  let uncarried = queue(scratch.db(), AGENT, OWNER, "a review check", &Queued::default()).await.unwrap();
  let cut = queue(scratch.db(), AGENT, OWNER, "a queue notice", &Queued::default()).await.unwrap();

  for id in [carried, uncarried, cut] {
    handed(&scratch, id).await;
  }

  carried_by(scratch.db(), &[carried], "session-a", 7).await.unwrap();

  // A later turn names no carrying of a row a turn carried before.
  carried_by(scratch.db(), &[carried], "double-a", 9).await.unwrap();
  assert_eq!(
    scratch
      .count(r#"SELECT "turn" FROM "instructions" WHERE "id" = $1"#, vec![carried.into()])
      .await,
    7
  );

  assert_eq!(settle(scratch.db(), "round-a", &[carried, uncarried]).await.unwrap(), [uncarried]);
  assert_eq!(stood(&scratch, carried).await, (true, true, 0, 0, None), "the carried row is settled");
  assert_eq!(
    stood(&scratch, uncarried).await,
    (false, false, 0, 0, None),
    "no turn carried it: it waits"
  );

  // A restart gives back a row it cut off however often it did, and deletes none.
  for _ in 0..4 {
    carried_by(scratch.db(), &[cut], "session-a", 8).await.unwrap();
    requeue_interrupted(scratch.db()).await.unwrap();
    handed(&scratch, cut).await;
  }

  assert_eq!(stood(&scratch, cut).await, (true, false, 4, 0, None), "a restart counts no failure");
}

#[tokio::test]
async fn rows_handed_across_a_reconnect_to_no_standing_conversation_are_handed_again() {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let chat = Arc::new(StandInChat::new("road-a").owners(OWNER));
  let standing = woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat], deployment()));
  let rooms = Recorded::default();
  let mut last = 0;

  standing.agency.registering(vec![registration(AGENT, "Agent A", vec![], None)]);

  let lost = queue(scratch.db(), AGENT, OWNER, "a research result", &Queued::default())
    .await
    .unwrap();
  let held = queue(scratch.db(), AGENT, -7, "a job's report", &Queued::default()).await.unwrap();

  handed(&scratch, lost).await;
  handed(&scratch, held).await;
  rooms.active.lock().unwrap().push(-7);

  let given = reconcile(scratch.db(), &rooms).await.unwrap();

  assert_eq!(given, [lost], "a conversation that stands keeps the row it holds");
  assert_eq!(stood(&scratch, held).await, (true, false, 0, 0, None));

  deliver_pending(&standing.receiver, &rooms, &mut last, now_ms(), 0).await.unwrap();
  assert_eq!(*rooms.handed.lock().unwrap(), [lost], "the next pass hands the row again");
}

/// A hearer that keeps the number of every event handed to it.
#[derive(Default)]
struct Heard(Mutex<Vec<i64>>);

impl Hearer for Heard {
  fn kinds(&self) -> &'static [&'static str] {
    &["rest.changed"]
  }

  fn hear<'a>(&'a self, event: &'a Event) -> BoxFuture<'a, ()> {
    self.0.lock().unwrap().push(event.id.unwrap_or(0));
    Box::pin(async {})
  }
}

fn event(id: i64, kind: &str, at: i64) -> Event {
  Event {
    kind: kind.to_owned(),
    data: json!({}),
    id: Some(id),
    at: Some(at),
  }
}

#[tokio::test]
async fn the_stream_resumes_after_the_last_event_heard_and_hears_a_fresh_numbering_whole() {
  let scratch = Scratch::new().await;
  let agency = StandInAgency::new();
  let heard = Arc::new(Heard::default());
  let marks = Arc::new(HeardSetting(scratch.db().clone()));
  let hearing = Hearing::new(vec![heard.clone()]).marking(marks.clone());

  assert_eq!(hearing.resume(&agency).await, None, "nothing heard before, nothing resumed");

  marks.keep(Mark { id: 5, at: 500 }).await;
  agency.keeping(vec![
    event(4, "rest.changed", 400),
    event(5, "rest.changed", 500),
    event(6, "turn.ended", 600),
    event(7, "rest.changed", 700),
  ]);

  assert_eq!(hearing.resume(&agency).await, Some(7));
  assert_eq!(*heard.0.lock().unwrap(), [7], "only what came after the last heard, of the kinds heard");
  assert_eq!(marks.last().await, Some(Mark { id: 7, at: 700 }));

  // The agency started again and numbers its events afresh: everything it keeps is new.
  agency.keeping(vec![event(1, "rest.changed", 900), event(2, "rest.changed", 950)]);
  heard.0.lock().unwrap().clear();

  assert_eq!(hearing.resume(&agency).await, Some(2));
  assert_eq!(*heard.0.lock().unwrap(), [1, 2]);
  assert_eq!(marks.last().await, Some(Mark { id: 2, at: 950 }));
}

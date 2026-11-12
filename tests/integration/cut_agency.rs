//! A round's turn across an agency that stops and starts again in the middle of it (docs/bmo-window.md, 「Quiet
//! instruction」): the turn goes to an agency served on a real port, which the test stops, cutting the send and the
//! event stream, and starts again. A turn the agency still holds is re-attached to and its end read off the events
//! it kept, never sent twice; a send cut before its turn was named goes again under its key and attaches to one
//! turn; a turn the agency lost fails the round uncounted and gives its row back, never settled. A round handed to
//! the double and handed back returns what was pushed into the double's turn (docs/drafting.md, 「A round handed to
//! the double」).

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::conversation::Timings;
use bmo_receiver::chat::double_slots;
use bmo_receiver::chat::instructions::{Queued, queue};
use bmo_receiver::chat::runner::Runner;
use bmo_receiver::data_root::DataRoot;
use receiver_policy::chat_line::CHAT_LINE;
use receiver_ports::agency::Said;

use crate::integration::common::Scratch;
use crate::integration::common::served_agency::ServedAgency;
use crate::integration::common::session::StandInSession;
use crate::integration::common::units::{
  ADDRESS, AGENT, OWNER, StandInChat, Standing, deployment, registration, unrun, woken,
};

/// What the agency's turn ends with.
const ANSWER: &str = "Here is what was asked.";

/// The session the agent's turns go on.
const SESSION: &str = "agent-a@room";

/// The instruction the round carries.
const CHECK: &str = "a job's review check";

// The round's timings cut to what the tests can wait on: the queue handing over at once, the working turn
// looked at often, and no window or wait running out while a test stands.
fn timings() -> Timings {
  Timings {
    queue: Duration::from_millis(1),
    wait: Duration::from_secs(60),
    window: Some(Duration::from_secs(60)),
    look: Duration::from_millis(5),
  }
}

// The room's documents a turn's block cannot go out without, under the data root.
fn documents(data: &Path) {
  let chat = data.join("prompts").join("chat");

  std::fs::create_dir_all(&chat).unwrap();
  std::fs::write(chat.join(&CHAT_LINE.room.owner_document), "The owner's own chat.").unwrap();
  std::fs::write(chat.join(&CHAT_LINE.room.group_document), "A group room.").unwrap();
  std::fs::write(chat.join(&CHAT_LINE.capabilities.document), "What the agent can do.").unwrap();
}

// Waits, bounded, until `holds` does.
async fn until(what: &str, holds: impl Fn() -> bool) {
  let waited = tokio::time::timeout(Duration::from_secs(20), async {
    while !holds() {
      tokio::time::sleep(Duration::from_millis(5)).await;
    }
  })
  .await;

  assert!(waited.is_ok(), "{what} never came");
}

/// What a row stands as: settled, the session and turn that carried it, how often its text reached a turn that did
/// not end answered, how often a failure the agency answered was counted on it, and why it was let go.
#[derive(Debug, PartialEq)]
struct Row {
  settled: bool,
  session: Option<String>,
  turn: Option<i64>,
  restarts: i64,
  failures: i64,
  let_go: Option<String>,
}

/// The agent's rounds in the owner's chat, their turns carried by the agency on a real port.
struct Cut {
  scratch: Scratch,
  _data: tempfile::TempDir,
  standing: Standing,
  chat: Arc<StandInChat>,
  served: ServedAgency,
  runner: Runner,
}

impl Cut {
  async fn stood() -> Self {
    let scratch = Scratch::new().await;

    crate::integration::common::session::no_drafting(scratch.db()).await;

    let data = tempfile::tempdir().unwrap();
    let chat = Arc::new(StandInChat::new("road-a").owners(OWNER));

    documents(data.path());

    let mut standing =
      woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat.clone()], deployment()));

    standing.receiver = standing.receiver.clone().re_attaching_every(Duration::from_millis(2));

    let served = ServedAgency::start().await;
    let receiver = standing.receiver.clone();
    let runner = Runner::new(receiver.clone(), Arc::new(Around::new(receiver.clone())), timings(), AGENT);

    standing.agency.registering(vec![registration(AGENT, "Agent A", vec![ADDRESS], None)]);
    standing.agency.carrying_on(Arc::new(StandInSession::new(SESSION, ANSWER)));
    standing.agency.carried_through(Arc::new(served.port()));
    receiver.hold_runner(Arc::new(runner.clone()));

    Cut {
      scratch,
      _data: data,
      standing,
      chat,
      served,
      runner,
    }
  }

  // A quiet instruction for the agent in the owner's chat, handed to its conversation.
  async fn instruct(&self) -> i64 {
    self.instructing(CHECK).await
  }

  async fn instructing(&self, text: &str) -> i64 {
    let id = queue(self.standing.receiver.db(), AGENT, OWNER, text, &Queued::default())
      .await
      .unwrap();

    self.runner.of(AGENT).instruct(OWNER, text.to_owned(), Some(id), None, vec![]).await;
    id
  }

  // Every send handed to the room's chat service, as the words it carried.
  fn sent(&self) -> Vec<String> {
    self.chat.sent().iter().map(|send| serde_json::to_string(send).unwrap()).collect()
  }

  async fn row(&self, id: i64) -> Row {
    let read = bmo_receiver::one(
      self.scratch.db(),
      r#"SELECT "delivered_at", "session", "turn", "restarts", "failures", "let_go" FROM "instructions"
         WHERE "id" = $1"#,
      vec![id.into()],
    )
    .await
    .unwrap()
    .unwrap();

    Row {
      settled: read.try_get::<Option<i64>>("", "delivered_at").unwrap().is_some(),
      session: read.try_get("", "session").unwrap(),
      turn: read.try_get("", "turn").unwrap(),
      restarts: read.try_get("", "restarts").unwrap(),
      failures: read.try_get("", "failures").unwrap(),
      let_go: read.try_get("", "let_go").unwrap(),
    }
  }

  // Waits, bounded, until the row stands as `row`.
  async fn row_until(&self, id: i64, row: &Row) {
    let waited = tokio::time::timeout(Duration::from_secs(20), async {
      while self.row(id).await != *row {
        tokio::time::sleep(Duration::from_millis(5)).await;
      }
    })
    .await;

    assert!(waited.is_ok(), "the row never stood as {row:?}: {:?}", self.row(id).await);
  }
}

#[tokio::test]
async fn a_turn_the_agency_held_across_its_restart_is_re_attached_to_and_its_row_settled() {
  let cut = Cut::stood().await;
  let id = cut.instruct().await;

  until("the turn named", || cut.served.turns() == [1]).await;

  // The agency stops mid-turn, the turn it picked up ends while it is down, and it starts again.
  cut.served.stop().await;
  cut.served.end(1, ANSWER);
  cut.served.restart().await;

  until("the answer in the room", || !cut.sent().is_empty()).await;
  until("the round's end", || cut.runner.idle()).await;
  cut
    .row_until(
      id,
      &Row {
        settled: true,
        session: Some(SESSION.to_owned()),
        turn: Some(1),
        restarts: 0,
        failures: 0,
        let_go: None,
      },
    )
    .await;

  let sends = cut.served.sends();
  let sent = cut.sent();

  assert_eq!(sends.len(), 1, "the cut turn is re-attached to, never sent again: {sends:#?}");
  assert!(sends[0]["prompt"].as_str().unwrap().contains(CHECK), "the turn carried the instruction");
  assert_eq!(sent.len(), 1, "the room is answered once: {sent:?}");
  assert!(sent[0].contains(ANSWER), "the answer is the end the kept events held: {sent:?}");
}

#[tokio::test]
async fn a_send_cut_before_its_turn_was_named_goes_again_under_its_key_to_one_turn() {
  let cut = Cut::stood().await;

  cut.served.naming(false);

  let id = cut.instruct().await;

  until("the send taken", || cut.served.sends().len() == 1).await;

  // The agency stops before it named the turn, and starts again naming it.
  cut.served.stop().await;
  cut.served.naming(true);
  cut.served.restart().await;

  until("the turn named", || cut.served.turns() == [1]).await;
  cut.served.end(1, ANSWER);
  until("the answer in the room", || !cut.sent().is_empty()).await;
  until("the round's end", || cut.runner.idle()).await;
  cut
    .row_until(
      id,
      &Row {
        settled: true,
        session: Some(SESSION.to_owned()),
        turn: Some(1),
        restarts: 0,
        failures: 0,
        let_go: None,
      },
    )
    .await;

  let sends = cut.served.sends();
  let key = sends[0]["key"].as_str().expect("the send carries its key");

  assert_eq!(sends.len(), 2, "the cut send went once more: {sends:#?}");
  assert_eq!(sends[1]["key"].as_str(), Some(key), "under the same key");
  assert_eq!(sends[1]["prompt"], sends[0]["prompt"], "with the same words");
  assert_eq!(cut.sent().len(), 1, "the room is answered once");
}

#[tokio::test]
async fn a_turn_the_restarted_agency_lost_fails_the_round_uncounted_and_gives_its_row_back() {
  let cut = Cut::stood().await;
  let id = cut.instruct().await;

  until("the turn named", || cut.served.turns() == [1]).await;

  // The agency stops mid-turn and comes back holding no record of it.
  cut.served.stop().await;
  cut.served.forget();
  cut.served.restart().await;

  cut
    .row_until(
      id,
      &Row {
        settled: false,
        session: None,
        turn: None,
        restarts: 1,
        failures: 0,
        let_go: None,
      },
    )
    .await;
  until("the round's end", || cut.runner.idle()).await;

  assert_eq!(cut.served.sends().len(), 1, "nothing is sent again to a turn the agency lost");
  assert!(
    cut.sent().iter().all(|sent| !sent.contains(ANSWER)),
    "no answer went out for a turn whose end was never read: {:?}",
    cut.sent()
  );
}

#[tokio::test]
async fn a_round_the_double_hands_back_returns_what_was_pushed_into_its_turn() {
  const NOTICE: &str = "a queue notice";
  const HANDOFF: &str = "<<HANDOFF>>";

  let cut = Cut::stood().await;

  *cut.standing.agency.saying.lock().unwrap() = Some(Said::Delivered);

  let check = cut.instruct().await;

  // The agent's own turn hands the round to its double.
  until("the agent's turn", || cut.served.turns() == [1]).await;
  cut.served.end(1, HANDOFF);
  until("the double's turn", || cut.served.turns() == [1, 2]).await;

  // An instruction that comes while the double works is pushed into the double's turn.
  let notice = cut.instructing(NOTICE).await;

  until("the notice pushed into the double's turn", || {
    cut
      .standing
      .agency
      .pushes
      .lock()
      .unwrap()
      .iter()
      .any(|(session, turn, words)| session.starts_with("double@") && *turn == 2 && words.contains(NOTICE))
  })
  .await;

  // The double answers the sentinel itself, so it hands the round back.
  cut.served.end(2, HANDOFF);
  until("the agent's own session asked again", || cut.served.turns() == [1, 2, 3]).await;
  // Pending again with its carrying cleared, marked as received before by the double's session.
  cut
    .row_until(
      notice,
      &Row {
        settled: false,
        session: None,
        turn: None,
        restarts: 1,
        failures: 0,
        let_go: None,
      },
    )
    .await;
  cut.served.end(3, ANSWER);
  until("the answer in the room", || !cut.sent().is_empty()).await;
  until("the round's end", || cut.runner.idle()).await;

  let sends = cut.served.sends();
  let back = CHAT_LINE.room.handed_back.split("{why}").next().unwrap_or_default();
  let finishes = cut.standing.agency.double_finishes.lock().unwrap().clone();

  assert!(sends[2]["prompt"].as_str().unwrap().contains(back), "{:#?}", sends[2]);
  assert_eq!(finishes.len(), 1, "the double's slot is given back: {finishes:?}");
  assert!(finishes[0].1, "as cancelled, so its run does not go on: {finishes:?}");
  assert!(
    double_slots::held(cut.scratch.db()).await.unwrap().is_empty(),
    "the receiver holds no slot of the double once it was given back"
  );
  assert!(cut.row(check).await.settled, "the instruction the agent's own turn carried is settled");
  assert!(
    !cut.row(notice).await.settled,
    "the instruction only the double's turn received waits again: {:?}",
    cut.row(notice).await
  );
}

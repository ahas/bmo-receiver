//! A round's turn across an agency that stops and starts again in the middle of it (docs/bmo-window.md, 「Quiet
//! instruction」): the turn goes to an agency served on a real port, which the test stops, cutting the send and the
//! event stream, and starts again. A turn the agency still holds is re-attached to and its end read off the events
//! it kept, never sent twice; a send cut before its turn was named goes again under its key and attaches to one
//! turn; a turn the agency lost fails the round uncounted and gives its row back, never settled; and a turn that
//! ends while its stream stands listening unheard is found at the next look, the round failing and running again
//! rather than holding the agent behind an end nobody will tell. A work turn the agency's restart or the rest cut
//! is continued in its room rather than ordered as a broken run, its end told as cut.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::conversation::Timings;
use bmo_receiver::chat::instructions::{Queued, queue};
use bmo_receiver::chat::runner::Runner;
use bmo_receiver::data_root::DataRoot;
use receiver_policy::chat_line::CHAT_LINE;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_ports::agency::{Rest, RestKind};

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

    crate::integration::common::session::no_stages(scratch.db()).await;

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

  // Waits, bounded, until the receiver heard the agency name turn 1 and wrote it down as carrying the row, so a stop
  // after it cuts a turn the receiver knows rather than a send whose turn it never heard named.
  async fn carried(&self, id: i64) {
    self
      .row_until(
        id,
        &Row {
          settled: false,
          session: Some(SESSION.to_owned()),
          turn: Some(1),
          restarts: 0,
          failures: 0,
          let_go: None,
        },
      )
      .await;
  }

  // The continuations of cut rounds standing, each its id and its agent, room and words.
  async fn continuations(&self) -> Vec<(i64, String)> {
    self
      .scratch
      .texts(
        r#"SELECT "id"::text || '|' || "agent" || ' | ' || "chat_id"::text || ' | ' || "text"
           FROM "instructions" WHERE "kind" = 'turn-cut' ORDER BY "id""#,
      )
      .await
      .into_iter()
      .map(|row| {
        let (id, rest) = row.split_once('|').unwrap();

        (id.parse().unwrap(), rest.to_owned())
      })
      .collect()
  }

  // How many failure orders stand.
  async fn orders(&self) -> usize {
    self
      .scratch
      .texts(r#"SELECT "text" FROM "instructions" WHERE "kind" LIKE 'failure-order%'"#)
      .await
      .len()
  }

  // The one continuation standing, checked as a room reply of the agent's in the owner's chat naming `cause`, and
  // handed to the room's conversation as the pass hands it once the agency answers or the rest lifts; the turn it
  // opens ends answered, and its answer reaches the room.
  async fn continued(&self, cause: &str, sends_before: usize) {
    let continuations = self.continuations().await;

    assert_eq!(continuations.len(), 1, "one continuation stands: {continuations:?}");

    let (id, row) = &continuations[0];
    let opening = format!("{AGENT} | {OWNER} | {}\n", CHAT_ROUND.cut.marker);

    assert!(
      row.starts_with(&opening),
      "the agent's continuation in the room the round ran in: {row}"
    );
    assert!(row.contains(cause), "it names what cut the turn: {row}");
    assert!(row.contains(" UTC by "), "and when: {row}");
    assert_eq!(self.orders().await, 0, "a cut turn raises no failure order");

    let cut = self.standing.heard.events("run-cut");

    assert_eq!(cut.len(), 1, "the round's end is told as cut: {cut:?}");
    assert!(self.standing.heard.events("run-failed").is_empty(), "and never as broken");

    let text = row[row.find(&CHAT_ROUND.cut.marker).unwrap()..].to_owned();

    self.runner.of(AGENT).instruct(OWNER, text, Some(*id), None, vec![]).await;

    until("the continuation's turn", || self.served.sends().len() == sends_before + 1).await;

    let sends = self.served.sends();
    let prompt = sends.last().unwrap()["prompt"].as_str().unwrap().to_owned();

    assert!(prompt.contains(&CHAT_ROUND.cut.marker), "the turn opens on the continuation: {prompt}");

    let turn = *self.served.turns().last().unwrap();

    // The turn is ended only once the receiver heard it named and wrote it down as carrying the continuation, so
    // its end is read as the continuation's own.
    self
      .row_until(
        *id,
        &Row {
          settled: false,
          session: Some(SESSION.to_owned()),
          turn: Some(turn),
          restarts: 0,
          failures: 0,
          let_go: None,
        },
      )
      .await;
    self.served.end(turn, ANSWER);
    until("the continued work's answer in the room", || {
      self.sent().iter().any(|sent| sent.contains(ANSWER))
    })
    .await;
    until("the continuation's round end", || self.runner.idle()).await;
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

  cut.carried(id).await;

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

  cut.carried(id).await;

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
async fn a_turn_that_ends_unheard_while_the_stream_listens_is_found_by_the_comparison() {
  let cut = Cut::stood().await;
  let id = cut.instruct().await;

  cut.carried(id).await;

  // The turn ends with its stream still open and its end kept by nobody: the comparison reads the closed
  // turn, the round re-attaches, fails uncounted, and gives its row back.
  cut.served.end_unheard(1, ANSWER);

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
  assert_eq!(cut.served.sends().len(), 1, "nothing is sent for the turn that ended unheard");
  assert!(
    cut.sent().iter().all(|sent| !sent.contains(ANSWER)),
    "no answer went out for a turn whose end was never read: {:?}",
    cut.sent()
  );

  // The row goes round again, handed to its room's conversation as the pass would hand it, on a turn of its
  // own this time, whose end is told as ever.
  cut.runner.of(AGENT).instruct(OWNER, CHECK.to_owned(), Some(id), None, vec![]).await;

  until("the turn sent again", || cut.served.turns() == [1, 2]).await;
  cut.served.end(2, ANSWER);
  until("the answer in the room", || !cut.sent().is_empty()).await;
  until("the round's end", || cut.runner.idle()).await;
  cut
    .row_until(
      id,
      &Row {
        settled: true,
        session: Some(SESSION.to_owned()),
        turn: Some(2),
        restarts: 1,
        failures: 0,
        let_go: None,
      },
    )
    .await;

  assert!(
    cut.sent()[0].contains(ANSWER),
    "the second turn's answered end settled its row: {:?}",
    cut.sent()
  );
}

#[tokio::test]
async fn a_work_turn_the_agencys_restart_cut_is_continued_in_its_room_and_never_ordered() {
  let cut = Cut::stood().await;
  let id = cut.instruct().await;

  cut.carried(id).await;

  // The agency restarts under the working turn, as a deployment the agent itself started does, and the next server
  // ends the turn failed, its program gone.
  cut.served.stop().await;
  cut.served.fail(1, "its program was gone when the agency started again");
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
  assert_eq!(cut.standing.heard.events("run-cut")[0]["cut"], "agency");

  cut.continued(&CHAT_ROUND.cut.agency, 1).await;
}

#[tokio::test]
async fn a_work_turn_the_rest_cut_is_continued_in_its_room_once_the_rest_lifts() {
  let cut = Cut::stood().await;
  let id = cut.instruct().await;

  cut.carried(id).await;

  // The rest closes the agent's road while the turn works, and the turn ends failed under it.
  cut.standing.agency.at_rest(Rest {
    resting: true,
    until: None,
    kind: Some(RestKind::Limit),
  });
  cut.served.fail(1, "the usage limit was reached");

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
  assert_eq!(cut.standing.heard.events("run-cut")[0]["cut"], "rest");

  // The rest lifts, and the continuation opens the next turn.
  cut.standing.agency.at_rest(Rest {
    resting: false,
    until: None,
    kind: None,
  });
  cut.continued(&CHAT_ROUND.cut.rest, 1).await;
}

//! A companion's round carried whole (docs/wake.md, 「A companion's round」): its words handed to its conversations,
//! its turn carried into the agency's session through a stand-in, and what its reset cut off, a turn the reset
//! overtook, an answer held while a cut-off comes, and an engine's error each ending as the old companion round
//! ended them.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::conversation::{Conversations, Timings};
use bmo_receiver::chat::instructions::{Queued, queue};
use bmo_receiver::chat::runner::Runner;
use bmo_receiver::companion::cut::{CUT_OFF, CutOffs, cut_off};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::hearing::Hearing;
use bmo_receiver::receiver::wake::agents::window_until;
use bmo_receiver::receiver::{Inbound, Room};
use bmo_receiver::{now_ms, one};
use receiver_policy::chat_line::CHAT_LINE;
use serde_json::json;

use crate::common::Scratch;
use crate::common::session::StandInSession;
use crate::common::units::{
  ADDRESS, AGENT, OWNER, StandInChat, Standing, deployment, registration, unrun, woken,
};

/// The companion whose rounds these are.
const COMPANION: &str = "agent-c";

/// What the session's every turn ends with, where it answers.
const ANSWER: &str = "Here is what was asked.";

/// The session the companion's turns go on.
const SESSION: &str = "agent-c@room";

/// The second the room's words are said from: a message's moment is this and its number.
const SAID_FROM: i64 = 1_760_000_000;

const FIRST: &str = "the first words";
const LATER: &str = "words said after the first";

// The owner's private chat, the room every round of these tests runs in.
fn owners_room() -> Room {
  Room {
    owner_room: true,
    identity: None,
    private_to: None,
  }
}

fn said(message_id: i64, text: &str) -> Inbound {
  Inbound {
    chat_id: OWNER,
    message_id,
    sender_id: Some(OWNER),
    sender_name: "tester".into(),
    sent_at: (SAID_FROM + message_id) * 1_000,
    text: text.into(),
    image_path: None,
    reply_to_message_id: None,
    via: "road-a".into(),
    mode: Some("bot-account".into()),
    via_reader: None,
    album: None,
  }
}

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

/// The companion's rounds in the owner's chat, standing on the receiver's own conversations and the stand-in
/// units, every turn going on `session`.
struct Rounds {
  _scratch: Scratch,
  _data: tempfile::TempDir,
  standing: Standing,
  chat: Arc<StandInChat>,
  session: Arc<StandInSession>,
  runner: Runner,
  companion: Conversations,
}

impl Rounds {
  async fn on(session: StandInSession) -> Self {
    let scratch = Scratch::new().await;
    let data = tempfile::tempdir().unwrap();
    let chat = Arc::new(StandInChat::new("road-a").owners(OWNER));

    documents(data.path());

    let standing = woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat.clone()], deployment()));
    let session = Arc::new(session);
    let receiver = standing.receiver.clone();
    let runner = Runner::new(receiver.clone(), Arc::new(Around::new(receiver.clone())), timings(), AGENT);
    let mut companion = registration(COMPANION, "Agent C", vec!["agent-c"], None, false);

    companion.row.insert("turnShape".to_owned(), json!("pieces"));
    standing
      .agency
      .registering(vec![registration(AGENT, "Agent A", vec![ADDRESS], None, true), companion]);
    standing.agency.carrying_on(session.clone());
    receiver.hold_runner(Arc::new(runner.clone()));

    let companion = runner.of(COMPANION);

    Rounds {
      _scratch: scratch,
      _data: data,
      standing,
      chat,
      session,
      runner,
      companion,
    }
  }

  async fn hear(&self, message_id: i64, text: &str) {
    self.companion.enqueue(said(message_id, text), owners_room(), false).await;
  }

  // A quiet instruction for the companion in the room, its row handing back `run` where it names one.
  async fn instruct(&self, text: &str, run: Option<i64>) -> i64 {
    let queued = Queued {
      run_id: run,
      ..Queued::default()
    };
    let id = queue(self.standing.receiver.db(), COMPANION, OWNER, text, &queued).await.unwrap();

    self.companion.instruct(OWNER, text.to_owned(), Some(id), None, vec![]).await;
    id
  }

  // The companion's reset point moved to `point`, in epoch milliseconds.
  fn reset_at(&self, point: i64) {
    *self.standing.agency.point.lock().unwrap() = Some(point);
  }

  async fn settled(&self) {
    until("the rounds' end", || self.runner.idle() && !self.session.working()).await;
  }

  // Every send handed to the room's chat service, as the words it carried.
  fn sent(&self) -> Vec<String> {
    self.chat.sent().iter().map(|send| serde_json::to_string(send).unwrap()).collect()
  }

  async fn window(&self) -> i64 {
    window_until(self.standing.receiver.db(), OWNER, COMPANION).await.unwrap()
  }

  async fn delivered(&self, id: i64) -> bool {
    let row = one(
      self.standing.receiver.db(),
      r#"SELECT "delivered_at" FROM "instructions" WHERE "id" = $1"#,
      vec![id.into()],
    )
    .await
    .unwrap()
    .unwrap();

    row.try_get::<Option<i64>>("", "delivered_at").unwrap().is_some()
  }
}

#[tokio::test]
async fn words_said_before_the_reset_reach_no_turn() {
  let rounds = Rounds::on(StandInSession::new(SESSION, ANSWER)).await;

  // The point stands inside the first message's second, which goes with the words before it.
  rounds.reset_at((SAID_FROM + 1) * 1000 + 500);
  rounds.hear(1, FIRST).await;
  rounds.hear(2, LATER).await;
  until("the turn", || !rounds.session.prompts().is_empty()).await;
  rounds.settled().await;

  let prompts = rounds.session.prompts();

  assert_eq!(prompts.len(), 1, "the words said before the reset call no turn of their own");
  assert!(prompts[0].contains(LATER), "the words said after the reset are the turn's");
  assert!(!prompts[0].contains(FIRST), "the words said before the reset are not");
  assert_eq!(rounds.sent().len(), 1, "the turn answers the room");
  assert!(rounds.window().await > 0, "an answered turn moves the room's window");

  let attached = rounds.standing.gate.attached.lock().unwrap().clone();

  assert_eq!(
    attached.len(),
    1,
    "the gate is asked once to file the round's judgements under the answer"
  );
  assert_eq!(attached[0].0, OWNER, "in the room the answer went into");
}

#[tokio::test]
async fn a_line_handing_back_a_run_begun_before_the_reset_reaches_no_turn() {
  let rounds = Rounds::on(StandInSession::new(SESSION, ANSWER)).await;
  let point = now_ms();

  rounds.reset_at(point);
  rounds
    .standing
    .agency
    .runs
    .lock()
    .unwrap()
    .extend([(7, point / 1000 - 10), (8, point / 1000 + 10)]);

  let late = rounds.instruct("the result of the run begun before", Some(7)).await;

  rounds.instruct("the result of the run begun after", Some(8)).await;
  until("the turn", || !rounds.session.prompts().is_empty()).await;
  rounds.settled().await;

  let prompts = rounds.session.prompts();

  assert_eq!(prompts.len(), 1);
  assert!(
    prompts[0].contains("the run begun after"),
    "a run begun after the reset hands its result back"
  );
  assert!(!prompts[0].contains("the run begun before"), "a run begun before it hands back nothing");
  assert!(rounds.delivered(late).await, "the line dropped is settled with the round that took it");
}

#[tokio::test]
async fn a_turn_its_reset_overtook_leaves_neither_an_answer_nor_a_window() {
  let rounds = Rounds::on(StandInSession::new(SESSION, ANSWER).holding_the_first()).await;

  rounds.hear(1, FIRST).await;
  until("the turn", || rounds.session.working()).await;
  // The reset comes while the turn works; the agency answers the turn all the same.
  rounds.reset_at(now_ms());
  rounds.session.let_go();
  rounds.settled().await;

  assert_eq!(rounds.session.prompts().len(), 1);
  assert!(rounds.sent().is_empty(), "no answer goes into the room");
  assert_eq!(rounds.window().await, 0, "no window is moved");
}

#[tokio::test]
async fn an_engine_error_is_told_to_the_room_in_one_line() {
  let ending = json!({
    "outcome": "failed",
    "reason": "the engine was busy",
    "engineError": { "kind": "rate_limit" },
  });
  let rounds = Rounds::on(StandInSession::new(SESSION, ANSWER).ending_with(ending)).await;

  rounds.hear(1, FIRST).await;
  until("the line", || !rounds.sent().is_empty()).await;
  rounds.settled().await;

  let sent = rounds.sent();

  assert_eq!(sent.len(), 1, "the room is told once");
  assert!(
    sent[0].contains("companion-engine-error in en"),
    "the line is the fixed line, in the room's language"
  );
}

#[tokio::test]
async fn any_other_failure_says_nothing_in_the_room() {
  let ending = json!({ "outcome": "failed", "reason": "the turn went wrong" });
  let rounds = Rounds::on(StandInSession::new(SESSION, ANSWER).ending_with(ending)).await;

  rounds.hear(1, FIRST).await;
  until("the turn", || !rounds.session.prompts().is_empty()).await;
  rounds.settled().await;

  assert!(rounds.sent().is_empty());
}

#[tokio::test]
async fn an_answer_being_sent_holds_off_the_reset_until_it_is_out() {
  let rounds = Rounds::on(StandInSession::new(SESSION, ANSWER)).await;
  let release = rounds.chat.holding_the_next_send();

  rounds.hear(1, FIRST).await;
  until("the answer held being sent", || rounds.chat.send_held()).await;

  let point = now_ms();

  rounds.reset_at(point);

  let cutting = {
    let (receiver, runner, chat) =
      (rounds.standing.receiver.clone(), rounds.runner.clone(), rounds.chat.clone());

    tokio::spawn(async move {
      cut_off(&receiver, &runner, &json!({ "agent": COMPANION, "point": point })).await;
      chat.sent().len()
    })
  };

  release.notify_one();

  assert_eq!(cutting.await.unwrap(), 1, "the cut-off waits until the answer being sent is out");
  rounds.settled().await;
}

#[tokio::test]
async fn a_cut_off_lets_go_unread_what_waited_for_the_companions_turns() {
  let rounds = Rounds::on(StandInSession::new(SESSION, ANSWER).holding_the_first()).await;

  Hearing::new(vec![Arc::new(CutOffs::new(rounds.standing.receiver.clone(), rounds.runner.clone()))])
    .keep(rounds.standing.agency.clone());
  until("the cut-off heard", || rounds.session.listeners() == 1).await;
  rounds.hear(1, FIRST).await;
  until("the turn", || rounds.session.working()).await;
  // What comes while a companion's turn works waits for the round after it.
  rounds.hear(2, LATER).await;

  let waiting = rounds.instruct("an instruction that waited", None).await;
  let point = now_ms();

  rounds.reset_at(point);
  rounds.session.told(CUT_OFF, json!({ "agent": COMPANION, "point": point }));

  let waited = tokio::time::timeout(Duration::from_secs(20), async {
    while !rounds.delivered(waiting).await {
      tokio::time::sleep(Duration::from_millis(5)).await;
    }
  })
  .await;

  assert!(waited.is_ok(), "the instruction that waited is settled by the cut-off");

  rounds.session.let_go();
  rounds.settled().await;

  assert_eq!(rounds.session.prompts().len(), 1, "what waited calls no turn");
  assert!(rounds.sent().is_empty(), "the overtaken turn answers nothing");
}

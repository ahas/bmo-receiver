//! Every supervisor is alike (docs/wake.md, 「The room's queue and the round」; 「An agent's reset」): another
//! supervisor's round is the same round the default answerer's is, every event of it names the supervisor under its
//! own number, and its reset's cut-off lets go unread what waited for its turns.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::conversation::{Conversations, Timings};
use bmo_receiver::chat::cut_off::{self, CUT_OFF, CutOffs};
use bmo_receiver::chat::instructions::{Queued, queue};
use bmo_receiver::chat::runner::Runner;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::hearing::Hearing;
use bmo_receiver::receiver::{Inbound, Room};
use bmo_receiver::{now_ms, one};
use receiver_policy::chat_line::CHAT_LINE;
use serde_json::json;

use crate::integration::common::Scratch;
use crate::integration::common::session::StandInSession;
use crate::integration::common::units::{
  ADDRESS, AGENT, OWNER, StandInChat, Standing, deployment, registration, unrun, woken,
};

/// The agent added on the console whose rounds these are.
const ADDED: &str = "agent-c";

/// What the session's every turn ends with, where it answers.
const ANSWER: &str = "Here is what was asked.";

/// The session the agent's turns go on.
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
    image_state: None,
    transcript: None,
    voice_seconds: None,
    voice_state: None,
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

/// A supervisor's rounds in the owner's chat, other than the default answerer's, standing on the receiver's own conversations and the stand-in
/// units, every turn going on `session`.
struct Rounds {
  _scratch: Scratch,
  _data: tempfile::TempDir,
  standing: Standing,
  chat: Arc<StandInChat>,
  session: Arc<StandInSession>,
  runner: Runner,
  added: Conversations,
}

impl Rounds {
  async fn on(session: StandInSession) -> Self {
    let scratch = Scratch::new().await;

    crate::integration::common::session::no_drafting(scratch.db()).await;

    let data = tempfile::tempdir().unwrap();
    let chat = Arc::new(StandInChat::new("road-a").owners(OWNER));

    documents(data.path());

    let standing = woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat.clone()], deployment()));
    let session = Arc::new(session);
    let receiver = standing.receiver.clone();
    let runner = Runner::new(receiver.clone(), Arc::new(Around::new(receiver.clone())), timings(), AGENT);
    let mut added = registration(ADDED, "Agent C", vec!["agent-c"], None);

    added.row.insert("turnShape".to_owned(), json!("pieces"));
    standing
      .agency
      .registering(vec![registration(AGENT, "Agent A", vec![ADDRESS], None), added]);
    standing.agency.carrying_on(session.clone());
    receiver.hold_runner(Arc::new(runner.clone()));

    let added = runner.of(ADDED);

    Rounds {
      _scratch: scratch,
      _data: data,
      standing,
      chat,
      session,
      runner,
      added,
    }
  }

  async fn hear(&self, message_id: i64, text: &str) {
    self.added.enqueue(said(message_id, text), owners_room(), false, false).await;
  }

  // A quiet instruction for the supervisor in the room, its row handing back `run` where it names one.
  async fn instruct(&self, text: &str, run: Option<i64>) -> i64 {
    let queued = Queued {
      run_id: run,
      ..Queued::default()
    };
    let id = queue(self.standing.receiver.db(), ADDED, OWNER, text, &queued).await.unwrap();

    self.added.instruct(OWNER, text.to_owned(), Some(id), None, vec![]).await;
    id
  }

  async fn settled(&self) {
    until("the rounds' end", || self.runner.idle() && !self.session.working()).await;
  }

  // Every send handed to the room's chat service, as the words it carried.
  fn sent(&self) -> Vec<String> {
    self.chat.sent().iter().map(|send| serde_json::to_string(send).unwrap()).collect()
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
async fn another_supervisors_round_is_the_default_answerers_round() {
  let rounds = Rounds::on(StandInSession::new(SESSION, ANSWER)).await;

  rounds.hear(1, FIRST).await;
  until("the turn", || !rounds.session.prompts().is_empty()).await;
  rounds.settled().await;

  let prompt = &rounds.session.prompts()[0];

  assert!(prompt.contains("TURN SCOPE"), "the round is composed as every agent's is: {prompt}");
  assert!(prompt.contains(FIRST));
  assert!(
    rounds.sent().iter().any(|send| send.contains(ANSWER)),
    "its answer goes out: {:?}",
    rounds.sent()
  );
}

#[tokio::test]
async fn a_cut_off_lets_go_unread_what_waited_for_the_agents_turns() {
  let rounds = Rounds::on(StandInSession::new(SESSION, ANSWER).holding_the_first()).await;

  Hearing::new(vec![Arc::new(CutOffs::new(rounds.standing.receiver.clone(), rounds.runner.clone()))])
    .keep(rounds.standing.agency.clone());
  until("the cut-off heard", || rounds.session.listeners() == 1).await;
  rounds.hear(1, FIRST).await;
  until("the turn", || rounds.session.working()).await;
  // What comes while the agent's turn works waits for the round after it.
  rounds.hear(2, LATER).await;

  let waiting = rounds.instruct("an instruction that waited", None).await;
  let point = now_ms();

  rounds.session.told(CUT_OFF, json!({ "agent": ADDED, "point": point }));

  let waited = tokio::time::timeout(Duration::from_secs(20), async {
    while !rounds.delivered(waiting).await {
      tokio::time::sleep(Duration::from_millis(5)).await;
    }
  })
  .await;

  assert!(waited.is_ok(), "the instruction that waited is settled by the cut-off");
  assert_eq!(
    cut_off::point(rounds.standing.receiver.db(), ADDED).await.unwrap(),
    Some(point),
    "the cut-off's point is kept as the agent's reset point"
  );

  rounds.session.let_go();
  rounds.settled().await;

  assert_eq!(rounds.session.prompts().len(), 1, "what waited calls no turn");
  assert!(
    !rounds.session.prompts()[0].contains(LATER),
    "the words the cut-off let go reach no turn"
  );
}

#[tokio::test]
async fn every_event_of_another_supervisors_round_names_it_under_its_own_number() {
  let rounds = Rounds::on(StandInSession::new(SESSION, ANSWER)).await;

  bmo_receiver::run(
    rounds.standing.receiver.db(),
    r#"INSERT INTO "provider_users" ("provider", "user_id", "sender_id") VALUES ('road-a', $1, 31)"#,
    vec![ADDED.into()],
  )
  .await
  .unwrap();
  rounds.hear(1, FIRST).await;
  until("the turn", || !rounds.session.prompts().is_empty()).await;
  rounds.settled().await;

  let speaker = json!({ "id": 31, "name": ADDED });

  for kind in ["run-started", "sent"] {
    let events = rounds.standing.heard.events(kind);

    assert!(!events.is_empty(), "the round tells {kind}");

    for event in events {
      assert_eq!(event["speaker"], speaker, "{kind} names the agent by its name and its own number");
    }
  }
}

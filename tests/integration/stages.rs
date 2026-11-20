//! A room's round carried through its chain of instruction stages (docs/instruction.md, 「The stages before the
//! send」): the message turn writes the message, each stage is one system instruction carrying its own words over
//! the message as it stands, a stage that answers PASS leaves it as it was, a stage that reprints it changed puts
//! its whole answer in its place, and the answer after the last stage is what the room receives.

use std::sync::Arc;
use std::time::Duration;

use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::conversation::Timings;
use bmo_receiver::chat::runner::Runner;
use bmo_receiver::chat::stages::table::set_text;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::events::Events;
use bmo_receiver::receiver::{Context, Inbound, Room};
use receiver_policy::chat_line::CHAT_LINE;
use receiver_policy::fill_marks;
use receiver_policy::instruction::INSTRUCTION;
use serde_json::Value;

use crate::integration::common::Scratch;
use crate::integration::common::session::{StandInSession, set_chain};
use crate::integration::common::units::{
  ADDRESS, AGENT, OWNER, StandInChat, Standing, deployment, registration, unrun, woken,
};

const SESSION: &str = "agent-a@room";

const ASKED: &str = "is the build green";
const WRITTEN: &str = "The build is green, tester.";
const REVISED: &str = "Yes, tester: the build is green and deployed.";

// The two stages the tests chain, each with the words the console set for the supervisor.
const NAMES: (&str, &str) = ("names", "Does the message address every person by the term the brain keeps?");
const FACTS: (&str, &str) = ("facts", "Does the message say what was deployed?");

fn owners_room() -> Room {
  Room {
    owner_room: true,
    identity: Some(7),
    private_to: None,
    sealed: false,
  }
}

fn said(message_id: i64, text: &str) -> Inbound {
  Inbound {
    chat_id: OWNER,
    message_id,
    sender_id: Some(OWNER),
    sender_name: "tester".into(),
    sent_at: 1_760_000_000 + message_id,
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

// The round's timings cut to what the test can wait on: the queue handing over at once and no window or wait
// running out while it stands.
fn timings() -> Timings {
  Timings {
    queue: Duration::from_millis(1),
    wait: Duration::from_secs(60),
    window: Some(Duration::from_secs(60)),
    look: Duration::from_millis(5),
  }
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

/// One round of the owner's chat run through the chain `[names, facts]`, its session answering `answers` in turn
/// with every stage's turn shown: what the session was asked, what the room received, the round log's rows and the
/// stream's news.
struct Staged {
  scratch: Scratch,
  _data: tempfile::TempDir,
  _standing: Standing,
  chat: Arc<StandInChat>,
  session: Arc<StandInSession>,
  events: Arc<Events>,
}

impl Staged {
  async fn run(session: StandInSession) -> Self {
    let scratch = Scratch::new().await;

    for (name, text) in [NAMES, FACTS] {
      set_text(scratch.db(), AGENT, name, text).await.unwrap();
    }

    set_chain(scratch.db(), &[NAMES.0, FACTS.0]).await;

    let data = tempfile::tempdir().unwrap();
    let prompts = data.path().join("prompts").join("chat");

    std::fs::create_dir_all(&prompts).unwrap();
    std::fs::write(prompts.join(&CHAT_LINE.room.owner_document), "The owner's own chat.").unwrap();
    std::fs::write(prompts.join(&CHAT_LINE.capabilities.document), "What the agent can do.").unwrap();

    let chat = Arc::new(StandInChat::new("road-a").owners(OWNER));
    let events = Arc::new(Events::new());
    let mut standing =
      woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat.clone()], deployment()));

    standing.receiver = standing.receiver.clone().with_news(events.clone()).with_instruction_gate(true);

    let session = Arc::new(session.showing_stages());
    let receiver = standing.receiver.clone();
    let runner = Runner::new(receiver.clone(), Arc::new(Around::new(receiver.clone())), timings(), AGENT);

    standing.agency.registering(vec![registration(AGENT, "Agent A", vec![ADDRESS], None)]);
    standing.agency.carrying_on(session.clone());
    receiver.hold_runner(Arc::new(runner.clone()));
    receiver.message(&said(1, ASKED), &Context::default(), owners_room()).await.unwrap();
    until("the round's end", || {
      runner.idle() && !session.working() && !session.prompts().is_empty()
    })
    .await;

    Staged {
      scratch,
      _data: data,
      _standing: standing,
      chat,
      session,
      events,
    }
  }

  // The words of every send the room's chat service was handed.
  fn sent(&self) -> Vec<String> {
    self.chat.sent().iter().map(|send| serde_json::to_string(send).unwrap()).collect()
  }

  // The round log's rows, each its kind, its name and its outcome.
  async fn log(&self) -> Vec<String> {
    self
      .scratch
      .texts(
        r#"SELECT "kind" || ' ' || "question" || ' ' || "outcome" FROM "instruction_rounds" ORDER BY "id""#,
      )
      .await
  }

  // Each turn the stream told starting, as its name and what reached the agent on it.
  fn started(&self) -> Vec<(String, String)> {
    let text = |event: &Value, key: &str| event[key].as_str().unwrap_or_default().to_owned();

    self
      .events
      .held()
      .into_iter()
      .filter(|event| event["type"] == "draft" && event["kind"] == "start")
      .map(|event| (text(&event, "question"), text(&event, "input")))
      .collect()
  }
}

#[tokio::test]
async fn the_stages_run_in_order_a_pass_keeps_the_message_and_the_last_answer_is_what_the_room_receives() {
  let round =
    Staged::run(StandInSession::new(SESSION, "unused").answering(&[WRITTEN, "PASS", REVISED])).await;
  let prompts = round.session.prompts();

  assert_eq!(prompts.len(), 3, "the message turn and one turn a stage: {prompts:#?}");
  assert!(prompts[0].contains(ASKED), "the message turn carries the room's words");

  // Each stage is a system instruction carrying its own words and the message as it then stood.
  for (prompt, (name, text)) in prompts[1..].iter().zip([NAMES, FACTS]) {
    assert!(prompt.starts_with(&fill_marks(&INSTRUCTION.marker, &[("name", name)])), "{prompt}");
    assert!(prompt.contains(text), "the stage's words ride its turn: {prompt}");
    assert!(prompt.contains(&format!("{}\n{WRITTEN}", INSTRUCTION.standing)), "{prompt}");
  }

  assert!(!prompts[0].contains(NAMES.1), "the block carries no stage's words");
  assert_eq!(round.sent().len(), 1, "{:?}", round.sent());
  assert!(
    round.sent()[0].contains(REVISED),
    "the last stage's reprint is what goes out: {:?}",
    round.sent()
  );
  assert_eq!(
    round.log().await,
    ["turn message answered", "stage names pass", "stage facts revised", "end  message"]
  );
  assert_eq!(
    round.started(),
    [
      ("message".to_owned(), "message".to_owned()),
      ("names".to_owned(), "system".to_owned()),
      ("facts".to_owned(), "system".to_owned()),
    ],
    "the message turn is told as a message and each stage as a system instruction"
  );
}

#[tokio::test]
async fn a_chain_of_passes_sends_the_message_the_message_turn_wrote() {
  let round =
    Staged::run(StandInSession::new(SESSION, "unused").answering(&[WRITTEN, "PASS", WRITTEN])).await;

  assert_eq!(round.sent().len(), 1, "{:?}", round.sent());
  assert!(round.sent()[0].contains(WRITTEN), "{:?}", round.sent());
  assert_eq!(
    round.log().await,
    ["turn message answered", "stage names pass", "stage facts pass", "end  message"],
    "a stage that reprints the message unchanged passes it"
  );
}

#[tokio::test]
async fn a_stage_whose_run_failed_sends_nothing() {
  let session = StandInSession::new(SESSION, "unused").answering(&[WRITTEN, "PASS"]).failing(FACTS.1);
  let round = Staged::run(session).await;

  assert_eq!(round.session.prompts().len(), 3, "the message turn and both stages were asked");
  assert!(
    round.sent().is_empty(),
    "no message goes out past a stage it did not pass: {:?}",
    round.sent()
  );
  assert!(round.log().await.contains(&"stage facts failed".to_owned()), "{:?}", round.log().await);
}

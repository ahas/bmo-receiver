//! A room's round carried through its chain of instruction stages (docs/instruction.md, 「The stages before the
//! send」): the message turn writes the message, each stage is one system instruction carrying its own words over
//! the message as it stands, a stage that writes nothing leaves it as it was, a stage that writes any other
//! message, `PASS` among them, puts its whole answer in its place, and the answer after the last stage is what the
//! room receives.

use std::sync::Arc;
use std::time::Duration;

use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::conversation::Timings;
use bmo_receiver::chat::runner::Runner;
use bmo_receiver::chat::stages::audit;
use bmo_receiver::chat::stages::table::set_text;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::events::Events;
use bmo_receiver::receiver::{Context, Inbound, Room};
use receiver_policy::chat_line::CHAT_LINE;
use receiver_policy::fill_marks;
use receiver_policy::instruction::INSTRUCTION;
use receiver_ports::agency::Load;
use receiver_ports::brain::Known;
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

/// The owner's chat standing on a receiver that runs the audit and the instruction stages, its supervisor following
/// `chain` with the words `[names, facts]` set for those two stages, its session answering in turn with every stage's
/// turn shown: what the session was asked, what the room received, the round log's rows and the stream's news.
struct Staged {
  scratch: Scratch,
  _data: tempfile::TempDir,
  standing: Standing,
  runner: Runner,
  chat: Arc<StandInChat>,
  session: Arc<StandInSession>,
  events: Arc<Events>,
}

impl Staged {
  /// One round of the owner's words through the chain `[names, facts]`, the brain keeping no people.
  async fn run(session: StandInSession) -> Self {
    let staged = Staged::stand(session, &[NAMES.0, FACTS.0]).await;

    staged.say(1, ASKED).await;
    staged
  }

  async fn stand(session: StandInSession, chain: &[&str]) -> Self {
    let scratch = Scratch::new().await;

    for (name, text) in [NAMES, FACTS] {
      set_text(scratch.db(), AGENT, name, text).await.unwrap();
    }

    set_chain(scratch.db(), chain).await;

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

    Staged {
      scratch,
      _data: data,
      standing,
      runner,
      chat,
      session,
      events,
    }
  }

  // The owner's words heard in his chat, and the round they open run to its end.
  async fn say(&self, message_id: i64, text: &str) {
    let before = self.session.prompts().len();
    let receiver = self.standing.receiver.clone();

    receiver
      .message(&said(message_id, text), &Context::default(), owners_room())
      .await
      .unwrap();
    until("the round's end", || {
      self.runner.idle() && !self.session.working() && self.session.prompts().len() > before
    })
    .await;
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
async fn the_stages_run_in_order_an_empty_answer_keeps_the_message_and_the_last_answer_goes_out() {
  let round = Staged::run(StandInSession::new(SESSION, "unused").answering(&[WRITTEN, "", REVISED])).await;
  let prompts = round.session.prompts();

  assert_eq!(prompts.len(), 3, "the message turn and one turn a stage: {prompts:#?}");
  assert!(prompts[0].contains(ASKED), "the message turn carries the room's words");

  // Each stage is a system instruction carrying its own words.
  for (prompt, (name, text)) in prompts[1..].iter().zip([NAMES, FACTS]) {
    assert!(prompt.starts_with(&fill_marks(&INSTRUCTION.marker, &[("name", name)])), "{prompt}");
    assert!(prompt.contains(text), "the stage's words ride its turn: {prompt}");
  }

  // The message is the agent's own last one, so no stage turn carries it again, after an empty answer as well.
  for prompt in &prompts[1..] {
    assert!(!prompt.contains(WRITTEN), "{prompt}");
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
  let round = Staged::run(StandInSession::new(SESSION, "unused").answering(&[WRITTEN, "", WRITTEN])).await;

  assert_eq!(round.sent().len(), 1, "{:?}", round.sent());
  assert!(round.sent()[0].contains(WRITTEN), "{:?}", round.sent());
  assert_eq!(
    round.log().await,
    ["turn message answered", "stage names pass", "stage facts pass", "end  message"],
    "a stage that reprints the message unchanged passes it"
  );
}

#[tokio::test]
async fn a_stage_that_answers_pass_sends_pass_as_the_message() {
  let round = Staged::run(StandInSession::new(SESSION, "unused").answering(&[WRITTEN, "PASS", ""])).await;

  assert_eq!(round.sent().len(), 1, "{:?}", round.sent());
  assert!(round.sent()[0].contains("\"PASS\""), "PASS is the message's words: {:?}", round.sent());
  assert_eq!(
    round.log().await,
    ["turn message answered", "stage names revised", "stage facts pass", "end  message"]
  );
}

#[tokio::test]
async fn a_stage_whose_run_failed_sends_nothing() {
  let session = StandInSession::new(SESSION, "unused").answering(&[WRITTEN, ""]).failing(FACTS.1);
  let round = Staged::run(session).await;

  assert_eq!(round.session.prompts().len(), 3, "the message turn and both stages were asked");
  assert!(
    round.sent().is_empty(),
    "no message goes out past a stage it did not pass: {:?}",
    round.sent()
  );
  assert!(round.log().await.contains(&"stage facts failed".to_owned()), "{:?}", round.log().await);
}

// The audit's marker line, naming its stage.
fn audit_marker() -> String {
  fill_marks(&INSTRUCTION.marker, &[("name", &INSTRUCTION.audit.name)])
}

// The audit's line for the owner, out of the policy's line `line`.
fn owner_line(line: &str) -> String {
  fill_marks(line, &[("name", "tester"), ("id", &OWNER.to_string())])
}

#[tokio::test]
async fn a_person_the_brain_holds_nothing_of_is_asked_once_how_they_want_to_be_called() {
  let session = StandInSession::new(SESSION, "unused").answering(&[WRITTEN, REVISED, WRITTEN]);
  let staged = Staged::stand(session, &[]).await;

  staged.standing.brain.knowing(&[]);
  staged.say(1, ASKED).await;

  let prompts = staged.session.prompts();

  assert_eq!(prompts.len(), 2, "the message turn and the audit: {prompts:#?}");
  assert!(prompts[1].starts_with(&audit_marker()), "{}", prompts[1]);
  assert!(prompts[1].contains(&owner_line(&INSTRUCTION.audit.ask)), "{}", prompts[1]);
  assert!(
    staged.sent()[0].contains(REVISED),
    "the audit's revision is what goes out: {:?}",
    staged.sent()
  );
  assert_eq!(staged.log().await, ["turn message answered", "audit audit revised", "end  message"]);
  assert_eq!(staged.started()[1], ("audit".to_owned(), "system".to_owned()));

  // Asked once, the person is not asked again in a later round.
  staged.say(2, "and the tests?").await;
  assert_eq!(
    staged.session.prompts().len(),
    3,
    "the later round asks no audit: {:#?}",
    staged.session.prompts()
  );
}

#[tokio::test]
async fn a_person_whose_memory_the_session_holds_is_not_audited() {
  let staged = Staged::stand(StandInSession::new(SESSION, "unused").answering(&[WRITTEN]), &[]).await;
  let held = Known {
    held: true,
    named: true,
  };

  staged.standing.brain.knowing(&[(&OWNER.to_string(), held)]);
  staged.standing.brain.about.lock().unwrap().insert(77, vec![OWNER.to_string()]);
  staged.standing.agency.loads.lock().unwrap().insert(
    SESSION.to_owned(),
    vec![Load {
      path: "memories/77".to_owned(),
      version: "1".to_owned(),
    }],
  );
  staged.say(1, ASKED).await;

  assert_eq!(staged.session.prompts().len(), 1, "{:#?}", staged.session.prompts());
  assert!(staged.sent()[0].contains(WRITTEN), "{:?}", staged.sent());
}

#[tokio::test]
async fn a_named_person_the_session_read_nothing_of_is_sent_to_their_records_and_never_asked() {
  let session = StandInSession::new(SESSION, "unused").answering(&[WRITTEN, ""]);
  let staged = Staged::stand(session, &[]).await;
  let held = Known {
    held: true,
    named: true,
  };

  staged.standing.brain.knowing(&[(&OWNER.to_string(), held)]);
  staged.say(1, ASKED).await;

  let prompts = staged.session.prompts();

  assert!(prompts[1].contains(&owner_line(&INSTRUCTION.audit.read)), "{}", prompts[1]);
  assert!(!prompts[1].contains(&owner_line(&INSTRUCTION.audit.read_ask)), "{}", prompts[1]);
  assert!(staged.sent()[0].contains(WRITTEN), "a passed audit sends the message as written");
  assert!(audit::asked(staged.scratch.db(), AGENT).await.is_empty(), "nobody was asked");
}

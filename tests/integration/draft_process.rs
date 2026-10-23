//! A round's draft process carried whole (docs/drafting.md): the round's turn answers a draft, the questions its
//! row names follow on the same session, and the last answer is what goes back to the room; a skip in any
//! question sends nothing, a failed check is asked once, the ceiling sends what stands, a light turn is asked the
//! last question alone, words that came meanwhile ride the last question, and the code's readings are items of
//! the process rather than askings of their own.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::conversation::Timings;
use bmo_receiver::chat::runner::Runner;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::{Context, Inbound, Room};
use receiver_policy::chat_line::CHAT_LINE;
use receiver_policy::drafting::DRAFTING;
use receiver_policy::second_asks::SECOND_ASKS;

use crate::common::Scratch;
use crate::common::session::{StandInSession, drafting};
use crate::common::units::{
  ADDRESS, AGENT, OWNER, StandInChat, Standing, deployment, registration, unrun, woken,
};

const SESSION: &str = "agent-a@room";

/// Words long enough that a turn reading them is no light one.
const ASKED: &str = "what does the plan for this week hold";

fn owners_room() -> Room {
  Room {
    owner_room: true,
    identity: Some(7),
    private_to: None,
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
    reply_to_message_id: None,
    via: "road-a".into(),
    mode: Some("bot-account".into()),
    via_reader: None,
    album: None,
  }
}

fn timings() -> Timings {
  Timings {
    queue: Duration::from_millis(1),
    wait: Duration::from_secs(60),
    window: Some(Duration::from_secs(60)),
    look: Duration::from_millis(5),
  }
}

fn documents(data: &Path) {
  let chat = data.join("prompts").join("chat");

  std::fs::create_dir_all(&chat).unwrap();
  std::fs::write(chat.join(&CHAT_LINE.room.owner_document), "The owner's own chat.").unwrap();
  std::fs::write(chat.join(&CHAT_LINE.room.group_document), "A group room.").unwrap();
  std::fs::write(chat.join(&CHAT_LINE.capabilities.document), "What the agent can do.").unwrap();
}

fn fixture(name: &str) -> String {
  std::fs::read_to_string(format!("tests/fixtures/{name}")).unwrap().trim().to_owned()
}

fn question(name: &str) -> &'static str {
  DRAFTING.questions.get(name).map(String::as_str).unwrap()
}

async fn until(what: &str, holds: impl Fn() -> bool) {
  let waited = tokio::time::timeout(Duration::from_secs(20), async {
    while !holds() {
      tokio::time::sleep(Duration::from_millis(5)).await;
    }
  })
  .await;

  assert!(waited.is_ok(), "{what} never came");
}

/// The owner's chat with the chat agent's conversations standing on the stand-in units, its session answering
/// as the test scripts it.
struct Drafting {
  scratch: Scratch,
  _data: tempfile::TempDir,
  standing: Standing,
  chat: Arc<StandInChat>,
  session: Arc<StandInSession>,
  runner: Runner,
}

impl Drafting {
  async fn new(session: StandInSession, rounds: &[&str], light: &[&str], ceiling_seconds: u32) -> Self {
    let scratch = Scratch::new().await;
    let data = tempfile::tempdir().unwrap();
    let chat = Arc::new(StandInChat::new("road-a").owners(OWNER));

    documents(data.path());
    drafting(scratch.db(), rounds, light, ceiling_seconds).await;

    let standing = woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat.clone()], deployment()));
    let session = Arc::new(session);
    let receiver = standing.receiver.clone();
    let runner = Runner::new(receiver.clone(), Arc::new(Around::new(receiver.clone())), timings(), AGENT);

    standing
      .agency
      .registering(vec![registration(AGENT, "Agent A", vec![ADDRESS], None, true)]);
    standing.agency.carrying_on(session.clone());
    receiver.hold_runner(Arc::new(runner.clone()));

    Drafting {
      scratch,
      _data: data,
      standing,
      chat,
      session,
      runner,
    }
  }

  async fn hear(&self, message_id: i64, text: &str) {
    self
      .standing
      .receiver
      .message(&said(message_id, text), &Context::default(), owners_room())
      .await
      .unwrap();
  }

  async fn ended(&self, turns: usize) {
    until("the round's end", || {
      self.session.prompts().len() >= turns && self.runner.idle() && !self.session.working()
    })
    .await;
    // The round's last writes land after its turn ends.
    tokio::time::sleep(Duration::from_millis(100)).await;
  }

  fn sent(&self) -> Vec<String> {
    self.chat.sent().iter().map(|send| serde_json::to_string(send).unwrap()).collect()
  }

  // The round log, one line a row: its kind, its question and how it ended.
  async fn log(&self) -> Vec<String> {
    self
      .scratch
      .texts(r#"SELECT "kind" || ' ' || "question" || ' ' || "outcome" FROM "draft_rounds" ORDER BY "id""#)
      .await
  }
}

#[tokio::test]
async fn the_last_question_s_answer_is_the_message() {
  let session = StandInSession::new(SESSION, "unscripted").answering(&[
    "a draft",
    "notes on the intent",
    "what to read",
    "the message itself",
  ]);
  let round = Drafting::new(session, &["intent", "information", "final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  round.ended(4).await;

  let prompts = round.session.prompts();
  let sent = round.sent();

  assert_eq!(prompts.len(), 4, "the turn and its three questions");
  assert!(prompts[1].contains(question("intent")), "{}", prompts[1]);
  assert!(prompts[2].contains(question("information")));
  assert!(prompts[3].contains(question("final")));
  assert!(prompts[1].contains("1 of 3") && prompts[3].contains("3 of 3"));
  assert_eq!(sent.len(), 1, "one message goes out: {sent:?}");
  assert!(sent[0].contains("the message itself") && !sent[0].contains("a draft"), "{sent:?}");
  assert_eq!(
    round.log().await,
    [
      "turn turn answered",
      "question intent answered",
      "question information answered",
      "question final answered",
      "end  message",
    ]
  );

  let changed = round.scratch.texts(r#"SELECT "changed"::text FROM "draft_rounds" ORDER BY "id""#).await;

  assert_eq!(changed[..4], ["false", "true", "true", "true"]);
  assert_eq!(round.standing.heard.events("draft").len(), 5, "every row is told on the stream");
}

#[tokio::test]
async fn a_skip_in_the_second_question_sends_nothing() {
  // The owner's own chat owes an answer, so the skip is asked over once, and a skip again stands.
  let session =
    StandInSession::new(SESSION, "unscripted").answering(&["a draft", "notes", "<<SKIP>>", "<<SKIP>>"]);
  let round = Drafting::new(session, &["intent", "information", "final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  round.ended(4).await;

  let prompts = round.session.prompts();

  assert_eq!(prompts.len(), 4, "the last question is never asked: {prompts:#?}");
  assert!(!prompts.iter().any(|prompt| prompt.contains(question("final"))));
  assert!(
    prompts[3].contains(&SECOND_ASKS.errands.owed[..40]),
    "the owner's chat is asked over the skip"
  );
  assert!(round.sent().is_empty(), "nothing goes out");
  assert_eq!(
    round.log().await,
    [
      "turn turn answered",
      "question intent answered",
      "question information skip",
      "item owed skip",
      "end  skip",
    ]
  );
}

#[tokio::test]
async fn a_failed_check_is_asked_once_and_the_new_text_goes_out() {
  let long = "x".repeat(DRAFTING.max_chars + 1);
  let session =
    StandInSession::new(SESSION, "unscripted").answering(&["a draft", &long, "the short message"]);
  let round = Drafting::new(session, &["final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  round.ended(3).await;

  let prompts = round.session.prompts();
  let sent = round.sent();

  assert_eq!(prompts.len(), 3);
  assert!(prompts[2].contains(&DRAFTING.max_chars.to_string()), "the errand names the limit");
  assert_eq!(sent.len(), 1);
  assert!(sent[0].contains("the short message"));
  assert_eq!(
    round.log().await,
    ["turn turn answered", "question final answered", "check length answered", "end  message"]
  );
}

#[tokio::test]
async fn a_check_that_fails_twice_withholds_the_message() {
  let long = "x".repeat(DRAFTING.max_chars + 1);
  let session = StandInSession::new(SESSION, "unscripted").answering(&["a draft", &long, &long]);
  let round = Drafting::new(session, &["final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  round.ended(3).await;

  assert_eq!(round.session.prompts().len(), 3, "the check is asked once");
  assert!(round.sent().is_empty(), "nothing goes out");

  let end = round
    .scratch
    .texts(r#"SELECT "outcome" || ' ' || "detail" FROM "draft_rounds" WHERE "kind" = 'end'"#)
    .await;

  assert_eq!(end.len(), 1);
  assert!(end[0].starts_with("withheld ") && end[0].ends_with("length"), "{end:?}");
}

#[tokio::test]
async fn past_the_ceiling_what_stands_is_sent() {
  let session = StandInSession::new(SESSION, "unscripted")
    .answering(&["the draft as it stands", "notes", "more notes", "never asked"])
    .working_for(Duration::from_millis(600));
  let round = Drafting::new(session, &["intent", "information", "final"], &["final"], 1).await;

  round.hear(1, ASKED).await;
  round.ended(3).await;

  let sent = round.sent();

  assert_eq!(round.session.prompts().len(), 3, "no question starts past the ceiling");
  assert_eq!(sent.len(), 1);
  assert!(sent[0].contains("the draft as it stands"), "the turn's own draft goes out: {sent:?}");
  assert_eq!(
    round.log().await,
    [
      "turn turn answered",
      "question intent answered",
      "question information answered",
      "end  ceiling-message"
    ]
  );
}

#[tokio::test]
async fn a_greeting_is_asked_the_last_question_alone() {
  let session = StandInSession::new(SESSION, "unscripted").answering(&["a draft", "hello to you too"]);
  let round = Drafting::new(session, &["intent", "information", "final"], &["final"], 150).await;

  round.hear(1, "hi there").await;
  round.ended(2).await;

  let prompts = round.session.prompts();
  let sent = round.sent();

  assert_eq!(prompts.len(), 2, "the turn and the last question");
  assert!(prompts[1].contains(question("final")) && prompts[1].contains("1 of 1"));
  assert!(sent.len() == 1 && sent[0].contains("hello to you too"), "{sent:?}");
}

#[tokio::test]
async fn words_that_came_between_questions_ride_the_last_one() {
  const BETWEEN: &str = "and one more thing said between the questions";

  let session = StandInSession::new(SESSION, "unscripted")
    .answering(&["a draft", "notes", "the message that takes both"])
    .holding_turn(2);
  let round = Drafting::new(session, &["intent", "final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  until("the first question", || round.session.prompts().len() == 2 && round.session.working()).await;
  round.hear(2, BETWEEN).await;
  tokio::time::sleep(Duration::from_millis(50)).await;
  round.session.let_go();
  round.ended(3).await;

  let prompts = round.session.prompts();

  assert!(
    round.standing.agency.pushes.lock().unwrap().is_empty(),
    "no words are pushed into a question"
  );
  assert!(!prompts[1].contains(BETWEEN), "the first question does not carry them");
  assert!(prompts[2].contains(BETWEEN), "the last question carries them: {}", prompts[2]);
  assert_eq!(prompts.len(), 3, "the words call no round of their own");
  assert_eq!(round.sent().len(), 1);
}

#[tokio::test]
async fn a_promise_is_an_item_of_the_process_and_no_asking_of_its_own() {
  let promise = fixture("promises-waiting.txt");
  let session =
    StandInSession::new(SESSION, "unscripted").answering(&[&promise, "the message, with the watch set"]);
  let round = Drafting::new(session, &[], &[], 150).await;

  round.hear(1, ASKED).await;
  round.ended(2).await;

  let prompts = round.session.prompts();
  let sent = round.sent();

  assert_eq!(prompts.len(), 2);
  assert!(prompts[1].contains(&SECOND_ASKS.errands.wake[..40]), "{}", prompts[1]);
  assert!(sent.len() == 1 && sent[0].contains("the message, with the watch set"), "{sent:?}");
  assert_eq!(round.log().await, ["turn turn answered", "item wake answered", "end  message"]);

  let stages: Vec<String> = round
    .standing
    .heard
    .events("judgement")
    .iter()
    .map(|event| event["stage"].as_str().unwrap_or_default().to_owned())
    .collect();

  assert!(
    !stages.iter().any(|stage| stage.ends_with("-again")),
    "no second asking writes a judgement of its own: {stages:?}"
  );
}

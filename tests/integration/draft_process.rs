//! A round's draft process carried whole (docs/drafting.md): the round's turn carries the first question its row
//! names, the rest follow on the same session, every answer before the last is notes and the last is what goes
//! back to the room; a send in the first round is the message at once, a skip in any question sends nothing, a
//! failed check is asked once, past the ceiling `final` is asked at once or nothing goes out, a light turn is
//! asked the last question alone in its own turn, words that came meanwhile are never pushed and stand under the
//! next question or the closing check, each answered with the message, marked done or marked separate, and the
//! code's readings are items of the process rather than askings of their own.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::conversation::Timings;
use bmo_receiver::chat::draft::table::PastCeiling;
use bmo_receiver::chat::runner::Runner;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::{Context, Inbound, Room};
use receiver_policy::chat_line::CHAT_LINE;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::drafting::DRAFTING;
use receiver_policy::second_asks::SECOND_ASKS;

use crate::common::Scratch;
use crate::common::session::{StandInSession, drafting, drafting_past, drafting_sending};
use crate::common::units::{
  ADDRESS, AGENT, OWNER, StandInChat, Standing, deployment, registration, unrun, woken,
};

const SESSION: &str = "agent-a@room";

/// The number the chat agent speaks under in the owner's chat, as the room's road says.
const IDENTITY: i64 = 7;

/// Words long enough that a turn reading them is no light one.
const ASKED: &str = "what does the plan for this week hold";

fn owners_room() -> Room {
  Room {
    owner_room: true,
    identity: Some(IDENTITY),
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

// The fixed head of the opening the first question rides the round's turn on.
fn opening_head() -> &'static str {
  DRAFTING.opening.split('{').next().unwrap_or_default()
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
    let chat = Arc::new(StandInChat::new("road-a").owners(OWNER).agent_in(OWNER, IDENTITY));

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
async fn the_first_question_rides_the_turn_and_only_the_last_answer_goes_out() {
  let session = StandInSession::new(SESSION, "unscripted")
    .answering(&["notes on the intent", "what was read and established", "the message itself"])
    .holding_turn(3);
  let round = Drafting::new(session, &["intent", "information", "final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  until("the last question", || round.session.prompts().len() == 3 && round.session.working()).await;
  assert!(round.sent().is_empty(), "nothing goes out before the last question answered");
  round.session.let_go();
  round.ended(3).await;

  let prompts = round.session.prompts();
  let sent = round.sent();

  assert_eq!(prompts.len(), 3, "the round's turn carried the first of three questions");
  assert!(prompts[0].contains(ASKED) && prompts[0].contains(question("intent")), "{}", prompts[0]);
  assert!(prompts[0].contains(opening_head()), "the first question opens on the owner's opening");
  assert!(
    prompts[0].contains(&CHAT_ROUND.closing.questioned_waiting)
      && !prompts[0].contains(&CHAT_ROUND.closing.waiting),
    "the turn's closing asks for the question's answer, not the message"
  );
  assert!(prompts[1].contains(question("information")) && prompts[1].contains("2 of 3"));
  assert!(prompts[2].contains(question("final")) && prompts[2].contains("3 of 3"));
  assert_eq!(sent.len(), 1, "one message goes out: {sent:?}");
  assert!(sent[0].contains("the message itself") && !sent[0].contains("notes"), "{sent:?}");
  assert_eq!(
    round.log().await,
    [
      "question intent answered",
      "question information answered",
      "question final answered",
      "end  message",
    ]
  );

  let numbers = round.scratch.texts(r#"SELECT "number"::text FROM "draft_rounds" ORDER BY "id""#).await;
  let notes = round
    .scratch
    .texts(r#"SELECT "answer" FROM "draft_rounds" WHERE "kind" = 'question' ORDER BY "id""#)
    .await;

  assert_eq!(numbers, ["1", "2", "3", "4"]);
  assert_eq!(
    notes[..2],
    ["notes on the intent", "what was read and established"],
    "each round's notes are kept"
  );

  let events = round.standing.heard.events("draft");
  let starts: Vec<(i64, i64, String)> = events
    .iter()
    .filter(|event| event["kind"] == "start")
    .map(|event| {
      (
        event["number"].as_i64().unwrap_or_default(),
        event["count"].as_i64().unwrap_or_default(),
        event["question"].as_str().unwrap_or_default().to_owned(),
      )
    })
    .collect();

  assert_eq!(
    starts,
    [
      (1, 3, "intent".to_owned()),
      (2, 3, "information".to_owned()),
      (3, 3, "final".to_owned())
    ],
    "every question is told as it starts"
  );
  assert_eq!(events.len(), 7, "every row is told on the stream beside the starts");

  // The gate's `draft` stage reads every row event by these fields, and lets a start go by its kind.
  let fields = |event: &serde_json::Value| -> Vec<String> {
    let mut held: Vec<String> = event.as_object().unwrap().keys().cloned().collect();

    held.retain(|key| key != "speaker");
    held.sort();
    held
  };

  for event in &events {
    let expected: &[&str] = if event["kind"] == "start" {
      &[
        "agentName",
        "at",
        "chatId",
        "count",
        "kind",
        "messageId",
        "number",
        "question",
        "round",
        "type",
      ]
    } else {
      &[
        "agentName",
        "answerChars",
        "at",
        "changed",
        "chatId",
        "detail",
        "kind",
        "messageId",
        "ms",
        "number",
        "outcome",
        "question",
        "reads",
        "round",
        "type",
      ]
    };

    assert_eq!(fields(event), expected, "{event}");
  }
}

#[tokio::test]
async fn a_send_in_the_first_round_is_the_message_and_nothing_more_is_asked() {
  let session = StandInSession::new(SESSION, "unscripted")
    .answering(&["<<SEND>> the answer, ready at once", "never asked"]);
  let round = Drafting::new(session, &["intent", "information", "final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  round.ended(1).await;

  let sent = round.sent();

  assert_eq!(round.session.prompts().len(), 1, "no later question is asked");
  assert!(sent.len() == 1 && sent[0].contains("the answer, ready at once"), "{sent:?}");
  assert!(!sent[0].contains("<<SEND>>"), "the marker never reaches the room: {sent:?}");
  assert_eq!(round.log().await, ["question intent send", "end  message"]);
}

#[tokio::test]
async fn a_send_before_the_question_a_send_is_read_from_is_notes_and_the_next_question_is_asked() {
  let session = StandInSession::new(SESSION, "unscripted").answering(&[
    "<<SEND>> too soon",
    "<<SEND>> the answer, once the facts are read",
    "never asked",
  ]);
  let rounds = ["information", "facts", "final"];
  let round = Drafting::new(session, &rounds, &["final"], 150).await;

  drafting_sending(round.scratch.db(), &rounds, "facts", &["final"], 150).await;
  round.hear(1, ASKED).await;
  round.ended(2).await;

  let prompts = round.session.prompts();
  let sent = round.sent();

  assert_eq!(prompts.len(), 2, "facts is asked after the early send: {prompts:#?}");
  assert!(
    prompts[0].contains("before question 2 (facts)") && !prompts[0].contains(&DRAFTING.voice),
    "the first question says no message is written before facts, and asks for notes: {}",
    prompts[0]
  );
  let open = receiver_policy::fill_marks(&DRAFTING.send_open, &[("voice", &DRAFTING.voice)]);
  assert!(
    prompts[1].contains(&open) && prompts[1].contains(&DRAFTING.voice),
    "facts reads a send, written in the room's voice: {}",
    prompts[1]
  );
  assert!(
    sent.len() == 1 && sent[0].contains("once the facts are read") && !sent[0].contains("too soon"),
    "{sent:?}"
  );
  assert_eq!(
    round.log().await,
    ["question information answered", "question facts send", "end  message"]
  );
}

#[tokio::test]
async fn a_skip_in_the_second_question_sends_nothing() {
  // The owner's own chat owes an answer, so the skip is asked over once, and a skip again stands.
  let session = StandInSession::new(SESSION, "unscripted").answering(&["notes", "<<SKIP>>", "<<SKIP>>"]);
  let round = Drafting::new(session, &["intent", "information", "final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  round.ended(3).await;

  let prompts = round.session.prompts();

  assert_eq!(prompts.len(), 3, "the last question is never asked: {prompts:#?}");
  assert!(!prompts.iter().any(|prompt| prompt.contains(question("final"))));
  assert!(
    prompts[2].contains(&SECOND_ASKS.errands.owed[..40]),
    "the owner's chat is asked over the skip"
  );
  assert!(round.sent().is_empty(), "nothing goes out");
  assert_eq!(
    round.log().await,
    ["question intent answered", "question information skip", "item owed skip", "end  skip",]
  );
}

#[tokio::test]
async fn a_failed_check_is_asked_once_and_the_new_text_goes_out() {
  let long = "x".repeat(DRAFTING.max_chars + 1);
  let session = StandInSession::new(SESSION, "unscripted").answering(&[&long, "the short message"]);
  let round = Drafting::new(session, &["final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  round.ended(2).await;

  let prompts = round.session.prompts();
  let sent = round.sent();

  assert_eq!(prompts.len(), 2);
  assert!(prompts[1].contains(&DRAFTING.max_chars.to_string()), "the errand names the limit");
  assert_eq!(sent.len(), 1);
  assert!(sent[0].contains("the short message"));
  assert_eq!(round.log().await, ["question final answered", "check length answered", "end  message"]);
}

#[tokio::test]
async fn a_check_that_fails_twice_withholds_the_message() {
  let long = "x".repeat(DRAFTING.max_chars + 1);
  let session = StandInSession::new(SESSION, "unscripted").answering(&[&long, &long]);
  let round = Drafting::new(session, &["final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  round.ended(2).await;

  assert_eq!(round.session.prompts().len(), 2, "the check is asked once");
  assert!(round.sent().is_empty(), "nothing goes out");

  let end = round
    .scratch
    .texts(r#"SELECT "outcome" || ' ' || "detail" FROM "draft_rounds" WHERE "kind" = 'end'"#)
    .await;

  assert_eq!(end.len(), 1);
  assert!(end[0].starts_with("withheld ") && end[0].ends_with("length"), "{end:?}");
}

#[tokio::test]
async fn a_korean_answer_to_a_korean_speaker_is_never_asked_again() {
  let answer = fixture("answer-korean.txt");
  let session = StandInSession::new(SESSION, "unscripted").answering(&[&answer, "never asked"]);
  let round = Drafting::new(session, &["final"], &["final"], 150).await;

  round.hear(1, &fixture("addressing-korean.txt")).await;
  round.ended(1).await;

  let prompts = round.session.prompts();

  assert_eq!(prompts.len(), 1, "nothing is asked over the answer: {prompts:#?}");
  assert!(prompts[0].contains("tester: ko"), "the line names the speaker's own language");
  assert_eq!(round.sent().len(), 1);
  assert!(round.sent()[0].contains(&answer));
  assert_eq!(round.log().await, ["question final answered", "end  message"]);
}

#[tokio::test]
async fn a_second_answer_in_none_of_the_peoples_languages_lets_the_first_go_out() {
  let first = fixture("answer-korean.txt");
  let second = fixture("addressing-korean.txt");
  let session = StandInSession::new(SESSION, "unscripted").answering(&[&first, &second]);
  let round = Drafting::new(session, &["final"], &["final"], 150).await;

  // The owner writes English prose, so an answer in Korean alone holds none of the owner's language.
  round.hear(1, ASKED).await;
  round.ended(2).await;

  let prompts = round.session.prompts();
  let sent = round.sent();

  assert_eq!(prompts.len(), 2, "the language check is asked once: {prompts:#?}");
  assert!(prompts[1].contains("tester: en"), "the errand names each person with their language");
  assert_eq!(sent.len(), 1, "the answer is never withheld for its language");
  assert!(sent[0].contains(&first) && !sent[0].contains(&second), "{sent:?}");
  assert_eq!(
    round.log().await,
    ["question final answered", "check language answered", "end  message"]
  );
}

#[tokio::test]
async fn past_the_ceiling_before_final_final_is_asked_at_once() {
  let session = StandInSession::new(SESSION, "unscripted")
    .answering(&["notes", "more notes", "the message final wrote", "never asked"])
    .working_for(Duration::from_millis(600));
  let round = Drafting::new(session, &["intent", "information", "facts", "final"], &["final"], 1).await;

  round.hear(1, ASKED).await;
  round.ended(3).await;

  let prompts = round.session.prompts();
  let sent = round.sent();

  assert_eq!(prompts.len(), 3, "the questions past the ceiling are passed over to final");
  assert!(
    prompts[2].contains(question("final"))
      && !prompts.iter().any(|prompt| prompt.contains(question("facts")))
  );
  assert_eq!(sent.len(), 1);
  assert!(sent[0].contains("the message final wrote"), "final's answer goes out: {sent:?}");
  assert_eq!(
    round.log().await,
    [
      "question intent answered",
      "question information answered",
      "question final answered",
      "end  ceiling-message"
    ]
  );
}

#[tokio::test]
async fn past_the_ceiling_before_final_a_skip_row_sends_nothing() {
  let rounds = ["intent", "information", "final"];
  // The owner's own chat owes an answer, so the ceiling's silence is asked over once.
  let session = StandInSession::new(SESSION, "unscripted")
    .answering(&["notes", "<<SKIP>>", "never asked"])
    .working_for(Duration::from_millis(1200));
  let round = Drafting::new(session, &rounds, &["final"], 1).await;

  drafting_past(round.scratch.db(), &rounds, &["final"], 1, PastCeiling::Skip).await;
  round.hear(1, ASKED).await;
  round.ended(2).await;

  let prompts = round.session.prompts();

  assert_eq!(prompts.len(), 2, "no question is asked past the ceiling: {prompts:#?}");
  assert!(!prompts.iter().any(|prompt| prompt.contains(question("final"))));
  assert!(round.sent().is_empty(), "nothing goes out");
  assert_eq!(round.log().await, ["question intent answered", "item owed skip", "end  ceiling-skip"]);
}

#[tokio::test]
async fn a_greeting_is_asked_the_last_question_alone_in_its_own_turn() {
  let session = StandInSession::new(SESSION, "unscripted").answering(&["hello to you too"]);
  let round = Drafting::new(session, &["intent", "information", "final"], &["final"], 150).await;

  round.hear(1, "hi there").await;
  round.ended(1).await;

  let prompts = round.session.prompts();
  let sent = round.sent();

  assert_eq!(prompts.len(), 1, "the round's turn is the last question");
  assert!(prompts[0].contains(question("final")) && prompts[0].contains("1 of 1"));
  assert!(prompts[0].contains(&CHAT_ROUND.closing.waiting), "its closing asks the message as ever");
  assert!(sent.len() == 1 && sent[0].contains("hello to you too"), "{sent:?}");
  assert_eq!(round.log().await, ["question final answered", "end  message"]);
}

#[tokio::test]
async fn words_that_came_while_the_second_question_worked_stand_under_the_third() {
  const BETWEEN: &str = "and one more thing said between the questions";

  let session = StandInSession::new(SESSION, "unscripted")
    .answering(&["notes", "more notes", "the message that takes both"])
    .holding_turn(2);
  let round = Drafting::new(session, &["intent", "information", "final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  until("the second question", || round.session.prompts().len() == 2 && round.session.working()).await;
  round.hear(2, BETWEEN).await;
  tokio::time::sleep(Duration::from_millis(50)).await;
  round.session.let_go();
  round.ended(3).await;

  let prompts = round.session.prompts();

  assert!(
    round.standing.agency.pushes.lock().unwrap().is_empty(),
    "no words are pushed into a later question"
  );
  assert!(!prompts[1].contains(BETWEEN), "the second question does not carry them");
  assert!(prompts[2].contains(BETWEEN), "the third question carries them: {}", prompts[2]);
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

#[tokio::test]
async fn every_event_of_the_chat_agent_s_round_names_it_as_the_room_s_own_voice() {
  let session = StandInSession::new(SESSION, "unscripted")
    .answering(&["notes on the intent", "the message itself"])
    .holding_turn(2);
  let round = Drafting::new(session, &["intent", "final"], &[], 150).await;

  round.hear(1, ASKED).await;
  until("the last question", || round.session.prompts().len() == 2 && round.session.working()).await;
  round.session.let_go();
  round.ended(2).await;

  let speaker = serde_json::json!({ "id": IDENTITY, "name": AGENT });

  for kind in ["run-started", "sent", "draft"] {
    let events = round.standing.heard.events(kind);

    assert!(!events.is_empty(), "the round tells {kind}");

    for event in events {
      assert_eq!(
        event["speaker"], speaker,
        "{kind} names the chat agent by its name and the room's identity"
      );
    }
  }
}

/// Words short enough that a round taking them alone is a light one.
const SHORT: &str = "and this?";

// The round's own turn held working while `SHORT` comes into the room, then let go once it waits there.
async fn said_during_the_first(round: &Drafting) {
  round.hear(1, ASKED).await;
  until("the round's turn", || round.session.prompts().len() == 1 && round.session.working()).await;
  round.hear(2, SHORT).await;
  // The words stand in the conversation, past the queue that gathers them, before the turn is let go.
  until("the words waiting", || round.runner.of(AGENT).buffered(OWNER) == [2]).await;
  // The working turn is looked at meanwhile, and nothing is pushed into it.
  tokio::time::sleep(Duration::from_millis(50)).await;
  round.session.let_go();
}

#[tokio::test]
async fn words_said_during_the_first_question_are_not_pushed_and_stand_under_the_second() {
  let session = StandInSession::new(SESSION, "unscripted")
    .answering(&["notes", "more notes", "the message that takes both"])
    .holding_turn(1);
  let round = Drafting::new(session, &["intent", "information", "final"], &["final"], 150).await;

  said_during_the_first(&round).await;
  round.ended(3).await;

  let prompts = round.session.prompts();
  let sent = round.sent();

  assert!(
    round.standing.agency.pushes.lock().unwrap().is_empty(),
    "no room's words go into a working turn"
  );
  assert!(!prompts[0].contains(SHORT), "the round's own turn does not carry them");
  assert!(prompts[1].contains(SHORT), "the second question carries them: {}", prompts[1]);
  assert!(
    prompts[0].contains("<<DONE #<number>>>"),
    "the room's block names the marks: {}",
    prompts[0]
  );
  assert!(!prompts[1].contains("<<DONE #<number>>>"), "a question's lead names only its place");
  assert!(!prompts[2].contains(SHORT), "a block is shown once");
  assert_eq!(prompts.len(), 3, "the words shown call no round of their own");
  assert!(sent.len() == 1 && sent[0].contains("the message that takes both"), "{sent:?}");
}

#[tokio::test]
async fn words_marked_done_are_read_and_answered_by_nothing() {
  let session = StandInSession::new(SESSION, "unscripted")
    .answering(&["notes", "<<DONE #2>> the message, the call needing nothing"])
    .holding_turn(1);
  let round = Drafting::new(session, &["intent", "final"], &["final"], 150).await;

  said_during_the_first(&round).await;
  round.ended(2).await;

  let prompts = round.session.prompts();
  let sent = round.sent();

  assert!(prompts[1].contains(SHORT), "the last question shows them: {}", prompts[1]);
  assert_eq!(prompts.len(), 2, "words marked done call no round of their own");
  assert_eq!(sent.len(), 1, "{sent:?}");
  assert!(
    sent[0].contains("the message, the call needing nothing") && !sent[0].contains("DONE"),
    "{sent:?}"
  );
  assert!(round.runner.of(AGENT).pending(OWNER).is_empty(), "nothing waits after the round");
}

#[tokio::test]
async fn words_marked_separate_open_a_round_of_their_own_after_the_message() {
  let session = StandInSession::new(SESSION, "unscripted")
    .answering(&["notes", "<<SEPARATE #2>> the message for the first words", "the answer to the second"])
    .holding_turn(1);
  let round = Drafting::new(session, &["intent", "final"], &["final"], 150).await;

  said_during_the_first(&round).await;
  round.ended(3).await;

  let prompts = round.session.prompts();
  let sent = round.sent();

  assert!(prompts[1].contains(SHORT), "the last question shows them: {}", prompts[1]);
  assert_eq!(prompts.len(), 3, "the words marked separate open one round of their own, a light one");
  assert!(
    prompts[2].contains(SHORT)
      && !prompts[2]
        .contains(&receiver_policy::fill_marks(&DRAFTING.arrived, &[("voice", &DRAFTING.voice)])),
    "{}",
    prompts[2]
  );
  assert_eq!(sent.len(), 2, "{sent:?}");
  assert!(
    sent[0].contains("the message for the first words") && !sent[0].contains("SEPARATE"),
    "{sent:?}"
  );
  assert!(sent[1].contains("the answer to the second"), "{sent:?}");
}

#[tokio::test]
async fn words_said_during_the_last_question_stand_in_the_closing_check_before_the_message_goes_out() {
  let session = StandInSession::new(SESSION, "unscripted")
    .answering(&["notes", "the first message", "<<DONE #2>>"])
    .holding_turn(2);
  let round = Drafting::new(session, &["intent", "final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  until("the last question", || round.session.prompts().len() == 2 && round.session.working()).await;
  round.hear(2, SHORT).await;
  // The words stand in the conversation, past the queue that gathers them, before the turn is let go.
  until("the words waiting", || round.runner.of(AGENT).buffered(OWNER) == [2]).await;
  round.session.let_go();
  round.ended(3).await;

  let prompts = round.session.prompts();
  let sent = round.sent();

  assert_eq!(prompts.len(), 3, "the closing check is one more turn, and the words open no round");
  assert!(
    prompts[2].contains(&receiver_policy::fill_marks(&DRAFTING.arrived, &[("voice", &DRAFTING.voice)]))
      && prompts[2].contains(SHORT),
    "{}",
    prompts[2]
  );
  assert!(
    sent.len() == 1 && sent[0].contains("the first message"),
    "an answer of marks alone keeps the message: {sent:?}"
  );
  assert_eq!(
    round.log().await,
    [
      "question intent answered",
      "question final answered",
      "item arrived answered",
      "end  message"
    ]
  );
}

// The last question held working while `SHORT` comes into the room, then the closing check answered as scripted.
async fn said_during_the_last(round: &Drafting) {
  round.hear(1, ASKED).await;
  until("the last question", || round.session.prompts().len() == 2 && round.session.working()).await;
  round.hear(2, SHORT).await;
  // The words stand in the conversation, past the queue that gathers them, before the turn is let go.
  until("the words waiting", || round.runner.of(AGENT).buffered(OWNER) == [2]).await;
  round.session.let_go();
  round.ended(3).await;
}

#[tokio::test]
async fn the_closing_checks_answer_is_the_whole_message_in_the_standing_ones_place() {
  let session = StandInSession::new(SESSION, "unscripted")
    .answering(&["notes", "the first message", "the first message, and the newest words answered"])
    .holding_turn(2);
  let round = Drafting::new(session, &["intent", "final"], &["final"], 150).await;

  said_during_the_last(&round).await;

  let prompts = round.session.prompts();
  let sent = round.sent();

  assert!(
    prompts[2].contains(&DRAFTING.standing) && prompts[2].contains("the first message"),
    "the closing check shows the message as it stands: {}",
    prompts[2]
  );
  assert_eq!(sent.len(), 1, "{sent:?}");
  assert!(sent[0].contains("the first message, and the newest words answered"), "{sent:?}");
  assert_eq!(sent[0].matches("the first message").count(), 1, "nothing is joined: {sent:?}");
}

#[tokio::test]
async fn the_live_pair_goes_out_once_as_the_closing_check_wrote_it() {
  // The standing message and the closing check's answer as the store held them for one message, the same
  // sentence but for one mark: the answer alone goes out.
  let (standing, answer) = (fixture("closing-standing.txt"), fixture("closing-answer.txt"));
  let session = StandInSession::new(SESSION, "unscripted")
    .answering(&["notes", &standing, &answer])
    .holding_turn(2);
  let round = Drafting::new(session, &["intent", "final"], &["final"], 150).await;

  // The people write Korean, as the room's did, so the answers stand in their language.
  round.hear(1, &fixture("addressing-korean.txt")).await;
  until("the last question", || round.session.prompts().len() == 2 && round.session.working()).await;
  round.hear(2, &fixture("addressing-korean.txt")).await;
  // The words stand in the conversation, past the queue that gathers them, before the turn is let go.
  until("the words waiting", || round.runner.of(AGENT).buffered(OWNER) == [2]).await;
  round.session.let_go();
  round.ended(3).await;

  let sent = round.sent();
  let texts: Vec<String> = sent
    .iter()
    .map(|send| {
      serde_json::from_str::<serde_json::Value>(send).unwrap()["fields"]["text"]
        .as_str()
        .unwrap()
        .to_owned()
    })
    .collect();

  assert_eq!(texts, [answer], "the second text alone goes out");
}

#[tokio::test]
async fn a_turn_past_its_bound_tells_the_room_nothing_and_goes_on() {
  let session = StandInSession::new(SESSION, "unscripted")
    .answering(&["notes", "the message after a long while"])
    .working_for(Duration::from_millis(1300));
  let round = Drafting::new(session, &["intent", "final"], &["final"], 150).await;

  crate::common::session::drafting_turn_bounded(
    round.scratch.db(),
    &["intent", "final"],
    &["final"],
    150,
    1,
  )
  .await;
  round.hear(1, ASKED).await;
  round.ended(2).await;

  let sent = round.sent();

  // Both turns ran past the bound, and the room heard nothing but the message: no holding line in any room.
  assert_eq!(sent.len(), 1, "{sent:?}");
  assert!(sent[0].contains("the message after a long while"), "the turn went on: {sent:?}");
}

#[tokio::test]
async fn the_end_row_names_the_core_the_turn_carried_and_the_pieces_the_agent_read_itself() {
  use bmo_receiver::js::bun_hash;
  use receiver_ports::agency::Load;
  use receiver_ports::gate::{Entry, IndexEntry, IndexGroup, Source};

  let session = StandInSession::new(SESSION, "the message itself").holding_the_first();
  let round = Drafting::new(session, &["final"], &["final"], 150).await;
  let entry = |path: &str, text: &str, source: Source| Entry {
    path: path.to_owned(),
    text: text.to_owned(),
    source,
    fragment: None,
  };
  let load = |path: &str, version: &str| Load {
    path: path.to_owned(),
    version: version.to_owned(),
  };

  *round.standing.gate.entries.lock().unwrap() = vec![
    entry("rooms/42/people/42/sheet.md", "BODY OF A PERSON", Source::Person),
    entry("memories/42", "BODY OF THE DUE MEMORIES", Source::Memory),
  ];
  round.standing.gate.index.lock().unwrap().insert(
    OWNER,
    vec![IndexGroup {
      group: "knowledge".to_owned(),
      unread: 1,
      hint: None,
      entries: vec![IndexEntry {
        slug: "plans".to_owned(),
        holds: "the plan for the week".to_owned(),
        path: "knowledge/plans".to_owned(),
      }],
    }],
  );
  // The session already holds the person's sheet head at this version, and an earlier piece it read.
  round.standing.agency.loads.lock().unwrap().insert(
    SESSION.to_owned(),
    vec![
      load("rooms/42/people/42/sheet.md", &bun_hash("BODY OF A PERSON").to_string()),
      load("knowledge/older", "v1"),
    ],
  );

  round.hear(1, ASKED).await;
  until("the round's turn", || round.session.working()).await;
  // The agent reads a piece through the index while its turn works.
  round
    .standing
    .agency
    .loads
    .lock()
    .unwrap()
    .get_mut(SESSION)
    .unwrap()
    .push(load("knowledge/plans", "v1"));
  round.session.let_go();
  round.ended(1).await;

  let reads = round.scratch.texts(r#"SELECT "reads" FROM "draft_rounds" WHERE "kind" = 'end'"#).await;
  let reads: serde_json::Value = serde_json::from_str(&reads[0]).unwrap();

  assert_eq!(
    reads,
    serde_json::json!([
      {"path": "memories/42", "holds": null, "how": "attached"},
      {"path": "rooms/42/people/42/sheet.md", "holds": null, "how": "held"},
      {"path": "knowledge/plans", "holds": "the plan for the week", "how": "read"},
    ]),
    "the core first, then what the agent read itself, named by the index; what it held before is no read"
  );

  let others = round
    .scratch
    .texts(r#"SELECT coalesce("reads", 'none') FROM "draft_rounds" WHERE "kind" <> 'end'"#)
    .await;

  assert!(others.iter().all(|row| row == "none"), "{others:?}");
}

#[tokio::test]
async fn the_answers_pin_stands_on_the_rooms_record_as_on_the_road() {
  let session = StandInSession::new(SESSION, "unscripted").answering(&["the message itself"]);
  let round = Drafting::new(session, &["final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  round.ended(1).await;

  let sent = round.sent();

  assert_eq!(sent.len(), 1, "{sent:?}");
  assert!(sent[0].contains("\"replyToMessageId\":1"), "the road pins it: {sent:?}");

  let kept = round
    .scratch
    .texts(
      r#"SELECT coalesce("reply_to_message_id"::text, 'none') FROM "messages" WHERE "text" = 'the message itself'"#,
    )
    .await;

  assert_eq!(kept, ["1"], "the record names the message the answer is pinned to");
}

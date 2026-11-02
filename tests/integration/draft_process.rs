//! A round's draft process carried whole (docs/drafting.md): the round's turn carries the first question its row
//! names, the rest follow on the same session, every answer before the last is notes and the last is what goes
//! back to the room; a send in the first round is the message at once, a skip in any question sends nothing, a
//! failed check is asked once and the repaired text goes out, a light turn is
//! asked the last question alone in its own turn, words that came meanwhile are pushed into the working turn where
//! it takes them and otherwise stand under the next question or the closing check, each answered with the message,
//! marked done or marked separate, and the code's readings are items of the process rather than askings of their
//! own.

use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use bmo_chat_client::MessageFile;
use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::conversation::Timings;
use bmo_receiver::chat::instructions::{Queued, deliver_pending, queue, requeue_interrupted};
use bmo_receiver::chat::mid_turn::{Push, paragraph};
use bmo_receiver::chat::runner::Runner;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::now_ms;
use bmo_receiver::receiver::{Context, Inbound, Room};
use image::{ImageFormat, Rgb, RgbImage};
use receiver_policy::chat_line::CHAT_LINE;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::drafting::DRAFTING;
use receiver_policy::fill_marks;
use receiver_policy::second_asks::SECOND_ASKS;
use receiver_ports::agency::Said;

use crate::integration::common::Scratch;
use crate::integration::common::session::{StandInSession, drafting, drafting_sending};
use crate::integration::common::units::{
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
    Self::with_hiding(session, rounds, light, ceiling_seconds, true, false).await
  }

  async fn new_visible(
    session: StandInSession,
    rounds: &[&str],
    light: &[&str],
    ceiling_seconds: u32,
  ) -> Self {
    Self::with_hiding(session, rounds, light, ceiling_seconds, false, false).await
  }

  async fn new_reviewing(
    session: StandInSession,
    rounds: &[&str],
    light: &[&str],
    ceiling_seconds: u32,
  ) -> Self {
    Self::with_hiding(session, rounds, light, ceiling_seconds, true, true).await
  }

  async fn with_hiding(
    session: StandInSession,
    rounds: &[&str],
    light: &[&str],
    ceiling_seconds: u32,
    hide: bool,
    review: bool,
  ) -> Self {
    let scratch = Scratch::new().await;
    let data = tempfile::tempdir().unwrap();
    let chat = Arc::new(StandInChat::new("road-a").owners(OWNER).agent_in(OWNER, IDENTITY));

    documents(data.path());
    drafting(scratch.db(), rounds, light, ceiling_seconds).await;

    let mut standing =
      woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat.clone()], deployment()));

    standing.receiver = standing.receiver.with_situation_updates(!hide).with_review_gate(review);

    let session = Arc::new(
      if hide { session.hiding_situations() } else { session.showing_situations() }
        .hiding_reviews_or(review),
    );
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

  round
    .scratch
    .run(
      r#"INSERT INTO "room_situations" ("chat_id", "text", "updated_at") VALUES ($1, 'a standing note', 0)"#,
      vec![OWNER.into()],
    )
    .await;
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
  assert!(prompts[2].contains(question("final")) && prompts[2].contains("3 of 3"), "{prompts:#?}");
  assert!(
    prompts[2].starts_with(&format!("{}\na standing note", CHAT_ROUND.situation.front)),
    "the message question reads the room's situation note first: {}",
    prompts[2]
  );
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
        "loaded",
        "messageId",
        "ms",
        "number",
        "outcome",
        "question",
        "reads",
        "round",
        "session",
        "type",
      ]
    };

    assert_eq!(fields(event), expected, "{event}");
  }
}

const FIVE: &[&str] = &["information", "facts", "memory", "promises", "final"];

#[tokio::test]
async fn an_instruction_a_restart_interrupted_is_delivered_after_the_restart() {
  let session = StandInSession::new(SESSION, "the recovered answer");
  let round = Drafting::new(session, &["final"], &["final"], 150).await;
  let text = "an instruction a restart cut off";
  let id = queue(
    round.scratch.db(),
    AGENT,
    OWNER,
    text,
    &Queued {
      sort: Some("job"),
      ..Queued::default()
    },
  )
  .await
  .unwrap();

  // The row a turn was reading when the process stopped: handed.
  round
    .scratch
    .run(r#"UPDATE "instructions" SET "handed_at" = 1 WHERE "id" = $1"#, vec![id.into()])
    .await;
  requeue_interrupted(round.scratch.db()).await.unwrap();

  assert_eq!(
    round
      .scratch
      .texts(&format!(
        r#"SELECT (CASE WHEN "handed_at" IS NULL THEN 'pending' ELSE 'handed' END) || '|' || "restarts" FROM "instructions" WHERE "id" = {id}"#
      ))
      .await,
    ["pending|1"]
  );

  let mut last = 0;
  deliver_pending(&round.standing.receiver, &round.runner, &mut last, now_ms(), 0)
    .await
    .unwrap();
  round.ended(1).await;

  let prompts = round.session.prompts();
  let sent = round.sent();

  assert_eq!(prompts.len(), 1, "{prompts:#?}");
  assert!(prompts[0].contains(text), "{}", prompts[0]);
  assert!(prompts[0].contains(&CHAT_ROUND.instructions.restarted), "{}", prompts[0]);
  assert!(!sent.is_empty(), "the recovered answer went out");
  assert_eq!(
    round
      .scratch
      .texts(&format!(
        r#"SELECT (CASE WHEN "delivered_at" IS NOT NULL THEN 'delivered' ELSE 'open' END) FROM "instructions" WHERE "id" = {id}"#
      ))
      .await,
    ["delivered"]
  );
}

#[tokio::test]
async fn an_instruction_queued_while_a_turn_works_is_held_by_it_and_delivered_when_it_ends() {
  let session = StandInSession::new(SESSION, "here").working_for(Duration::from_millis(300));
  let round = Drafting::new(session, &["final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  until("the turn working", || round.session.working()).await;

  let text = "a notice held by the working turn";
  let id = queue(round.scratch.db(), AGENT, OWNER, text, &Queued::default()).await.unwrap();
  let mut last = 0;
  let held = deliver_pending(&round.standing.receiver, &round.runner, &mut last, now_ms(), 0)
    .await
    .unwrap();

  assert!(!held);
  assert_eq!(
    round
      .scratch
      .texts(&format!(
        r#"SELECT (CASE WHEN "handed_at" IS NOT NULL THEN 'true' ELSE 'false' END) || '|' || (CASE WHEN "delivered_at" IS NOT NULL THEN 'true' ELSE 'false' END) FROM "instructions" WHERE "id" = {id}"#
      ))
      .await,
    ["true|false"],
    "the row is held by the working turn"
  );

  let waited = tokio::time::timeout(Duration::from_secs(20), async {
    loop {
      let delivered = round
        .scratch
        .texts(&format!(
          r#"SELECT (CASE WHEN "delivered_at" IS NOT NULL THEN 'true' ELSE 'false' END) FROM "instructions" WHERE "id" = {id}"#
        ))
        .await;

      if delivered == ["true"] && round.runner.idle() {
        break;
      }

      tokio::time::sleep(Duration::from_millis(10)).await;
    }
  })
  .await;

  assert!(waited.is_ok(), "the held instruction never settled");
  assert!(!round.sent().is_empty(), "the answer the held turn produced went out");
}

#[tokio::test]
async fn an_instruction_however_old_still_opens_a_turn() {
  let session = StandInSession::new(SESSION, "the late answer");
  let round = Drafting::new(session, &["final"], &["final"], 150).await;
  let text = "a notice that waited a long time";
  let id = queue(round.scratch.db(), AGENT, OWNER, text, &Queued::default()).await.unwrap();

  round
    .scratch
    .run(r#"UPDATE "instructions" SET "created_at" = 1 WHERE "id" = $1"#, vec![id.into()])
    .await;

  let mut last = 0;

  deliver_pending(&round.standing.receiver, &round.runner, &mut last, now_ms(), 0)
    .await
    .unwrap();
  round.ended(1).await;

  let prompts = round.session.prompts();

  assert_eq!(prompts.len(), 1, "{prompts:#?}");
  assert!(prompts[0].contains(text), "{}", prompts[0]);
}

#[tokio::test]
async fn a_long_turn_in_a_room_does_not_starve_a_later_message() {
  let session = StandInSession::new(SESSION, "unscripted")
    .answering(&["the first answer", "the second answer"])
    .working_for(Duration::from_millis(400));
  let round = Drafting::new(session, &["final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  until("the turn working", || round.session.working()).await;
  round.hear(2, "a later message").await;
  round.ended(2).await;

  let sent = round.sent();

  assert!(!sent.is_empty(), "{sent:?}");
  assert!(
    sent.iter().any(|send| send.contains("the second answer")),
    "the later message is answered after the long turn: {sent:?}"
  );
}

#[tokio::test]
async fn an_agent_only_notice_reaches_the_agent_and_leaves_no_room_message() {
  let session = StandInSession::new(SESSION, "Here.");
  let round = Drafting::new(session, &["final"], &["final"], 150).await;
  let text = "Nothing needs saying in the room: this only tells you what your tools are now.";
  let id = queue(
    round.scratch.db(),
    AGENT,
    OWNER,
    text,
    &Queued {
      agent_only: true,
      ..Queued::default()
    },
  )
  .await
  .unwrap();

  round.runner.of(AGENT).instruct(OWNER, text.to_owned(), Some(id), None, vec![]).await;
  round.ended(1).await;

  let prompts = round.session.prompts();

  assert_eq!(prompts.len(), 1, "{prompts:#?}");
  assert!(prompts[0].contains(text), "{}", prompts[0]);
  assert!(round.sent().is_empty(), "{:?}", round.sent());
  assert_eq!(
    round.scratch.texts(r#"SELECT "text" FROM "messages" WHERE "chat_id" = 42"#).await,
    Vec::<String>::new()
  );
}

#[tokio::test]
async fn a_notice_meant_for_the_room_still_goes_there() {
  let session = StandInSession::new(SESSION, "Here.");
  let round = Drafting::new(session, &["final"], &["final"], 150).await;
  let text = "The room hears this notice.";
  let id = queue(
    round.scratch.db(),
    AGENT,
    OWNER,
    text,
    &Queued {
      agent_only: false,
      ..Queued::default()
    },
  )
  .await
  .unwrap();

  round.runner.of(AGENT).instruct(OWNER, text.to_owned(), Some(id), None, vec![]).await;
  round.ended(1).await;

  let prompts = round.session.prompts();
  let sent = round.sent();

  assert_eq!(prompts.len(), 1, "{prompts:#?}");
  assert!(prompts[0].contains(text), "{}", prompts[0]);
  assert_eq!(sent.len(), 1, "{sent:?}");
  assert!(sent[0].contains("Here."), "{sent:?}");
  assert_eq!(
    round.scratch.texts(r#"SELECT "text" FROM "messages" WHERE "chat_id" = 42"#).await,
    ["Here."]
  );
}

#[tokio::test]
async fn a_private_chat_round_writes_the_situation_note_and_hands_it_to_the_judgement() {
  let session = StandInSession::new(SESSION, "unscripted").answering(&["the private note", "the message"]);
  let round = Drafting::new_visible(session, &["final"], &["final"], 150).await;

  round.hear(1, "hello").await;
  round.ended(2).await;

  let prompts = round.session.prompts();
  let sent = round.sent();

  assert_eq!(prompts.len(), 2, "{prompts:#?}");
  assert!(prompts[0].contains(&CHAT_ROUND.situation.previous), "{}", prompts[0]);
  assert!(
    prompts[1].starts_with(&format!("{}\nthe private note", CHAT_ROUND.situation.front)),
    "the message turn reads the note it just wrote first: {}",
    prompts[1]
  );
  assert!(prompts[1].contains(question("final")), "{}", prompts[1]);
  assert_eq!(
    round
      .scratch
      .texts(r#"SELECT "text" FROM "room_situations" WHERE "chat_id" = 42"#)
      .await,
    ["the private note"]
  );
  assert_eq!(
    round
      .scratch
      .texts(
        r#"SELECT COALESCE("situation", '') || '|' || COALESCE("situation_changed"::text, '')
           FROM "messages" WHERE "chat_id" = 42 AND "message_id" = 1"#
      )
      .await,
    ["the private note|true"]
  );
  assert!(sent.len() == 1 && sent[0].contains("the message"), "{sent:?}");
}

#[tokio::test]
async fn a_second_rounds_note_turn_reads_only_what_came_since_the_note() {
  let session = StandInSession::new(SESSION, "unscripted").answering(&[
    "note one",
    "message one",
    "note two",
    "message two",
  ]);
  let round = Drafting::new_visible(session, &["final"], &["final"], 150).await;

  round.hear(1, "first words").await;
  round.ended(2).await;
  round.hear(2, "second words").await;
  round.ended(4).await;

  let prompts = round.session.prompts();

  assert_eq!(prompts.len(), 4, "{prompts:#?}");
  assert!(
    prompts[0].contains("first words"),
    "the first note seeds from the room's latest messages: {}",
    prompts[0]
  );
  assert!(prompts[2].contains("second words"), "{}", prompts[2]);
  assert!(
    !prompts[2].contains("first words"),
    "the note turn reads only what came since the note: {}",
    prompts[2]
  );
}

#[tokio::test]
async fn a_note_far_behind_its_room_reads_the_newest_messages_and_moves_its_mark() {
  let session = StandInSession::new(SESSION, "unscripted").answering(&["the catch-up note", "the message"]);
  let round = Drafting::new_visible(session, &["final"], &["final"], 150).await;

  // One message the note has read, then a gap longer than the policy's bound behind it.
  round
    .scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text")
         VALUES (42, 1, 42, 'tester', 1000, 'm-001')"#,
      vec![],
    )
    .await;
  let read_to: i64 = round
    .scratch
    .texts(r#"SELECT "rowid"::text FROM "messages" WHERE "chat_id" = 42 AND "message_id" = 1"#)
    .await
    .remove(0)
    .parse()
    .unwrap();

  round
    .scratch
    .run(
      r#"INSERT INTO "room_situations" ("chat_id", "text", "updated_at", "read_through")
         VALUES (42, 'a standing note', 0, $1)"#,
      vec![read_to.into()],
    )
    .await;

  for id in 2..=201i64 {
    round
      .scratch
      .run(
        r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text")
           VALUES (42, $1, 42, 'tester', $2, $3)"#,
        vec![id.into(), (1000 + id).into(), format!("m-{id:03}").into()],
      )
      .await;
  }

  round.hear(202, "the newest words").await;
  round.ended(2).await;

  let prompts = round.session.prompts();

  assert!(
    !prompts[0].contains("m-001") && !prompts[0].contains("m-002"),
    "a gap longer than the bound is not read whole: {}",
    prompts[0]
  );
  assert!(
    prompts[0].contains("m-201") && prompts[0].contains("the newest words"),
    "{}",
    prompts[0]
  );
  assert_eq!(
    round
      .scratch
      .texts(r#"SELECT "read_through"::text FROM "room_situations" WHERE "chat_id" = 42"#)
      .await,
    round
      .scratch
      .texts(r#"SELECT "rowid"::text FROM "messages" WHERE "chat_id" = 42 AND "message_id" = 202"#)
      .await,
    "the mark moves to the newest message read"
  );
}

#[tokio::test]
async fn a_private_chat_skip_leaves_the_standing_note_unchanged() {
  let session = StandInSession::new(SESSION, "unscripted").answering(&["<<SKIP>>", "the message"]);
  let round = Drafting::new_visible(session, &["final"], &["final"], 150).await;

  round
    .scratch
    .run(
      r#"INSERT INTO "room_situations" ("chat_id", "text", "updated_at") VALUES ($1, 'standing note', 0)"#,
      vec![OWNER.into()],
    )
    .await;
  round.hear(1, "hello").await;
  round.ended(2).await;

  assert_eq!(
    round
      .scratch
      .texts(r#"SELECT "text" FROM "room_situations" WHERE "chat_id" = 42"#)
      .await,
    ["standing note"]
  );
  assert_eq!(
    round
      .scratch
      .texts(
        r#"SELECT COALESCE("situation", '') || '|' || COALESCE("situation_changed"::text, '')
           FROM "messages" WHERE "chat_id" = 42 AND "message_id" = 1"#
      )
      .await,
    ["standing note|false"]
  );
}

#[tokio::test]
async fn old_console_rows_fall_to_the_policy_one_row() {
  use bmo_receiver::chat::draft::table::{self, ANY, PastCeiling, Process};

  let scratch = Scratch::new().await;
  let row = |provider: &str, rounds: &[&str]| Process {
    provider: provider.to_owned(),
    tier: ANY.to_owned(),
    agent: ANY.to_owned(),
    rounds: rounds.iter().map(|name| (*name).to_owned()).collect(),
    send_from: "facts".to_owned(),
    light: vec!["final".to_owned()],
    light_chars: 12,
    ceiling_seconds: 150,
    past_ceiling: PastCeiling::Send,
    set: true,
  };

  table::set(scratch.db(), &row("claude-code", FIVE)).await.unwrap();
  table::set(scratch.db(), &row("small-model", &["information", "facts", "final"]))
    .await
    .unwrap();

  let rows = table::rows(scratch.db()).await.unwrap();

  assert_eq!(rows.len(), 1, "{rows:?}");
  assert_eq!(
    (rows[0].provider.as_str(), rows[0].tier.as_str(), rows[0].agent.as_str()),
    (ANY, ANY, ANY)
  );
  assert_eq!(rows[0].rounds, FIVE);
}

#[tokio::test]
async fn a_small_model_agent_runs_the_five_questions() {
  let session = StandInSession::new(SESSION, "unscripted").on_provider("small").answering(&[
    "notes",
    "more notes",
    "memories",
    "promises kept",
    "the message",
  ]);
  let round = Drafting::new(session, FIVE, &["final"], 150).await;

  round.hear(1, ASKED).await;
  round.ended(5).await;

  let prompts = round.session.prompts();

  assert_eq!(prompts.len(), 5, "{prompts:#?}");
  assert!(prompts[4].contains(question("final")));
  assert!(round.sent()[0].contains("the message"));
}

#[tokio::test]
async fn a_claude_agent_runs_the_same_five_questions() {
  let session = StandInSession::new(SESSION, "unscripted").on_provider("claude-code").answering(&[
    "notes",
    "more notes",
    "memories",
    "promises kept",
    "the message",
  ]);
  let round = Drafting::new(session, FIVE, &["final"], 150).await;

  round.hear(1, ASKED).await;
  round.ended(5).await;

  let prompts = round.session.prompts();

  assert_eq!(prompts.len(), 5, "{prompts:#?}");
  assert!(prompts[4].contains(question("final")));
  assert!(round.sent()[0].contains("the message"));
}

#[tokio::test]
async fn a_light_turn_still_runs_final_alone_with_the_one_process() {
  let session = StandInSession::new(SESSION, "unscripted").answering(&["hello to you too"]);
  let round = Drafting::new(session, FIVE, &["final"], 150).await;

  round.hear(1, "hi").await;
  round.ended(1).await;

  let prompts = round.session.prompts();

  assert_eq!(prompts.len(), 1, "a light turn asks final alone: {prompts:#?}");
  assert!(prompts[0].contains(question("final")));
  assert!(round.sent()[0].contains("hello to you too"));
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
async fn a_send_before_the_question_a_send_is_read_from_is_the_message_and_nothing_more_is_asked() {
  let session = StandInSession::new(SESSION, "unscripted")
    .answering(&["<<SEND>> the answer, ready at once", "never asked"]);
  let rounds = ["information", "facts", "final"];
  let round = Drafting::new(session, &rounds, &["final"], 150).await;

  drafting_sending(round.scratch.db(), &rounds, "facts", &["final"], 150).await;
  round.hear(1, ASKED).await;
  round.ended(1).await;

  let prompts = round.session.prompts();
  let sent = round.sent();

  assert_eq!(prompts.len(), 1, "no later question is asked after an early send: {prompts:#?}");
  assert!(
    prompts[0].contains("writes no message of its own"),
    "the notes question says it writes no message: {}",
    prompts[0]
  );
  assert!(
    sent.len() == 1 && sent[0].contains("the answer, ready at once") && !sent[0].contains("<<SEND>>"),
    "{sent:?}"
  );
  assert_eq!(round.log().await, ["question information send", "end  message"]);
}

#[tokio::test]
async fn a_draft_that_holds_the_answer_is_sent_and_a_repeated_request_is_answered_again() {
  let session = StandInSession::new(SESSION, "unscripted")
    .answering(&["<<SEND>> 400207-005: 1,500,000", "400207-005: 1,500,000"]);
  let round = Drafting::new(session, &["final"], &["final"], 150).await;

  round.hear(1, "what is the price of 400207-005?").await;
  round.ended(1).await;

  let first = round.sent();

  assert_eq!(first.len(), 1, "{first:?}");
  assert!(first[0].contains("1,500,000"), "the draft's answer is the message: {first:?}");
  assert!(
    round.session.prompts()[0].contains("A draft"),
    "the round says a draft is not a message"
  );

  // The repeated request is answered again: the first answer was a draft that has now gone out, not a reason to skip.
  round.hear(2, "what is the price of 400207-005 again?").await;
  round.ended(2).await;

  let sent = round.sent();
  let prompts = round.session.prompts();

  assert_eq!(sent.len(), 2, "the repeated request is answered again: {sent:?}");
  assert!(sent[1].contains("1,500,000"), "{sent:?}");
  assert!(prompts[1].contains("what is the price of 400207-005 again?"), "{prompts:#?}");
  assert_eq!(
    round.log().await,
    ["question final send", "end  message", "question final answered", "end  message"]
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

#[test]
fn the_situation_update_says_never_that_a_reply_went_out() {
  let prompt = bmo_receiver::chat::situation::update_prompt(None, &[]);

  assert!(prompt.contains("nothing written here is sent to the room"), "{prompt}");
  assert!(prompt.contains("how it is best answered"), "{prompt}");
  assert!(prompt.contains("Never copy a message"), "{prompt}");
  assert!(prompt.contains("never restate your own answer"), "{prompt}");
  assert!(prompt.contains("write no first-person claim"), "{prompt}");
  assert!(prompt.contains("what remains to be done"), "{prompt}");
}

#[tokio::test]
async fn a_note_claiming_a_reply_does_not_skip_the_owners_directive() {
  let session = StandInSession::new(SESSION, "unscripted").answering(&[
    "the correction was attached and sent",
    "<<SKIP>>",
    "the corrected answer",
  ]);
  let round = Drafting::new_visible(session, &["final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  round.ended(3).await;

  let prompts = round.session.prompts();
  let sent = round.sent();

  assert_eq!(prompts.len(), 3, "{prompts:#?}");
  assert!(
    prompts[1].contains("the correction was attached and sent"),
    "the note the draft read claims the reply: {}",
    prompts[1]
  );
  assert!(prompts[2].contains(&SECOND_ASKS.errands.owed[..40]), "{}", prompts[2]);
  assert_eq!(sent.len(), 1, "{sent:?}");
  assert!(sent[0].contains("the corrected answer"), "{sent:?}");
}

#[tokio::test]
async fn the_review_passes_the_message_after_reading_the_room_and_the_round() {
  let session = StandInSession::new(SESSION, "unscripted").answering(&["the message", "PASS"]);
  let round = Drafting::new_reviewing(session, &["final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  round.ended(2).await;

  let prompts = round.session.prompts();
  let sent = round.sent();
  let log = round.log().await;

  assert_eq!(prompts.len(), 2, "{prompts:#?}");
  assert!(prompts[1].contains(&DRAFTING.review.head), "{}", prompts[1]);
  assert!(prompts[1].contains("The room's words #1 were handed in this turn"), "{}", prompts[1]);
  assert!(prompts[1].contains("the message"), "the draft: {}", prompts[1]);
  assert_eq!(sent.len(), 1);
  assert!(sent[0].contains("the message"), "{sent:?}");
  assert!(log.contains(&"review review PASS".to_owned()), "{log:?}");
}

#[tokio::test]
async fn a_rejected_message_is_written_again_with_the_reason() {
  let session = StandInSession::new(SESSION, "unscripted").answering(&[
    "the first message",
    "REJECT: the date is missing",
    "the second message",
    "PASS",
  ]);
  let round = Drafting::new_reviewing(session, &["final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  round.ended(4).await;

  let prompts = round.session.prompts();
  let sent = round.sent();
  let log = round.log().await;

  assert_eq!(prompts.len(), 4, "{prompts:#?}");
  assert!(prompts[2].contains("the date is missing"), "{}", prompts[2]);
  assert_eq!(sent.len(), 1, "{sent:?}");
  assert!(sent[0].contains("the second message"), "{sent:?}");
  assert_eq!(log.iter().filter(|line| line.contains("review REJECT")).count(), 1, "{log:?}");
  assert!(log.contains(&"review review PASS".to_owned()), "{log:?}");
}

#[tokio::test]
async fn a_turn_hands_its_material_once_and_later_rounds_carry_lines() {
  use receiver_ports::gate::{Entry, Source};

  let body = "BODY OF A LONG RULE ".repeat(120);
  let room = "A LONG LINE OF THE ROOM ".repeat(80);
  let session = StandInSession::new(SESSION, "unscripted").answering(&["the message", "PASS"]);
  let round = Drafting::new_reviewing(session, &["final"], &["final"], 150).await;

  *round.standing.gate.entries.lock().unwrap() = vec![Entry {
    path: "docs/long-rule.md".to_owned(),
    text: body.clone(),
    source: Source::Knowledge,
    fragment: None,
    holds: None,
  }];

  round.hear(1, &room).await;
  round.ended(2).await;

  let prompts = round.session.prompts();
  let main = &prompts[0];
  let review = &prompts[1];
  let full = review.replace("[docs/long-rule.md] (handed in this turn)", &body).replace(
    "The room's words #1 were handed in this turn; only the lines after them are new.",
    &room,
  );

  assert!(main.contains(&body) && main.contains(&room), "the main round handed it");
  assert!(
    !review.contains(&body) && !review.contains(&room),
    "the later round carries lines, not the bodies: {review}"
  );
  assert!(review.contains("[docs/long-rule.md] (handed in this turn)"), "{review}");
  assert!(review.contains("The room's words #1 were handed in this turn"), "{review}");
  eprintln!(
    "turn mark: main {} chars; later round {} before {} after {}",
    main.chars().count(),
    review.chars().count(),
    full.chars().count(),
    review.chars().count()
  );
}

#[tokio::test]
async fn eight_rejections_send_the_last_draft_and_report_the_reasons() {
  let mut answers: Vec<String> = vec!["the first draft".to_owned()];
  let mut drafts = vec!["the first draft".to_owned()];

  for at in 1..=8 {
    answers.push(format!("REJECT: reason {at}"));

    if at < 8 {
      let draft = format!("the draft after {at}");
      answers.push(draft.clone());
      drafts.push(draft);
    }
  }

  let borrowed: Vec<&str> = answers.iter().map(String::as_str).collect();
  let session = StandInSession::new(SESSION, "unscripted").answering(&borrowed);
  let round = Drafting::new_reviewing(session, &["final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  round.ended(16).await;

  let sent = round.sent();
  let log = round.log().await;
  let end = round.scratch.texts(r#"SELECT "detail" FROM "draft_rounds" WHERE "kind" = 'end'"#).await;

  assert_eq!(sent.len(), 1, "{sent:?}");
  assert!(sent[0].contains(drafts.last().unwrap()), "{sent:?}");
  assert_eq!(log.iter().filter(|line| line.contains("review REJECT")).count(), 8, "{log:?}");
  assert!(end[0].contains("review rejected eight times"), "{end:?}");
  assert!(end[0].contains("reason 1"), "{end:?}");
  assert!(end[0].contains("reason 8"), "{end:?}");
}

#[tokio::test]
async fn a_long_answer_goes_out_with_no_length_check() {
  let long = "x".repeat(10_000);
  let session = StandInSession::new(SESSION, "unscripted").answering(&[&long]);
  let round = Drafting::new(session, &["final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  round.ended(1).await;

  let sent = round.sent();

  assert_eq!(sent.len(), 1, "{sent:?}");
  assert!(sent[0].contains(&long), "the answer goes out whole");
}

#[tokio::test]
async fn an_answer_folds_only_where_the_message_itself_carries_the_fold_mark() {
  let body = "The lead.\n\n- the first point of a long body\n- the second point\n- the third point";
  let session = StandInSession::new(SESSION, "unscripted").answering(&["notes <<FOLD>>", body]);
  let round = Drafting::new(session, &["intent", "final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  round.ended(2).await;

  let sent = round.sent();

  assert_eq!(sent.len(), 1, "{sent:?}");
  assert!(
    !sent[0].contains("\"fold\":true"),
    "a mark in the notes folds no message, and a long message goes out unfolded: {sent:?}"
  );

  let marked = format!("{body}\n\n<<FOLD>>");
  let session = StandInSession::new(SESSION, "unscripted").answering(&["notes", &marked]);
  let round = Drafting::new(session, &["intent", "final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  round.ended(2).await;

  let sent = round.sent();

  assert_eq!(sent.len(), 1, "{sent:?}");
  assert!(sent[0].contains("\"fold\":true"), "the message's own mark folds it: {sent:?}");
  assert!(!sent[0].contains("<<FOLD>>"), "the mark never reaches the room: {sent:?}");
}

#[tokio::test]
async fn a_check_that_fails_twice_withholds_the_message() {
  let lines = "The process asks question";
  let session = StandInSession::new(SESSION, "unscripted").answering(&[lines, lines]);
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
  assert!(end[0].starts_with("withheld ") && end[0].ends_with("format"), "{end:?}");
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
  assert!(
    !prompts[0].contains("ANSWER LANGUAGE"),
    "no turn names an answer language: {prompts:#?}"
  );
  assert_eq!(round.sent().len(), 1);
  assert!(round.sent()[0].contains(&answer));
  assert_eq!(round.log().await, ["question final answered", "end  message"]);
}

#[tokio::test]
async fn a_korean_answer_to_a_korean_owner_goes_out_untouched() {
  let korean = fixture("answer-korean.txt");
  let session = StandInSession::new(SESSION, "unscripted").answering(&[&korean]);
  let round = Drafting::new(session, &["final"], &["final"], 150).await;

  round.hear(1, &fixture("addressing-korean.txt")).await;
  round.ended(1).await;

  let sent = round.sent();

  assert_eq!(sent.len(), 1, "{sent:?}");
  assert!(sent[0].contains(&korean), "{sent:?}");
  assert_eq!(round.log().await, ["question final answered", "end  message"]);
}

#[tokio::test]
async fn an_answer_in_a_language_the_person_does_not_write_also_goes_out() {
  let answer = fixture("answer-japanese.txt");
  let session = StandInSession::new(SESSION, "unscripted").answering(&[&answer]);
  let round = Drafting::new(session, &["final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  round.ended(1).await;

  let sent = round.sent();

  assert_eq!(sent.len(), 1, "{sent:?}");
  assert!(sent[0].contains(&answer), "{sent:?}");
  assert_eq!(round.log().await, ["question final answered", "end  message"]);
}

#[tokio::test]
async fn a_round_past_the_old_ceiling_still_asks_every_question_and_sends() {
  let session = StandInSession::new(SESSION, "unscripted")
    .answering(&["notes", "more notes", "facts", "the message final wrote"])
    .working_for(Duration::from_millis(600));
  let rounds = ["intent", "information", "facts", "final"];
  let round = Drafting::new(session, &rounds, &["final"], 1).await;

  round.hear(1, ASKED).await;
  round.ended(4).await;

  let prompts = round.session.prompts();
  let sent = round.sent();

  assert_eq!(prompts.len(), 4, "no question is passed over by a clock: {prompts:#?}");
  assert!(prompts.iter().any(|prompt| prompt.contains(question("facts"))));
  assert!(prompts[3].contains(question("final")));
  assert_eq!(sent.len(), 1);
  assert!(sent[0].contains("the message final wrote"), "the message goes out: {sent:?}");
  assert_eq!(
    round.log().await,
    [
      "question intent answered",
      "question information answered",
      "question facts answered",
      "question final answered",
      "end  message"
    ]
  );
}

#[tokio::test]
async fn a_failing_answer_after_the_old_ceiling_gets_its_repair_round() {
  let head = DRAFTING.lead.split('{').next().unwrap_or_default().trim();
  let first = format!("{head} and then the words");
  let session = StandInSession::new(SESSION, "unscripted")
    .answering(&[&first, "the repaired message"])
    .working_for(Duration::from_millis(700));
  let round = Drafting::new(session, &["final"], &["final"], 1).await;

  round.hear(1, ASKED).await;
  round.ended(2).await;

  let prompts = round.session.prompts();
  let sent = round.sent();

  assert_eq!(prompts.len(), 2, "the check is asked despite the old clock: {prompts:#?}");
  assert!(
    prompts[1].contains(&SECOND_ASKS.errands.format),
    "the format check's errand: {}",
    prompts[1]
  );
  assert_eq!(sent.len(), 1);
  assert!(sent[0].contains("the repaired message"), "{sent:?}");
  assert_eq!(round.log().await, ["question final answered", "check format answered", "end  message"]);
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
  // The working turn is looked at meanwhile, and the stand-in agency takes nothing pushed into it.
  tokio::time::sleep(Duration::from_millis(50)).await;
  round.session.let_go();
}

// The round's own turn held working while `SHORT` comes into the room and is pushed into it, the agency taking
// it, then let go.
async fn pushed_during_the_first(round: &Drafting) {
  *round.standing.agency.saying.lock().unwrap() = Some(Said::Delivered);
  round.hear(1, ASKED).await;
  until("the round's turn", || round.session.prompts().len() == 1 && round.session.working()).await;
  round.hear(2, SHORT).await;
  until("the push", || !round.standing.agency.pushes.lock().unwrap().is_empty()).await;
  round.session.let_go();
}

// The message the room's read mark stands at.
async fn read_mark(round: &Drafting) -> Vec<String> {
  round
    .scratch
    .texts(&format!(
      r#"SELECT CAST("last_read_message_id" AS TEXT) FROM "chat_reads" WHERE "chat_id" = {OWNER}"#
    ))
    .await
}

#[tokio::test]
async fn a_push_carries_the_room_s_words_and_the_instructions_in_one_paragraph() {
  const INSTRUCTION: &str = "a quiet instruction from the service";

  let session = StandInSession::new(SESSION, "unscripted");
  let round = Drafting::new(session, &["final"], &["final"], 150).await;
  let receiver = &round.standing.receiver;
  let built = paragraph(&Push {
    receiver,
    agent: AGENT,
    owner: OWNER,
    chat_id: OWNER,
    round: "round-a",
    messages: &[said(2, SHORT)],
    instructions: &[INSTRUCTION.to_owned()],
    events: &[],
    holds: &Default::default(),
    consumed: Some(1),
    tools_allowed: true,
    reader: None,
    judged: true,
  })
  .await
  .unwrap();
  let text = &built.text;
  let push = &CHAT_ROUND.push;
  let at = |part: &str| text.find(part).unwrap_or_else(|| panic!("{part:?} stands in {text}"));

  assert!(text.contains(&push.judge), "the rule each message is judged by: {text}");
  assert!(text.contains(&push.heed), "how the instructions are taken: {text}");
  assert!(text.contains("<<SEPARATE #<number>>>") && text.contains("<<DONE #<number>>>"), "{text}");
  assert!(text.contains(SHORT) && text.contains("tester"), "the message with its sender: {text}");
  assert!(
    text.contains(&fill_marks(&push.scope, &[("ids", "[2]")])),
    "its number joins the scope: {text}"
  );
  assert!(at(&push.judge) < at(SHORT), "the rule stands before the words");
  assert!(at(SHORT) < at(INSTRUCTION), "the messages stand before the instructions");
  assert_eq!(built.instructions, INSTRUCTION);
}

#[tokio::test]
async fn words_a_turn_does_not_take_while_it_works_stand_under_the_next_question() {
  let session = StandInSession::new(SESSION, "unscripted")
    .answering(&["notes", "more notes", "the message that takes both"])
    .holding_turn(1);
  let round = Drafting::new(session, &["intent", "information", "final"], &["final"], 150).await;

  // A provider that takes no words mid-turn: the agency refuses every push.
  *round.standing.agency.saying.lock().unwrap() = Some(Said::Refused);
  round.hear(1, ASKED).await;
  until("the round's turn", || round.session.prompts().len() == 1 && round.session.working()).await;
  round.hear(2, SHORT).await;
  until("the refused push", || !round.standing.agency.pushes.lock().unwrap().is_empty()).await;
  until("the words waiting again", || round.runner.of(AGENT).buffered(OWNER) == [2]).await;
  round.session.let_go();
  round.ended(3).await;

  let prompts = round.session.prompts();
  let sent = round.sent();

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
async fn words_pushed_with_no_mark_are_the_round_s_own_and_shown_no_more() {
  let session = StandInSession::new(SESSION, "unscripted")
    .answering(&["notes", "the message that takes both"])
    .holding_turn(1);
  let round = Drafting::new(session, &["intent", "final"], &["final"], 150).await;

  pushed_during_the_first(&round).await;
  round.ended(2).await;

  let prompts = round.session.prompts();
  let sent = round.sent();
  let pushes = round.standing.agency.pushes.lock().unwrap().clone();

  assert_eq!(pushes.len(), 1, "{pushes:?}");
  assert!(
    pushes[0].2.contains(SHORT) && pushes[0].2.contains(&CHAT_ROUND.push.judge),
    "{}",
    pushes[0].2
  );
  assert!(!prompts[1].contains(SHORT), "a pushed message stands under no question: {}", prompts[1]);
  assert_eq!(prompts.len(), 2, "the pushed words call no round of their own");
  assert!(sent.len() == 1 && sent[0].contains("the message that takes both"), "{sent:?}");
  assert!(round.runner.of(AGENT).pending(OWNER).is_empty(), "nothing waits after the round");
  assert_eq!(read_mark(&round).await, ["2"], "the round settled the pushed message");
}

#[tokio::test]
async fn a_pushed_message_held_stays_unsettled_and_opens_the_round_after() {
  let session = StandInSession::new(SESSION, "unscripted")
    .answering(&["<<SEPARATE #2>> notes", "the message for the first words", "the answer to the second"])
    .holding_turn(1);
  let round = Drafting::new(session, &["intent", "final"], &["final"], 150).await;

  pushed_during_the_first(&round).await;
  round.ended(3).await;

  let prompts = round.session.prompts();
  let sent = round.sent();

  assert!(!prompts[1].contains(SHORT), "the held message stands under no question: {}", prompts[1]);
  assert_eq!(prompts.len(), 3, "the held message opens one round of its own");
  assert!(prompts[2].contains(SHORT), "the round after asks it: {}", prompts[2]);
  assert_eq!(sent.len(), 2, "{sent:?}");
  assert!(
    sent[0].contains("the message for the first words") && !sent[0].contains("SEPARATE"),
    "{sent:?}"
  );
  assert!(sent[1].contains("the answer to the second"), "{sent:?}");
}

#[tokio::test]
async fn a_pushed_message_set_aside_is_read_and_answered_by_nothing() {
  let session = StandInSession::new(SESSION, "unscripted")
    .answering(&["<<DONE #2>> notes", "the message for the first words"])
    .holding_turn(1);
  let round = Drafting::new(session, &["intent", "final"], &["final"], 150).await;

  pushed_during_the_first(&round).await;
  round.ended(2).await;

  let prompts = round.session.prompts();
  let sent = round.sent();

  assert_eq!(prompts.len(), 2, "a message set aside calls no round of its own");
  assert!(sent.len() == 1 && !sent[0].contains("DONE"), "{sent:?}");
  assert!(round.runner.of(AGENT).pending(OWNER).is_empty(), "nothing waits after the round");
  assert_eq!(read_mark(&round).await, ["2"], "the read mark moved past the message set aside");
}

#[tokio::test]
async fn words_pushed_into_a_later_question_are_the_round_s_and_stand_under_no_question() {
  const BETWEEN: &str = "and one more thing said between the questions";

  let session = StandInSession::new(SESSION, "unscripted")
    .answering(&["notes", "more notes", "the message that takes both"])
    .holding_turn(2);
  let round = Drafting::new(session, &["intent", "information", "final"], &["final"], 150).await;

  *round.standing.agency.saying.lock().unwrap() = Some(Said::Delivered);
  round.hear(1, ASKED).await;
  until("the second question", || round.session.prompts().len() == 2 && round.session.working()).await;
  round.hear(2, BETWEEN).await;
  until("the push", || !round.standing.agency.pushes.lock().unwrap().is_empty()).await;
  round.session.let_go();
  round.ended(3).await;

  let prompts = round.session.prompts();
  let pushes = round.standing.agency.pushes.lock().unwrap().clone();

  assert!(pushes[0].2.contains(BETWEEN), "{}", pushes[0].2);
  assert!(!pushes[0].2.contains(&CHAT_ROUND.push.heed), "a later question takes no instruction");
  assert!(!prompts[2].contains(BETWEEN), "the third question does not carry them again");
  assert_eq!(prompts.len(), 3, "the words call no round of their own");
  assert_eq!(round.sent().len(), 1);
  assert_eq!(read_mark(&round).await, ["2"]);
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

#[tokio::test]
async fn a_closing_check_answered_with_the_fold_mark_alone_sends_the_message_as_it_stands() {
  let session = StandInSession::new(SESSION, "unscripted")
    .answering(&["notes", "the first message", "<<FOLD>>"])
    .holding_turn(2);
  let round = Drafting::new(session, &["intent", "final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  until("the last question", || round.session.prompts().len() == 2 && round.session.working()).await;
  round.hear(2, SHORT).await;
  until("the words waiting", || round.runner.of(AGENT).buffered(OWNER) == [2]).await;
  round.session.let_go();
  round.ended(3).await;

  let sent = round.sent();

  assert!(
    sent.len() == 1 && sent[0].contains("the first message") && !sent[0].contains("\"fold\":true"),
    "an answer of the fold mark alone is marks alone, and keeps the message as it stood: {sent:?}"
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
async fn a_long_turn_still_sends_what_it_produced() {
  let session = StandInSession::new(SESSION, "unscripted")
    .answering(&["notes", "the message after a long while"])
    .working_for(Duration::from_millis(1300));
  let round = Drafting::new(session, &["intent", "final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  round.ended(2).await;

  let sent = round.sent();

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
    holds: None,
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
    vec![
      IndexGroup {
        group: "knowledge".to_owned(),
        unread: 1,
        hint: None,
        entries: vec![
          IndexEntry {
            slug: "plans".to_owned(),
            holds: "the plan for the week".to_owned(),
            path: "knowledge/plans".to_owned(),
            memory_id: None,
          },
          IndexEntry {
            slug: "older".to_owned(),
            holds: "what was planned before".to_owned(),
            path: "knowledge/older".to_owned(),
            memory_id: Some(31),
          },
        ],
      },
      IndexGroup {
        group: "room-memories".to_owned(),
        unread: 1,
        hint: None,
        entries: vec![IndexEntry {
          slug: "memory-7".to_owned(),
          holds: "a promise to keep".to_owned(),
          path: "memories/42/memory-7".to_owned(),
          memory_id: None,
        }],
      },
    ],
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
  // It reads again a piece the session held before the round, and reads a memory, both logged by its run.
  round
    .standing
    .agency
    .loads
    .lock()
    .unwrap()
    .get_mut(SESSION)
    .unwrap()
    .push(load("memories/42/memory-7", "v1"));

  for (at, input) in [
    serde_json::json!({ "group": "knowledge", "slug": "older" }),
    serde_json::json!({ "group": "room-memories", "slug": "memory-7" }),
  ]
  .into_iter()
  .enumerate()
  {
    let line = serde_json::json!({ "id": at, "kind": "tool", "name": "piece", "text": input.to_string() });

    round.session.logged.lock().unwrap().push(line.as_object().unwrap().clone());
  }

  round.session.let_go();
  round.ended(1).await;

  let reads = round.scratch.texts(r#"SELECT "reads" FROM "draft_rounds" WHERE "kind" = 'end'"#).await;
  let reads: serde_json::Value = serde_json::from_str(&reads[0]).unwrap();

  assert_eq!(
    reads,
    serde_json::json!([
      {"path": "memories/42", "holds": "the room's memories now due", "how": "attached"},
      {"path": "rooms/42/people/42/sheet.md", "holds": "a person's sheet", "how": "held"},
      {"path": "knowledge/older", "holds": "what was planned before", "memoryId": 31, "how": "held"},
      {"path": "knowledge/plans", "holds": "the plan for the week", "how": "read"},
      {"path": "memories/42/memory-7", "holds": "a promise to keep", "memoryId": 7, "how": "read"},
    ]),
    "the core first, named as it went in, then what the agent read itself, named by the index: a piece it held \
     before and read again stands held, a piece it held and did not read again stands nowhere"
  );

  let others = round
    .scratch
    .texts(r#"SELECT coalesce("reads", 'none') FROM "draft_rounds" WHERE "kind" <> 'end'"#)
    .await;

  assert!(others.iter().all(|row| row == "none"), "{others:?}");
}

/// An end row of an earlier round written straight into the round log: its round, room, session, count and reads.
async fn earlier_end(
  round: &Drafting,
  name: &str,
  chat_id: i64,
  session: &str,
  loaded: i64,
  reads: serde_json::Value,
) {
  round
    .scratch
    .run(
      r#"INSERT INTO "draft_rounds" ("round", "chat_id", "agent", "number", "question", "kind", "answer_chars",
           "changed", "outcome", "detail", "answer", "ms", "at", "reads", "session", "loaded")
         VALUES ($1, $2, $3, 2, '', 'end', 0, false, 'message', '', NULL, 1, 1, $4, $5, $6)"#,
      vec![
        name.into(),
        chat_id.into(),
        AGENT.into(),
        reads.to_string().into(),
        session.into(),
        loaded.into(),
      ],
    )
    .await;
}

/// The rows of this round's log as the route answers them: the end and every other row.
async fn answered_rows(round: &Drafting) -> (serde_json::Value, Vec<serde_json::Value>) {
  let name = round
    .scratch
    .texts(r#"SELECT "round" FROM "draft_rounds" WHERE "kind" = 'end' AND "at" > 1 ORDER BY "id""#)
    .await;

  assert_eq!(name.len(), 1, "one round ran: {name:?}");

  let read = bmo_receiver::chat::draft::log::read(round.scratch.db(), OWNER, &name).await.unwrap();
  let rows = read[0]["rows"].as_array().unwrap().clone();
  let (ends, others): (Vec<_>, Vec<_>) = rows.into_iter().partition(|row| row["kind"] == "end");

  (ends[0].clone(), others)
}

#[tokio::test]
async fn a_sessions_first_round_in_a_room_has_read_nothing_before_it() {
  let session = StandInSession::new(SESSION, "the message");
  let round = Drafting::new(session, &["final"], &["final"], 150).await;
  let full = serde_json::json!([{"path": "knowledge/plans", "holds": null, "how": "read"}]);

  // What the same session read in another room, and what another session read in this one, carry nothing over.
  earlier_end(&round, "elsewhere", OWNER + 1, SESSION, 5, full.clone()).await;
  earlier_end(&round, "another session", OWNER, "agent-a@other", 9, full).await;

  round.hear(1, ASKED).await;
  round.ended(1).await;

  let (end, others) = answered_rows(&round).await;

  assert_eq!(
    (&end["session"], &end["loaded"]),
    (&serde_json::json!(SESSION), &serde_json::json!(0)),
    "{end}"
  );
  assert!(!others.is_empty());
  assert!(
    others.iter().all(|row| row["session"].is_null() && row["loaded"].is_null()),
    "only the end carries the session and its count: {others:?}"
  );
  assert_eq!(
    bmo_receiver::chat::draft::log::loaded(round.scratch.db(), OWNER, None).await.unwrap(),
    0,
    "a round on no session counts nothing"
  );
}

#[tokio::test]
async fn the_end_row_counts_what_its_session_read_in_full_in_the_room_before_it() {
  let session = StandInSession::new(SESSION, "the message");
  let round = Drafting::new(session, &["final"], &["final"], 150).await;

  earlier_end(&round, "long ago", OWNER, SESSION, 40, serde_json::json!([])).await;
  earlier_end(
    &round,
    "the round before",
    OWNER,
    SESSION,
    2,
    serde_json::json!([
      {"path": "memories/42", "holds": "the room's memories now due", "how": "attached"},
      {"path": "knowledge/plans", "holds": "the plan for the week", "how": "read"},
      {"path": "rooms/42/people/42/sheet.md", "holds": "a person's sheet", "how": "held"},
    ]),
  )
  .await;
  earlier_end(&round, "elsewhere", OWNER + 1, SESSION, 70, serde_json::json!([])).await;
  earlier_end(&round, "another session", OWNER, "agent-a@other", 90, serde_json::json!([])).await;

  round.hear(1, ASKED).await;
  round.ended(1).await;

  let (end, _) = answered_rows(&round).await;

  assert_eq!(
    (&end["session"], &end["loaded"]),
    (&serde_json::json!(SESSION), &serde_json::json!(4)),
    "the latest earlier end of the room and session, its count and its attached and read pieces, a held one \
     counting nothing: {end}"
  );

  let told: Vec<serde_json::Value> = round
    .standing
    .heard
    .events("draft")
    .into_iter()
    .filter(|event| event["kind"] == "end")
    .collect();

  assert_eq!(told.len(), 1, "{told:?}");
  assert_eq!((&told[0]["session"], &told[0]["loaded"]), (&end["session"], &end["loaded"]), "{told:?}");
}

fn pinned(sent: &[String]) -> Option<i64> {
  sent
    .first()
    .and_then(|line| serde_json::from_str::<serde_json::Value>(line).ok())
    .and_then(|line| line.get("replyToMessageId").and_then(serde_json::Value::as_i64))
}

async fn pinned_rows(round: &Drafting) -> Vec<String> {
  round
    .scratch
    .texts(
      r#"SELECT "reply_to_message_id"::text FROM "messages"
         WHERE "chat_id" = 42 AND "agent_name" IS NOT NULL AND "reply_to_message_id" IS NOT NULL"#,
    )
    .await
}

#[tokio::test]
async fn a_picture_with_no_caption_reaches_the_turn_as_its_saved_path() {
  let session = StandInSession::new(SESSION, "unscripted");
  let round = Drafting::new(session, &["final"], &["final"], 150).await;
  let mut picture = Cursor::new(Vec::new());

  RgbImage::from_pixel(8, 8, Rgb([10, 200, 30]))
    .write_to(&mut picture, ImageFormat::Jpeg)
    .unwrap();
  round
    .scratch
    .run(r#"INSERT INTO "photo_chats" ("chat_id", "look") VALUES ($1, 1)"#, vec![OWNER.into()])
    .await;

  let file = MessageFile {
    id: "1".into(),
    name: "photo.jpg".into(),
    mime: "image/jpeg".into(),
    bytes: picture.into_inner(),
    kind: None,
  };
  let context = Context {
    carries_file: true,
    spoken_text: Some(String::new()),
    ..Context::default()
  };

  round
    .standing
    .receiver
    .attachment(&said(1, ""), &context, owners_room(), &[file])
    .await
    .unwrap();
  round.ended(1).await;

  let prompt = &round.session.prompts()[0];
  let data = round._data.path().join(format!("images/{OWNER}/1.jpg"));

  assert!(prompt.contains(&data.to_string_lossy().into_owned()), "{prompt}");
  assert!(prompt.contains("[attached image saved at"), "{prompt}");
  assert!(
    !prompt.contains(": \n") && !prompt.contains(": \r\n"),
    "the line is not empty: {prompt}"
  );
}

#[tokio::test]
async fn an_answer_to_the_newest_line_goes_unpinned() {
  let session = StandInSession::new(SESSION, "unscripted").answering(&["the message itself"]);
  let round = Drafting::new(session, &["final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  round.ended(1).await;

  let sent = round.sent();

  assert_eq!(sent.len(), 1, "{sent:?}");
  assert_eq!(pinned(&sent), None, "a direct answer carries no reply pin: {sent:?}");
  assert!(pinned_rows(&round).await.is_empty(), "the record carries no visible pin either");
}

#[tokio::test]
async fn an_answer_marked_to_an_older_line_is_stored_and_sent_as_its_reply() {
  let session = StandInSession::new(SESSION, "unscripted").answering(&["<<REPLY:1>> the message itself"]);
  let round = Drafting::new(session, &["final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  round.hear(2, "and this?").await;
  round.ended(1).await;

  let sent = round.sent();

  assert_eq!(sent.len(), 1, "{sent:?}");
  assert_eq!(pinned(&sent), Some(1), "the older line named by the marker is pinned: {sent:?}");
  assert_eq!(pinned_rows(&round).await, ["1"], "the record names the older message");
}

#[tokio::test]
async fn an_unmarked_answer_goes_out_plain_though_an_older_line_asked() {
  let session = StandInSession::new(SESSION, "unscripted").answering(&["the message itself"]);
  let round = Drafting::new(session, &["final"], &["final"], 150).await;

  round.hear(1, "what does the plan for this week hold?").await;
  round.hear(2, "noted").await;
  round.ended(1).await;

  let sent = round.sent();

  assert_eq!(sent.len(), 1, "{sent:?}");
  assert_eq!(pinned(&sent), None, "nothing pins a message the agent left unmarked: {sent:?}");
  assert!(pinned_rows(&round).await.is_empty(), "the record carries no visible pin either");
}

#[tokio::test]
async fn a_bare_reply_mark_pins_the_newest_line_the_round_answers() {
  let session = StandInSession::new(SESSION, "unscripted").answering(&["<<REPLY>> the message itself"]);
  let round = Drafting::new(session, &["final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  round.hear(2, "and this?").await;
  round.ended(1).await;

  let sent = round.sent();

  assert_eq!(sent.len(), 1, "{sent:?}");
  assert_eq!(
    pinned(&sent),
    Some(2),
    "a mark naming no message pins the newest the round answers: {sent:?}"
  );
  assert_eq!(pinned_rows(&round).await, ["2"], "the record names the pinned message");
  assert!(!sent[0].contains("<<REPLY"), "the mark never reaches the room: {sent:?}");
}

#[tokio::test]
async fn an_explicit_reply_mark_pins_even_the_newest_line() {
  let session = StandInSession::new(SESSION, "unscripted").answering(&["<<REPLY #1>> the message itself"]);
  let round = Drafting::new(session, &["final"], &["final"], 150).await;

  round.hear(1, ASKED).await;
  round.ended(1).await;

  let sent = round.sent();

  assert_eq!(sent.len(), 1, "{sent:?}");
  assert_eq!(pinned(&sent), Some(1), "the mark pins the newest line: {sent:?}");
  assert_eq!(pinned_rows(&round).await, ["1"], "the record names the pinned message");
}

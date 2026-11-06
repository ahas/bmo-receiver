//! A room's round carried whole (docs/wake.md, 「The room's queue and the round」; docs/bmo-window.md, 「Every
//! road into the agent」): the owner's words filed in his chat wake the default answerer's conversations, the round
//! composes its turn and carries it into the agency's session, its answer goes back to the room, and words that
//! come into the room while the turn works and that the turn does not take wait, and the round's closing check
//! shows them before its message goes out. Words a turn takes while it works move its turn in the live picture on
//! and out of the queue, and each carries a verdict of what the round's marks made of it.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::conversation::Timings;
use bmo_receiver::chat::runner::Runner;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::events::Events;
use bmo_receiver::receiver::{Context, Inbound, Room};
use receiver_policy::chat_line::CHAT_LINE;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::consult::CONSULT;
use receiver_policy::drafting::DRAFTING;
use receiver_policy::fill_marks;
use receiver_policy::handover::HANDOVER;
use receiver_ports::agency::Said;
use serde_json::{Value, json};

use crate::integration::common::Scratch;
use crate::integration::common::session::StandInSession;
use crate::integration::common::units::{
  ADDRESS, AGENT, BRAIN, OWNER, ROUND_RUN, StandInChat, Standing, deployment, registration, unrun, woken,
};

/// What the session's every turn ends with.
const ANSWER: &str = "Here is what was asked.";

/// The session the default answerer's turns go on.
const SESSION: &str = "agent-a@room";

const FIRST: &str = "the first words";
const LATER: &str = "words said while the turn works";

// The owner's private chat, the room every round of these tests runs in.
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

// The default answerer's documents a room's block cannot go out without, under the data root.
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

/// A round of the owner's chat, standing on the receiver's own conversations and the stand-in units, whose
/// session holds its first turn working until words said meanwhile are waiting in the room.
struct Round {
  _scratch: Scratch,
  _data: tempfile::TempDir,
  standing: Standing,
  chat: Arc<StandInChat>,
  session: Arc<StandInSession>,
  runner: Runner,
}

impl Round {
  async fn said_meanwhile() -> Self {
    Round::timed(timings()).await
  }

  async fn timed(timings: Timings) -> Self {
    let round = Round::stood(timings, StandInSession::new(SESSION, ANSWER), None).await;

    round.hear(1, FIRST).await;
    until("the first turn", || round.session.working()).await;
    round.hear(2, LATER).await;
    until("the words said meanwhile waiting", || round.runner.of(AGENT).buffered(OWNER) == [2]).await;
    // A look at the working turn passes, and the stand-in agency takes nothing pushed into it.
    tokio::time::sleep(Duration::from_millis(50)).await;
    round.session.let_go();
    until("the rounds' end", || round.runner.idle() && !round.session.working()).await;

    round
  }

  // The round's standing before a word is heard, its session holding the first turn working: its news the
  // receiver's own stream where `news` stands, else the recording hearer.
  async fn stood(timings: Timings, session: StandInSession, news: Option<Arc<Events>>) -> Self {
    let scratch = Scratch::new().await;
    crate::integration::common::session::no_drafting(scratch.db()).await;
    let data = tempfile::tempdir().unwrap();
    let chat = Arc::new(StandInChat::new("road-a").owners(OWNER));

    documents(data.path());

    let mut standing =
      woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat.clone()], deployment()));

    if let Some(news) = news {
      standing.receiver = standing.receiver.with_news(news);
    }

    let session = Arc::new(session.holding_the_first());
    let receiver = standing.receiver.clone();
    let runner = Runner::new(receiver.clone(), Arc::new(Around::new(receiver.clone())), timings, AGENT);

    standing.agency.registering(vec![registration(AGENT, "Agent A", vec![ADDRESS], None)]);
    standing.agency.carrying_on(session.clone());
    receiver.hold_runner(Arc::new(runner.clone()));

    Round {
      _scratch: scratch,
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

  fn pushes(&self) -> Vec<(String, i64, String)> {
    self.standing.agency.pushes.lock().unwrap().clone()
  }

  // Every send handed to the room's chat service, as the words it carried.
  fn sent(&self) -> Vec<String> {
    self.chat.sent().iter().map(|send| serde_json::to_string(send).unwrap()).collect()
  }
}

// The first turn carried the first words alone, on the session the agency started for the agent; the words said
// meanwhile did not go into it, and the closing check showed them before the message went out.
fn shown_in_the_closing_check(round: &Round) {
  let prompts = round.session.prompts();

  assert!(prompts[0].contains(FIRST), "the first turn carries the words that woke it");
  assert!(!prompts[0].contains(LATER), "words said after the turn went out are not in its prompt");
  assert!(
    round.pushes().iter().all(|(_, _, words)| words.contains(LATER)),
    "only the words said meanwhile are offered to the working turn: {:?}",
    round.pushes()
  );
  assert_eq!(prompts.len(), 2, "the closing check is the round's one more turn");
  assert!(prompts[1].contains(&DRAFTING.arrived_marker), "{}", prompts[1]);
  assert!(
    !prompts[1].contains("Answer with the message as it should now go out"),
    "the closing check's instruction stands in the room's block: {}",
    prompts[1]
  );
  assert!(
    prompts[0].contains(&receiver_policy::fill_marks(&DRAFTING.arrived, &[("voice", &DRAFTING.voice)])),
    "the room's block carries the closing check's rule"
  );
  assert!(prompts[1].contains(LATER), "the closing check shows the words said meanwhile");
}

#[tokio::test]
async fn words_said_while_the_turn_works_wait_and_the_closing_check_shows_them() {
  let round = Round::said_meanwhile().await;

  shown_in_the_closing_check(&round);

  let sent = round.sent();

  assert_eq!(
    sent.len(),
    1,
    "the round answers the room once, and the words it showed call no round of their own"
  );
  assert!(sent[0].contains(ANSWER), "the closing check's answer is what goes back to the room");
}

// A round whose working turn takes the words said meanwhile as they are pushed, its session answering `answer`, on
// the receiver's own stream: the stream, and the picture as it stood once the push went in.
async fn pushed_into(answer: &str) -> (Round, Arc<Events>, Value) {
  let events = Arc::new(Events::new());
  let round = Round::stood(timings(), StandInSession::new(SESSION, answer), Some(events.clone())).await;

  *round.standing.agency.saying.lock().unwrap() = Some(Said::Delivered);
  round.hear(1, FIRST).await;
  until("the first turn", || round.session.working()).await;
  round.hear(2, LATER).await;
  until("the words said meanwhile pushed into the turn", || {
    events.picture()["turns"][0]["messageId"] == 2
  })
  .await;

  let during = events.picture();

  round.session.let_go();
  until("the rounds' end", || round.runner.idle() && !round.session.working()).await;
  // The conversation stands idle a moment before its round's turn leaves the picture.
  until("the turn's end in the picture", || events.picture()["turns"] == json!([])).await;

  (round, events, during)
}

// The verdicts the stream heard of a message's stage.
fn verdicts(events: &Events, message_id: i64, stage: &str) -> Vec<Value> {
  events
    .held()
    .into_iter()
    .filter(|event| {
      event["type"] == "judgement" && event["messageId"] == message_id && event["stage"] == stage
    })
    .map(|event| event["verdict"].clone())
    .collect()
}

#[tokio::test]
async fn words_pushed_into_the_working_turn_leave_the_queue_and_move_the_turn_on_to_them() {
  let (round, events, during) = pushed_into(ANSWER).await;

  assert_eq!(round.pushes().len(), 1, "the words said meanwhile went into the working turn");
  assert_eq!(during["queued"], json!([]), "the turn read the pushed message: {during}");
  assert_eq!(during["turns"][0]["messageIds"], json!([1, 2]), "{during}");

  let after = events.picture();

  assert_eq!((after["turns"].clone(), after["queued"].clone()), (json!([]), json!([])), "{after}");
  assert_eq!(
    round.sent().len(),
    1,
    "the round answers both at once, and the pushed words open no round"
  );
}

#[tokio::test]
async fn a_message_pushed_into_the_round_carries_what_its_mark_made_of_it() {
  let (_, events, _) = pushed_into(ANSWER).await;

  assert_eq!(verdicts(&events, 2, "read-with"), [json!("READ: #1")], "the round's message answers it");
  assert!(
    verdicts(&events, 1, "chat")
      .iter()
      .any(|verdict| verdict.as_str().is_some_and(|said| said.starts_with("REPLY"))),
    "the round's own verdict stands under the message it woke on"
  );

  let (_, events, _) = pushed_into(&format!("<<DONE #2>> {ANSWER}")).await;

  assert_eq!(
    verdicts(&events, 2, "read-with"),
    [json!("SKIP: #1")],
    "marked done, it needs no answer"
  );

  let (_, events, _) = pushed_into(&format!("<<SEPARATE #2>> {ANSWER}")).await;

  assert_eq!(
    verdicts(&events, 2, "read-with"),
    Vec::<Value>::new(),
    "marked separate, it is its own round's"
  );
}

#[tokio::test]
async fn words_the_closing_check_shows_leave_the_queue_and_carry_their_verdict() {
  let events = Arc::new(Events::new());
  let round = Round::stood(timings(), StandInSession::new(SESSION, ANSWER), Some(events.clone())).await;

  round.hear(1, FIRST).await;
  until("the first turn", || round.session.working()).await;
  round.hear(2, LATER).await;
  until("the words said meanwhile waiting", || round.runner.of(AGENT).buffered(OWNER) == [2]).await;
  assert_eq!(events.picture()["queued"][0]["messageId"], 2, "the words wait for a turn to read them");
  round.session.let_go();
  until("the rounds' end", || round.runner.idle() && !round.session.working()).await;
  until("the turn's end in the picture", || events.picture()["turns"] == json!([])).await;

  shown_in_the_closing_check(&round);
  assert_eq!(events.picture()["queued"], json!([]), "the closing check's turn read them");
  assert_eq!(verdicts(&events, 2, "read-with"), [json!("READ: #1")]);
  assert!(
    events
      .held()
      .iter()
      .any(|event| event["type"] == "live" && event["live"]["turns"][0]["messageId"] == 2),
    "the turn moved on to the words its closing check shows"
  );
}

#[tokio::test]
async fn a_turn_names_its_round_and_the_answer_is_attached_to_the_rounds_newest_run() {
  let round = Round::said_meanwhile().await;
  let turns = round.session.turns.lock().unwrap().clone();
  let named = turns[0].round.clone().expect("the turn names its round");
  let kept = round
    ._scratch
    .texts(&format!(
      r#"SELECT "message_id"::text FROM "messages" WHERE "chat_id" = {OWNER} AND "round" = '{named}'"#
    ))
    .await;

  assert_eq!(kept.len(), 1, "the round's answer is kept under the round the turn named");
  assert_eq!(*round.standing.agency.rounds_asked.lock().unwrap(), [named]);
  assert_eq!(
    *round.standing.agency.attached.lock().unwrap(),
    [(ROUND_RUN, kept[0].parse::<i64>().unwrap())],
    "the answer, as the room keeps it, is attached to the newest run the round opened",
  );
}

#[tokio::test]
async fn a_turn_hands_the_agency_no_base_persona_even_where_the_deployment_keeps_one() {
  const BASE: &str = "the deployment's base persona";

  let scratch = Scratch::new().await;

  crate::integration::common::session::no_drafting(scratch.db()).await;
  let data = tempfile::tempdir().unwrap();
  let chat = Arc::new(StandInChat::new("road-a").owners(OWNER));
  let persona = data.path().join("prompts").join("persona");

  documents(data.path());
  std::fs::create_dir_all(&persona).unwrap();
  std::fs::write(persona.join("base.md"), BASE).unwrap();

  let standing = woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat.clone()], deployment()));
  let session = Arc::new(StandInSession::new(SESSION, ANSWER));
  let receiver = standing.receiver.clone();
  let runner = Runner::new(receiver.clone(), Arc::new(Around::new(receiver.clone())), timings(), AGENT);

  standing.agency.registering(vec![registration(AGENT, "Agent A", vec![ADDRESS], None)]);
  standing.agency.carrying_on(session.clone());
  receiver.hold_runner(Arc::new(runner.clone()));
  receiver.message(&said(1, FIRST), &Context::default(), owners_room()).await.unwrap();
  until("the round's end", || {
    !session.prompts().is_empty() && runner.idle() && !session.working()
  })
  .await;

  let body = serde_json::to_value(&session.turns.lock().unwrap()[0]).unwrap();

  assert!(body.get("basePersona").is_none(), "the turn's body names no base persona");
  assert!(
    !body.to_string().contains(BASE),
    "the deployment's base persona file goes nowhere in the turn"
  );
}

#[tokio::test]
async fn a_rounds_judgements_are_attached_from_its_start_under_the_answer_as_the_room_keeps_it() {
  let before = bmo_receiver::now_ms();
  let round = Round::said_meanwhile().await;
  let attached = round.standing.gate.attached.lock().unwrap().clone();
  let kept: Vec<i64> = round
    ._scratch
    .texts(r#"SELECT "message_id"::text FROM "messages" WHERE "chat_id" = 42 AND "agent_name" IS NOT NULL"#)
    .await
    .iter()
    .map(|number| number.parse().unwrap())
    .collect();

  assert_eq!(
    kept,
    [900],
    "the record keeps the answer the road carried under the number the road gave it"
  );
  assert_eq!(
    attached.len(),
    1,
    "the round's judgements are attached once its answer went out: {attached:?}"
  );

  let (room, message, since) = attached[0];

  assert_eq!((room, message), (OWNER, kept[0]), "they stand under the answer as the room keeps it");
  assert!(
    since >= before && since <= bmo_receiver::now_ms(),
    "they are the ones written since the round began, in epoch milliseconds: {since}"
  );

  let sent = round.standing.heard.events("judgement");

  assert!(
    sent
      .iter()
      .any(|event| event["stage"] == "send" && event["verdict"] == "SENT" && event["messageId"] == kept[0]),
    "the send's own verdict is heard on the stream under the answer: {sent:?}"
  );
}

#[tokio::test]
async fn an_answer_opens_the_window_for_the_policy_span_where_the_agents_row_names_none() {
  // No window is cut for the test: the span is the one the agent's row, which names none, leaves to the policy.
  let round = Round::timed(Timings {
    window: None,
    ..timings()
  })
  .await;

  tokio::time::sleep(Duration::from_millis(200)).await;

  assert!(!round.sent().is_empty(), "the round answered");
  assert!(
    round.standing.heard.events("window-closed").is_empty(),
    "no close follows the opening while the span runs"
  );

  let until: i64 = round
    ._scratch
    .texts(&format!(r#"SELECT "open_until"::text FROM "room_windows" WHERE "chat_id" = {OWNER}"#))
    .await
    .first()
    .and_then(|until| until.parse().ok())
    .unwrap_or(0);
  let span = CHAT_ROUND.conversation.window_seconds * 1000;

  assert!(
    until > bmo_receiver::now_ms() + span - 30_000,
    "the window stands open for the policy's span: {until}"
  );
}

#[tokio::test]
async fn a_conversation_closing_its_window_before_the_end_tells_the_window_closed() {
  let round = Round::said_meanwhile().await;

  assert!(round.standing.heard.events("window-closed").is_empty(), "the window still runs");

  round.runner.of(AGENT).close(OWNER).await;

  let closed = round.standing.heard.events("window-closed");

  assert_eq!(closed.len(), 1, "{closed:?}");
  assert_eq!(closed[0]["chatId"], OWNER);
  assert_eq!(closed[0]["agent"], AGENT);
  assert!(closed[0]["closedAt"].as_i64().is_some_and(|at| at <= bmo_receiver::now_ms()));
}

#[tokio::test]
async fn a_replaced_sessions_first_turn_carries_the_archived_messages_behind_the_note() {
  const NOTE: &str = "the note the replaced session left";
  const MISSED: &str = "words said around the handover";

  let scratch = Scratch::new().await;

  crate::integration::common::session::no_drafting(scratch.db()).await;
  let data = tempfile::tempdir().unwrap();
  let chat = Arc::new(StandInChat::new("road-a").owners(OWNER));

  documents(data.path());

  let standing = woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat.clone()], deployment()));
  let session = Arc::new(
    StandInSession::new(SESSION, ANSWER)
      .at_generation(3)
      .acting(crate::integration::common::session::writing_handoff(NOTE)),
  );
  let receiver = standing.receiver.clone();
  let runner = Runner::new(receiver.clone(), Arc::new(Around::new(receiver.clone())), timings(), AGENT);
  let mut agent = registration(AGENT, "Agent A", vec![ADDRESS], None);

  agent.row.insert("sessionScope".to_owned(), serde_json::json!("agent"));
  standing.agency.registering(vec![agent]);
  standing.agency.carrying_on(session.clone());
  receiver.hold_runner(Arc::new(runner.clone()));
  bmo_receiver::chat::cache::force_reset(
    scratch.db(),
    AGENT,
    receiver_codes::SessionReason::DowngradeForced,
  )
  .await
  .unwrap();
  // The standing session has answered in this room before; the turn is a handover all the same, and the
  // messages behind the note ride it.
  scratch
    .run(
      r#"INSERT INTO "bmo_session_rooms" ("session_id", "chat_id") VALUES ($1, $2)"#,
      vec![SESSION.into(), OWNER.into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via")
         VALUES ($1, 5, $1, 'tester', 1760000000000, $2, 'road-a')"#,
      vec![OWNER.into(), MISSED.into()],
    )
    .await;

  receiver.message(&said(6, FIRST), &Context::default(), owners_room()).await.unwrap();
  until("the round's end", || session.prompts().len() > 1 && runner.idle() && !session.working()).await;

  // The old session was asked for its handoff first; the new one's first turn is the round's.
  let prompt = &session.prompts()[1];

  assert!(
    prompt.contains(NOTE),
    "the note rides at the head of the new session's first turn: {prompt}"
  );
  assert!(prompt.contains("Archived messages since your last reply"), "{prompt}");
  assert!(prompt.contains(MISSED), "what the agent never read reaches the new session: {prompt}");
  assert!(
    !prompt.contains("left out of this block"),
    "within the cap nothing is left out: {prompt}"
  );
  assert!(prompt.find(NOTE) < prompt.find(MISSED), "the note stands ahead of the turn");

  // The brain's rule and the session-opening index stand in the room's block, and no walk rides the turn.
  assert!(
    prompt.contains(&CHAT_ROUND.index.rule),
    "the brain's rule stands in the block: {prompt}"
  );
  assert!(
    prompt.contains(&fill_marks(&CHAT_ROUND.index.memories, &[("chat", &OWNER.to_string())])),
    "the memories rule stands in the block: {prompt}"
  );
  assert!(!prompt.contains(&CHAT_ROUND.index.walk_head), "no walk rides the turn: {prompt}");

  let rules = [
    fill_marks(&CHAT_ROUND.notice.rule, &[("marker", &CHAT_ROUND.notice.marker)]),
    fill_marks(&CHAT_ROUND.notice.optional_rule, &[("marker", &CHAT_ROUND.notice.optional_marker)]),
    CONSULT.windows.rule.clone(),
    fill_marks(&CONSULT.work.rule, &[("marker", &CONSULT.work.marker)]),
    fill_marks(&HANDOVER.chat.ask_rule, &[("marker", &HANDOVER.chat.ask_marker)]),
    fill_marks(&HANDOVER.chat.consolidate_rule, &[("marker", &HANDOVER.chat.consolidate_marker)]),
    fill_marks(&HANDOVER.chat.handed_rule, &[("marker", &HANDOVER.chat.handed_marker)]),
    fill_marks(&HANDOVER.chat.carried_rule, &[("marker", &HANDOVER.chat.carried_marker)]),
    fill_marks(&HANDOVER.chat.lost_rule, &[("marker", &HANDOVER.chat.lost_marker)]),
  ];

  for rule in &rules {
    assert!(prompt.contains(rule), "the moved rule stands in the block: {rule}");
  }
}

#[tokio::test]
async fn a_turn_carries_the_newest_unread_messages_up_to_the_cap_and_counts_the_rest() {
  let scratch = Scratch::new().await;
  crate::integration::common::session::no_drafting(scratch.db()).await;
  let data = tempfile::tempdir().unwrap();
  let chat = Arc::new(StandInChat::new("road-a").owners(OWNER));

  documents(data.path());

  let standing = woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat.clone()], deployment()));
  let session = Arc::new(StandInSession::new(SESSION, ANSWER));
  let receiver = standing.receiver.clone();
  let runner = Runner::new(receiver.clone(), Arc::new(Around::new(receiver.clone())), timings(), AGENT);
  let cap = CHAT_ROUND.archived_messages as i64;
  let unread = cap + 3;

  standing.agency.registering(vec![registration(AGENT, "Agent A", vec![ADDRESS], None)]);
  standing.agency.carrying_on(session.clone());
  receiver.hold_runner(Arc::new(runner.clone()));
  scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via")
         SELECT $1, "n", $1, 'tester', 1700000000 + "n", 'unread words ' || lpad("n"::text, 3, '0'), 'road-a'
         FROM generate_series(1, $2) AS "n""#,
      vec![OWNER.into(), unread.into()],
    )
    .await;
  bmo_receiver::receiver::rows::set_last_read(scratch.db(), OWNER, 0).await.unwrap();

  receiver
    .message(&said(unread + 1, FIRST), &Context::default(), owners_room())
    .await
    .unwrap();
  until("the round's end", || {
    !session.prompts().is_empty() && runner.idle() && !session.working()
  })
  .await;

  let prompt = &session.prompts()[0];
  let words = |n: i64| format!("unread words {n:03}");

  assert!(
    prompt.contains(&format!("Archived messages since your last reply ({cap}):")),
    "{prompt}"
  );
  assert!(
    prompt.contains("3 older unread messages"),
    "the block counts what it left out: {prompt}"
  );
  assert!(!prompt.contains(&words(3)), "the oldest unread are left in the archive: {prompt}");
  assert!(prompt.contains(&words(4)) && prompt.contains(&words(unread)), "{prompt}");
  assert!(prompt.find(&words(4)) < prompt.find(&words(unread)), "the carried stand in their order");
}

// What a round on a session past every fold share came to: the folds asked of it, the folds reported to the agent's
// brain, the consolidation turns the session took, and how many folds had been asked when the first of them ran.
struct Folded {
  asked: Vec<String>,
  reported: Vec<(i64, String)>,
  consolidations: usize,
  folds_before: Option<usize>,
}

// A round on a session past every fold share, its program carrying its own fold or not, the session refusing the
// consolidation turn or not.
async fn folds_after_a_round(folds_itself: bool, refuses: bool) -> Folded {
  let scratch = Scratch::new().await;

  crate::integration::common::session::no_drafting(scratch.db()).await;
  let data = tempfile::tempdir().unwrap();
  let chat = Arc::new(StandInChat::new("road-a").owners(OWNER));

  documents(data.path());

  let standing = woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat.clone()], deployment()));
  let consolidate = bmo_receiver::chat::consolidation::instruction();
  let folds_before = Arc::new(std::sync::Mutex::new(None));
  let seen = folds_before.clone();
  let agency = standing.agency.clone();
  let asked = consolidate.clone();
  let mut session = StandInSession::new(SESSION, ANSWER).full(folds_itself).acting(move |turn| {
    if turn.prompt.contains(&asked) {
      seen.lock().unwrap().get_or_insert(agency.folds.lock().unwrap().len());
    }
  });

  if refuses {
    session = session.refusing(&consolidate);
  }

  let session = Arc::new(session);
  let receiver = standing.receiver.clone();
  let runner = Runner::new(receiver.clone(), Arc::new(Around::new(receiver.clone())), timings(), AGENT);

  standing.agency.registering(vec![registration(AGENT, "Agent A", vec![ADDRESS], None)]);
  standing.agency.carrying_on(session.clone());
  receiver.hold_runner(Arc::new(runner.clone()));
  receiver.message(&said(1, FIRST), &Context::default(), owners_room()).await.unwrap();
  until("the round's end", || {
    !session.prompts().is_empty() && runner.idle() && !session.working()
  })
  .await;

  if !folds_itself {
    until("the fold reported", || !standing.brain.folds().is_empty()).await;
  } else {
    tokio::time::sleep(Duration::from_millis(300)).await;
  }

  let consolidations = session.prompts().iter().filter(|prompt| prompt.contains(&consolidate)).count();
  let folds_before = *folds_before.lock().unwrap();

  Folded {
    asked: standing.agency.folds.lock().unwrap().clone(),
    reported: standing.brain.folds(),
    consolidations,
    folds_before,
  }
}

#[tokio::test]
async fn a_session_whose_program_carries_its_own_fold_is_never_folded_by_the_receiver() {
  let folded = folds_after_a_round(false, false).await;

  assert_eq!(folded.asked, vec![SESSION.to_owned()], "a full session is folded");
  assert_eq!(folded.consolidations, 1, "the session is asked one consolidation turn");
  assert_eq!(folded.folds_before, Some(0), "the consolidation turn runs before the fold");
  assert_eq!(
    folded.reported,
    vec![(BRAIN, SESSION.to_owned())],
    "the fold is reported to the agent's brain under the session's id"
  );

  let folded = folds_after_a_round(true, false).await;

  assert!(folded.asked.is_empty(), "the program's own fold stands: none is asked");
  assert_eq!(folded.consolidations, 0, "nor is a consolidation turn");
  assert!(folded.reported.is_empty(), "a fold the receiver did not ask is reported by nobody here");
}

#[tokio::test]
async fn a_refused_consolidation_turn_never_holds_the_fold() {
  let folded = folds_after_a_round(false, true).await;

  assert_eq!(folded.consolidations, 0, "the refused turn was not taken");
  assert_eq!(folded.asked, vec![SESSION.to_owned()], "the fold goes ahead");
  assert_eq!(folded.reported, vec![(BRAIN, SESSION.to_owned())]);
}

// The first prompt of a round of the owner's words, the agent's brain saying a consolidation is due or not.
async fn first_prompt_with_due(due: bool) -> String {
  let scratch = Scratch::new().await;

  crate::integration::common::session::no_drafting(scratch.db()).await;
  let data = tempfile::tempdir().unwrap();
  let chat = Arc::new(StandInChat::new("road-a").owners(OWNER));

  documents(data.path());

  let standing = woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat.clone()], deployment()));
  let session = Arc::new(StandInSession::new(SESSION, ANSWER));
  let receiver = standing.receiver.clone();
  let runner = Runner::new(receiver.clone(), Arc::new(Around::new(receiver.clone())), timings(), AGENT);

  *standing.brain.due.lock().unwrap() = due;
  standing.agency.registering(vec![registration(AGENT, "Agent A", vec![ADDRESS], None)]);
  standing.agency.carrying_on(session.clone());
  receiver.hold_runner(Arc::new(runner.clone()));
  receiver.message(&said(1, FIRST), &Context::default(), owners_room()).await.unwrap();
  until("the round's end", || {
    !session.prompts().is_empty() && runner.idle() && !session.working()
  })
  .await;

  session.prompts()[0].clone()
}

#[tokio::test]
async fn a_turn_says_a_consolidation_is_due_only_where_the_agent_s_brain_says_so() {
  let due = &CHAT_ROUND.index.due;
  let prompt = first_prompt_with_due(true).await;

  assert!(prompt.contains(due.as_str()), "{prompt}");
  assert!(
    prompt.find(CHAT_ROUND.index.hint.as_str()) < prompt.find(due.as_str()),
    "the due line stands after the index's hint: {prompt}"
  );

  let prompt = first_prompt_with_due(false).await;

  assert!(!prompt.contains(due.as_str()), "{prompt}");
}

// The owner's words in a room that is not his private room, as the road hands them over.
fn owners_words_in(room: i64, message_id: i64, text: &str) -> Inbound {
  Inbound {
    chat_id: room,
    ..said(message_id, text)
  }
}

#[tokio::test]
async fn a_round_of_the_owners_own_words_is_privileged_in_any_room_after_a_quiet_instructions_round() {
  use bmo_receiver::chat::instructions::Rooms;

  const LAB: i64 = -9_000_000_000_003;

  let scratch = Scratch::new().await;

  crate::integration::common::session::no_drafting(scratch.db()).await;
  let data = tempfile::tempdir().unwrap();
  let chat = Arc::new(StandInChat::new("road-a").holding(&[LAB]).owners(OWNER));

  documents(data.path());

  let standing = woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat.clone()], deployment()));
  let session = Arc::new(StandInSession::new(SESSION, ANSWER));
  let receiver = standing.receiver.clone();
  let runner = Runner::new(receiver.clone(), Arc::new(Around::new(receiver.clone())), timings(), AGENT);
  let lab = Room {
    owner_room: false,
    identity: Some(7),
    private_to: None,
  };

  standing.agency.registering(vec![registration(AGENT, "Agent A", vec![ADDRESS], None)]);
  standing.agency.carrying_on(session.clone());
  receiver.hold_runner(Arc::new(runner.clone()));

  // A job's end opens the room's conversation as a quiet instruction, and its round answers.
  runner
    .instruct(AGENT, LAB, "A job of the room ended.".to_owned(), 1, None, vec![])
    .await;
  until("the instruction's round", || {
    session.prompts().len() == 1 && runner.idle() && !session.working()
  })
  .await;

  // Then the owner speaks in the same room while the conversation still stands.
  receiver
    .message(
      &owners_words_in(LAB, 2, &format!("{ADDRESS}, file this as an item.")),
      &Context::default(),
      lab,
    )
    .await
    .unwrap();
  until("the owner's round", || {
    session.prompts().len() == 2 && runner.idle() && !session.working()
  })
  .await;

  let turns = session.turns.lock().unwrap().clone();

  assert_eq!(
    turns[0].privileged,
    Some(false),
    "a quiet instruction outside his private room is not his"
  );
  assert_eq!(turns[1].privileged, Some(true), "the owner's own words are privileged in any room");
}

#[tokio::test]
async fn an_agents_line_beside_the_owners_words_in_one_round_leaves_the_round_his() {
  const LAB: i64 = -9_000_000_000_003;
  const AGENT_LINE: &str = "a supervisor's answer in the room";

  let scratch = Scratch::new().await;

  crate::integration::common::session::no_drafting(scratch.db()).await;
  let data = tempfile::tempdir().unwrap();
  let chat = Arc::new(StandInChat::new("road-a").holding(&[LAB]).owners(OWNER));

  documents(data.path());

  let standing = woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat.clone()], deployment()));
  let session = Arc::new(StandInSession::new(SESSION, ANSWER));
  let receiver = standing.receiver.clone();
  // The queue gathers long enough for the agent's line and the owner's words to go in one round, as they did live.
  let gathering = Timings {
    queue: Duration::from_millis(400),
    ..timings()
  };
  let runner = Runner::new(receiver.clone(), Arc::new(Around::new(receiver.clone())), gathering, AGENT);
  let lab = Room {
    owner_room: false,
    identity: Some(7),
    private_to: None,
  };

  standing.agency.registering(vec![registration(AGENT, "Agent A", vec![ADDRESS], None)]);
  standing.agency.carrying_on(session.clone());
  receiver.hold_runner(Arc::new(runner.clone()));

  // The owner opens the room's conversation, and its round answers.
  receiver
    .message(
      &owners_words_in(LAB, 1, &format!("{ADDRESS}, are you there?")),
      &Context::default(),
      lab,
    )
    .await
    .unwrap();
  until("the first round", || {
    session.prompts().len() == 1 && runner.idle() && !session.working()
  })
  .await;

  // While it stands, a supervisor's answer is filed in the room and the owner speaks again: one round takes both.
  let line = receiver.speech().kept(LAB, AGENT_LINE, None, Some("agent-b"), None).await.unwrap();

  receiver.filed(LAB, line, "record", lab).await.unwrap();
  receiver
    .message(
      &owners_words_in(LAB, 3, &format!("{ADDRESS}, file this as an item.")),
      &Context::default(),
      lab,
    )
    .await
    .unwrap();
  until("the owner's round", || {
    session.prompts().iter().any(|prompt| prompt.contains("file this as an item"))
      && runner.idle()
      && !session.working()
  })
  .await;

  let turns = session.turns.lock().unwrap().clone();
  let owners = turns.iter().find(|turn| turn.prompt.contains("file this as an item")).unwrap();

  assert!(
    owners.prompt.contains(AGENT_LINE),
    "the round took the agent's line with the owner's words"
  );
  assert_eq!(
    owners.privileged,
    Some(true),
    "an agent's line neither grants nor takes the owner's privilege"
  );
}

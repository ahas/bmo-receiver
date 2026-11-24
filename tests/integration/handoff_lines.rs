//! What the agency's handoff asking tells and keeps beyond the handoff itself (docs/api.md, 「Handoffs」;
//! docs/unified-bmo.md, 「Replacing the session」 and 「The room's line」): a clean asking has the standing session
//! consolidate and fold with no handoff, keeps nothing, and the fresh session opens on no note, its room told once
//! as a reset; an asking that asks no session tells the room's line alone and touches no other word; and an asking
//! that finds no standing session still tells one line.

use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::conversation::Timings;
use bmo_receiver::chat::handoff::{carried_key, clean_key, lost_key, pending_key};
use bmo_receiver::chat::runner::Runner;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::{Context, Inbound, Room};
use bmo_receiver::routes::routes;
use bmo_receiver::store::settings;
use receiver_policy::fill_marks;
use receiver_policy::handover::HANDOVER;
use serde_json::{Value, json};

use crate::integration::common::Scratch;
use crate::integration::common::queue::{TOKEN, posted_to};
use crate::integration::common::session::{
  StandInSession, handoff_path, prompt_documents, writing_handoff,
};
use crate::integration::common::units::{
  ADDRESS, AGENT, OWNER, StandInChat, Standing, deployment, registration, unrun, woken,
};

const NOTE: &str = "what the standing session was in the middle of";

/// A room of the chat service that is not the owner's private chat.
const OTHER: i64 = -7;

struct Stood {
  scratch: Scratch,
  _data: tempfile::TempDir,
  standing: Standing,
  runner: Runner,
  chat: Arc<StandInChat>,
}

impl Stood {
  // A receiver whose default answerer keeps one session across its rooms where `unified`, else one in each room,
  // stood with documents, a chat service holding the owner's chat and one other room, the agent registered, the
  // session given, and a runner taking its rounds and the handoffs asked of it. No round updates a room's
  // situation note first and no stage follows a turn, so a round's own turn is the only one it sends.
  async fn new(session: Arc<StandInSession>, unified: bool) -> Self {
    let scratch = Scratch::new().await;

    crate::integration::common::session::no_stages(scratch.db()).await;

    let data = tempfile::tempdir().unwrap();
    let chat = Arc::new(StandInChat::new("road-a").owners(OWNER).holding(&[OTHER]));

    prompt_documents(data.path());

    let mut standing =
      woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat.clone()], deployment()));

    standing.receiver = standing.receiver.with_situation_updates(false);

    let receiver = standing.receiver.clone();
    let timings = Timings {
      queue: Duration::from_millis(1),
      wait: Duration::from_secs(60),
      window: Some(Duration::from_secs(60)),
      look: Duration::from_millis(5),
    };
    let runner = Runner::new(receiver.clone(), Arc::new(Around::new(receiver.clone())), timings, AGENT);
    let mut agent = registration(AGENT, "Agent A", vec![ADDRESS], None);

    if unified {
      agent.row.insert("sessionScope".to_owned(), json!("agent"));
    }

    standing.agency.registering(vec![agent]);
    standing.agency.carrying_on(session);
    receiver.hold_runner(Arc::new(runner.clone()));
    receiver.clone().hold_touching(Arc::new(runner.clone()));
    bmo_receiver::receiver::rows::allow_chat(scratch.db(), OTHER).await.unwrap();

    Stood {
      scratch,
      _data: data,
      standing,
      runner,
      chat,
    }
  }

  async fn asked(&self, body: Value) -> (StatusCode, Value) {
    posted_to(routes(self.standing.receiver.clone(), TOKEN), "/handoffs/agent-a", body).await
  }

  // The `session-handed-over` lines a room's record keeps, in the order kept, each as the line it was said from.
  async fn lines(&self, chat_id: i64) -> Vec<Value> {
    self
      .scratch
      .texts(&format!(
        r#"SELECT "line" FROM "messages" WHERE "chat_id" = {chat_id} AND "kind" = 'fixed-line' ORDER BY "rowid""#
      ))
      .await
      .into_iter()
      .map(|line| serde_json::from_str::<Value>(&line).unwrap())
      .filter(|line| line["code"] == json!("session-handed-over"))
      .collect()
  }

  // Every row a room's record holds, whole, in the order of its numbers.
  async fn record(&self, chat_id: i64) -> Vec<String> {
    self
      .scratch
      .texts(&format!(
        r#"SELECT row_to_json(m)::text FROM "messages" m WHERE "chat_id" = {chat_id} ORDER BY "message_id""#
      ))
      .await
  }

  async fn kept(&self, key: &str) -> Option<String> {
    settings::get(self.scratch.db(), key).await.unwrap()
  }
}

fn handed(fields: Value) -> Value {
  json!({ "code": "session-handed-over", "fields": fields })
}

fn worded(message_id: i64, text: &str) -> Inbound {
  Inbound {
    chat_id: OWNER,
    message_id,
    sender_id: Some(OWNER),
    sender_name: "tester".into(),
    sent_at: 1_760_000_000_000 + message_id,
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

fn owners_room() -> Room {
  Room {
    owner_room: true,
    identity: Some(7),
    private_to: None,
    sealed: false,
  }
}

#[tokio::test]
async fn a_clean_asking_consolidates_with_no_handoff_keeps_nothing_and_the_fresh_session_opens_on_no_note()
{
  let session = Arc::new(
    StandInSession::new("agent-a@agent", "Here.")
      .at_generation(3)
      .acting(writing_handoff(NOTE)),
  );
  let stood = Stood::new(session.clone(), true).await;
  let db = stood.scratch.db();

  // A transcript and the word of a lost memory kept for the fresh session before are let go by a clean asking.
  settings::set(db, &carried_key(AGENT), "agent-a (room 1): an old answer").await.unwrap();
  settings::set(db, &lost_key(AGENT), "lost").await.unwrap();

  let sent = stood.chat.sent().len();
  let (status, body) = stood
    .asked(json!({ "reason": "a reset", "cause": "reset", "clean": true, "room": OTHER, "by": 3 }))
    .await;

  assert_eq!(status, StatusCode::NO_CONTENT, "{body}");

  // The one turn asked consolidates and folds by its marker, whose rule stands in the room's block, and names no
  // handoff file.
  let turns = session.turns.lock().unwrap().clone();

  assert_eq!(turns.len(), 1, "one turn is asked: {:?}", session.prompts());
  assert!(turns[0].prompt.starts_with(&HANDOVER.chat.clean_marker), "{}", turns[0].prompt);
  assert!(!turns[0].prompt.contains(&HANDOVER.chat.ask_marker), "{}", turns[0].prompt);
  assert_eq!(handoff_path(&turns[0].prompt), None, "no handoff file is named");

  let block = &turns[0].room.as_ref().expect("the turn carries its room's block").text;
  let rule = fill_marks(
    &HANDOVER.chat.clean_rule,
    &[("marker", &HANDOVER.chat.clean_marker), ("handoff", &HANDOVER.chat.ask_marker)],
  );

  assert!(block.contains(&rule), "the clean rule stands in the block");
  assert!(rule.contains(&HANDOVER.chat.ask_marker) && rule.contains("brain_fold"), "{rule}");

  // Nothing is kept for the fresh session, and it is marked to start clean.
  assert_eq!(stood.kept(&pending_key(AGENT)).await, None);
  assert_eq!(stood.kept(&carried_key(AGENT)).await, None);
  assert_eq!(stood.kept(&lost_key(AGENT)).await, None);
  assert!(stood.kept(&clean_key(AGENT, None)).await.is_some());

  // The room the asking named is told once, as a reset by the asker.
  assert_eq!(stood.lines(OTHER).await, [handed(json!({ "agent": AGENT, "cause": "reset", "by": 3 }))]);
  assert!(stood.lines(OWNER).await.is_empty(), "no other room is told");
  assert_eq!(stood.chat.sent().len(), sent, "nothing went to the chat service");

  // The fresh session the agency opens, even one its state says lost its handoff, opens on no note.
  session.now_at(None);
  session.handoff_lost(true);
  stood
    .standing
    .receiver
    .message(&worded(1, "hello again"), &Context::default(), owners_room())
    .await
    .unwrap();

  let waited = tokio::time::timeout(Duration::from_secs(20), async {
    while session.prompts().len() < 2 || !stood.runner.idle() || session.working() {
      tokio::time::sleep(Duration::from_millis(5)).await;
    }
  })
  .await;

  assert!(waited.is_ok(), "the round never ended");

  // A note rides at the head of the first turn; here the room's block, which names every marker's rule, comes
  // first.
  let first = &session.prompts()[1];

  for marker in [&HANDOVER.chat.handed_marker, &HANDOVER.chat.carried_marker, &HANDOVER.chat.lost_marker] {
    assert!(!first.starts_with(marker.as_str()), "the first turn opens on {marker}: {first}");
  }

  assert!(!first.contains("an old answer"), "no transcript rides the first turn: {first}");
  assert_eq!(stood.kept(&clean_key(AGENT, None)).await, None, "the mark is taken once");
  assert!(
    stood
      .scratch
      .texts(&format!(
        r#"SELECT "text" FROM "instructions" WHERE "chat_id" = {OWNER} AND "kind" = 'handoff-lost'"#
      ))
      .await
      .is_empty(),
    "the owner is told of no lost memory"
  );
  assert_eq!(stood.lines(OTHER).await.len(), 1, "the fresh session's turn tells nothing more");
}

#[tokio::test]
async fn an_asking_that_asks_no_session_tells_the_line_alone_and_touches_no_other_word() {
  let session = Arc::new(StandInSession::new("agent-a@agent", "Here.").at_generation(3));
  // The agent keeps one session in each room, whose handover the agency makes itself.
  let stood = Stood::new(session.clone(), false).await;

  stood
    .scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via")
         VALUES ($1, 5, 11, 'tester', 1760000000000, 'earlier words', 'road-a')"#,
      vec![OTHER.into()],
    )
    .await;

  let before = stood.record(OTHER).await;
  let sent = stood.chat.sent().len();
  let (status, body) = stood
    .asked(json!({ "reason": "the road moved", "cause": "road", "room": OTHER, "by": 5, "ask": false }))
    .await;

  assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
  assert!(session.prompts().is_empty(), "no session was asked");
  assert_eq!(stood.lines(OTHER).await, [handed(json!({ "agent": AGENT, "cause": "road", "by": 5 }))]);

  let after = stood.record(OTHER).await;

  assert_eq!(after.len(), before.len() + 1, "the line is all the room gained");
  assert!(
    before.iter().all(|row| after.contains(row)),
    "the room's earlier words stand as they were"
  );
  assert_eq!(stood.chat.sent().len(), sent, "nothing went to the chat service");
  assert!(stood.lines(OWNER).await.is_empty(), "no other room is told");

  // A clean reset the agency made of the room's session marks that room's fresh session to open on no note.
  let (status, _) = stood
    .asked(json!({ "reason": "a reset", "cause": "reset", "room": OTHER, "clean": true, "ask": false }))
    .await;

  assert_eq!(status, StatusCode::NO_CONTENT);
  assert!(session.prompts().is_empty(), "no session was asked");
  assert!(stood.kept(&clean_key(AGENT, Some(OTHER))).await.is_some());
  assert_eq!(stood.lines(OTHER).await.len(), 2);
  assert_eq!(stood.lines(OTHER).await[1], handed(json!({ "agent": AGENT, "cause": "reset" })));
}

#[tokio::test]
async fn an_asking_that_finds_no_standing_session_still_tells_one_line() {
  let session = Arc::new(StandInSession::new("agent-a@agent", "Here."));
  let stood = Stood::new(session.clone(), true).await;

  // The old program is down: the agency has no session to start.
  stood.standing.agency.none_standing();

  let (status, body) =
    stood.asked(json!({ "reason": "the program went down", "cause": "documents" })).await;

  assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
  assert!(session.prompts().is_empty(), "no session stood to ask");
  assert_eq!(
    stood.lines(OWNER).await,
    [handed(json!({ "agent": AGENT, "cause": "documents" }))],
    "the owner's private chat is told once, the asking naming no room"
  );
  assert_eq!(stood.kept(&pending_key(AGENT)).await, None, "nothing is kept");
}

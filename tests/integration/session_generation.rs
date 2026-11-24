//! A generation the agency opens under an agent's session (docs/unified-bmo.md, 「Replacing the session」): a turn
//! whose state names a generation past the one the session last answered on releases what the ended generation held,
//! once, and reports the fold to the brain; a resumed session and a return to another room release nothing. The
//! round log the console reads then starts from zero. A room's own session handed over by the agency tells its room
//! nothing of itself as a turn finds the generation: the agency asks for that line.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::conversation::Timings;
use bmo_receiver::chat::generation::{self, Standing};
use bmo_receiver::chat::handoff::pending_key;
use bmo_receiver::chat::runner::Runner;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::{Context, Inbound, Room};
use bmo_receiver::store::settings;
use receiver_policy::chat_line::CHAT_LINE;
use receiver_policy::handover::HANDOVER;
use receiver_ports::gate::{Entry, Source};
use serde_json::{Value, json};

use crate::integration::common::Scratch;
use crate::integration::common::session::{StandInSession, no_stages};
use crate::integration::common::units::{
  ADDRESS, AGENT, BRAIN, OWNER, StandInChat, Standing as StandingUnits, deployment, registration, unrun,
  woken,
};

/// The room-scoped session the default answerer's turns go on.
const SESSION: &str = "agent-a@room";

/// The one session across the agent's rooms.
const UNIFIED_SESSION: &str = "agent-a@agent";

/// A room of the chat service that is not the owner's private chat.
const OTHER: i64 = -7;

const ANSWER: &str = "Here is what was asked.";

const PIECE: &str = "knowledge/plans";
const PIECE_TEXT: &str = "the plan for the week";

fn owners_room() -> Room {
  Room {
    owner_room: true,
    identity: Some(7),
    private_to: None,
    sealed: false,
  }
}

fn a_group() -> Room {
  Room {
    owner_room: false,
    identity: None,
    private_to: None,
    sealed: false,
  }
}

fn said(chat_id: i64, message_id: i64, text: &str) -> Inbound {
  Inbound {
    chat_id,
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

async fn until(what: &str, holds: impl Fn() -> bool) {
  let waited = tokio::time::timeout(Duration::from_secs(20), async {
    while !holds() {
      tokio::time::sleep(Duration::from_millis(5)).await;
    }
  })
  .await;

  assert!(waited.is_ok(), "{what} never came");
}

fn piece() -> Entry {
  Entry {
    path: PIECE.to_owned(),
    text: PIECE_TEXT.to_owned(),
    source: Source::Knowledge,
    fragment: None,
    holds: None,
  }
}

/// The owner's chat with the default answerer's conversations standing on the stand-in units, its session answering
/// as the test scripts it.
struct Round {
  scratch: Scratch,
  _data: tempfile::TempDir,
  standing: StandingUnits,
  session: Arc<StandInSession>,
  runner: Runner,
  chat: Arc<StandInChat>,
}

impl Round {
  async fn new(mut session: StandInSession, unified: bool) -> Self {
    let scratch = Scratch::new().await;

    no_stages(scratch.db()).await;
    let data = tempfile::tempdir().unwrap();
    let chat = Arc::new(StandInChat::new("road-a").owners(OWNER).holding(&[OTHER]));

    documents(data.path());

    let standing = woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat.clone()], deployment()));
    // The rooms stand on the console's room list, so the receiver listens to them (docs/wake.md).
    bmo_receiver::receiver::rows::allow_chat(scratch.db(), OTHER).await.unwrap();
    let receiver = standing.receiver.clone();
    let runner = Runner::new(receiver.clone(), Arc::new(Around::new(receiver.clone())), timings(), AGENT);

    *standing.gate.entries.lock().unwrap() = vec![piece()];

    let mut agent = registration(AGENT, "Agent A", vec![ADDRESS], None);

    if unified {
      // A session the agency has not opened yet, so the receiver makes no replacement of its own: the one session
      // across rooms stands its ground across a room switch.
      agent.row.insert("sessionScope".to_owned(), json!("agent"));
      session = session.at_generation(0);
    }

    let session = Arc::new(session);

    standing.agency.registering(vec![agent]);
    standing.agency.carrying_on(session.clone());
    receiver.hold_runner(Arc::new(runner.clone()));

    Round {
      scratch,
      _data: data,
      standing,
      session,
      runner,
      chat,
    }
  }

  /// The owner's words, and the round they open run to its end.
  async fn round(&self, message_id: i64) {
    self.round_in(OWNER, owners_room(), message_id, "the words").await;
  }

  async fn round_in(&self, chat_id: i64, room: Room, message_id: i64, text: &str) {
    let before = self.session.prompts().len();

    self
      .standing
      .receiver
      .message(&said(chat_id, message_id, text), &Context::default(), room)
      .await
      .unwrap();
    until("the round's end", || {
      self.session.prompts().len() > before && self.runner.idle() && !self.session.working()
    })
    .await;
    // The round's log writes land after its turn ends.
    tokio::time::sleep(Duration::from_millis(100)).await;
  }

  /// The agency opening a generation under the session: its state names it, and its own record of the session's
  /// loads goes as the generation opens.
  fn opens_generation(&self, generation: i64) {
    self.session.now_at(Some(generation));
    self.standing.agency.loads.lock().unwrap().remove(&self.session.id);
  }

  /// The owner's words on a session whose turn the agency hands over as it works: the agency says a generation
  /// opened under it, its loads gone with the generation, and the turn runs on.
  async fn round_handed_over(&self, message_id: i64) {
    let before = self.session.prompts().len();

    self
      .standing
      .receiver
      .message(&said(OWNER, message_id, "the words"), &Context::default(), owners_room())
      .await
      .unwrap();
    until("the round's turn", || self.session.working()).await;
    self.standing.agency.loads.lock().unwrap().remove(&self.session.id);
    self
      .session
      .told("system-prompt-changed", json!({ "session": self.session.id, "run": "run-1" }));
    self.session.let_go();
    until("the round's end", || {
      self.session.prompts().len() > before && self.runner.idle() && !self.session.working()
    })
    .await;
    tokio::time::sleep(Duration::from_millis(100)).await;
  }

  /// The read marks the round logs build, the end row of each round in order.
  async fn reads(&self) -> Vec<Value> {
    self
      .scratch
      .texts(r#"SELECT "reads" FROM "instruction_rounds" WHERE "kind" = 'end' ORDER BY "id""#)
      .await
      .into_iter()
      .map(|row| serde_json::from_str(&row).unwrap())
      .collect()
  }

  fn folds(&self) -> Vec<(i64, String)> {
    self.standing.brain.folds()
  }
}

fn how_of(reads: &Value, path: &str) -> Option<String> {
  reads
    .as_array()?
    .iter()
    .find(|read| read["path"] == json!(path))
    .and_then(|read| read["how"].as_str())
    .map(str::to_owned)
}

#[test]
fn a_generation_is_changed_only_where_it_is_past_the_one_answered_on_or_the_session_is_gone() {
  assert!(
    !generation::changed(None, Standing::At(3)),
    "a session no turn answered on opens nothing"
  );
  assert!(!generation::changed(Some(0), Standing::At(3)), "a first generation opens nothing");
  assert!(generation::changed(Some(3), Standing::At(4)), "a higher generation opens one");
  assert!(!generation::changed(Some(3), Standing::At(3)), "the same generation is no change");
  assert!(generation::changed(Some(3), Standing::At(0)), "a session gone opens one");
  assert!(generation::changed(Some(3), Standing::Gone), "a harness gone opens one");
  assert!(!generation::changed(Some(3), Standing::Unread), "a state not read opens nothing");
  assert!(
    !generation::changed(Some(0), Standing::Gone),
    "a session already released opens nothing twice"
  );
}

#[tokio::test]
async fn a_generation_the_agency_opens_releases_the_session_once_and_the_round_log_starts_from_zero() {
  let round = Round::new(StandInSession::new(SESSION, ANSWER).at_generation(1), false).await;

  round.round(1).await;
  assert!(round.folds().is_empty(), "the first generation releases nothing: {:?}", round.folds());
  assert_eq!(how_of(&round.reads().await[0], PIECE).as_deref(), Some("attached"));

  // The agency opens a generation under the session as it hands it over, releasing its loads there.
  round.opens_generation(2);
  round.round(2).await;

  assert_eq!(
    round.folds(),
    [(BRAIN, SESSION.to_owned())],
    "the ended generation's handouts clear, once"
  );
  assert_eq!(
    how_of(&round.reads().await[1], PIECE).as_deref(),
    Some("attached"),
    "the first turn of the new generation holds nothing"
  );
  assert!(
    round.standing.agency.loads_forgotten().is_empty(),
    "the agency releases the loads as the generation opens; the receiver does not delete them again: {:?}",
    round.standing.agency.loads_forgotten()
  );

  // The generation is answered on from then on: a turn on it releases nothing, and what it holds stands again.
  round.round(3).await;

  assert_eq!(round.folds(), [(BRAIN, SESSION.to_owned())], "released once and only then");
  assert_eq!(
    how_of(&round.reads().await[2], PIECE).as_deref(),
    Some("held"),
    "the same generation holds what it read"
  );
}

#[tokio::test]
async fn a_session_resumed_on_its_generation_releases_nothing() {
  let round = Round::new(StandInSession::new(SESSION, ANSWER).at_generation(1), false).await;

  round.round(1).await;
  round.round(2).await;

  assert!(round.folds().is_empty(), "a plain turn releases nothing: {:?}", round.folds());
  assert_eq!(
    how_of(&round.reads().await[1], PIECE).as_deref(),
    Some("held"),
    "the resumed session holds what it read"
  );
}

#[tokio::test]
async fn a_return_to_another_room_of_the_one_session_releases_nothing() {
  let round = Round::new(StandInSession::new(UNIFIED_SESSION, ANSWER), true).await;

  round.round(1).await;
  round.round_in(OTHER, a_group(), 2, "agent-a, what is new in this room").await;

  assert!(
    round.folds().is_empty(),
    "a return to another room releases nothing: {:?}",
    round.folds()
  );
  assert!(
    round
      .standing
      .agency
      .loaded()
      .get(UNIFIED_SESSION)
      .is_some_and(|loads| loads.iter().any(|load| load.path.starts_with("room-context/"))),
    "the one session keeps what it holds across rooms"
  );
}

#[tokio::test]
async fn a_generation_the_agency_announces_for_the_one_session_across_rooms_is_released() {
  let round = Round::new(StandInSession::new(UNIFIED_SESSION, ANSWER).holding_the_first(), true).await;

  round.round_handed_over(1).await;
  assert!(round.folds().is_empty(), "the turn the handover opened releases nothing itself");

  // The one session across rooms names no higher generation, so the agency's word carries the handover to the
  // next turn, which releases it once.
  round.round(2).await;

  assert_eq!(
    round.folds(),
    [(BRAIN, UNIFIED_SESSION.to_owned())],
    "the announced generation's handouts clear"
  );
  assert_eq!(
    how_of(&round.reads().await[1], PIECE).as_deref(),
    Some("attached"),
    "the turn after the handover holds nothing"
  );

  round.round(3).await;
  assert_eq!(
    round.folds(),
    [(BRAIN, UNIFIED_SESSION.to_owned())],
    "the announced generation is released once"
  );
}

#[tokio::test]
async fn the_release_forgets_the_receivers_records_and_reports_the_fold() {
  use bmo_receiver::chat::cache::{active_route, state_key};
  use bmo_receiver::store::settings;

  let round = Round::new(StandInSession::new(SESSION, ANSWER).at_generation(1), false).await;

  round.round(1).await;
  round
    .scratch
    .run(
      r#"INSERT INTO "bmo_session_notes" ("session_id", "chat_id", "fingerprint") VALUES ($1, $2, 'v1')"#,
      vec![SESSION.into(), OTHER.into()],
    )
    .await;
  settings::set(round.scratch.db(), &state_key(AGENT, SESSION), "a cache record")
    .await
    .unwrap();

  assert_eq!(
    round
      .scratch
      .count(
        r#"SELECT count(*) FROM "bmo_room_contexts" WHERE "session_id" = $1"#,
        vec![SESSION.into()]
      )
      .await,
    1
  );

  // The agency released the session's loads as it opened the generation; the receiver does not delete an
  // already-empty record again.
  round.standing.agency.loads.lock().unwrap().remove(SESSION);

  generation::release(&round.standing.receiver, AGENT, SESSION).await;

  assert_eq!(
    round
      .scratch
      .count(
        r#"SELECT count(*) FROM "bmo_room_contexts" WHERE "session_id" = $1"#,
        vec![SESSION.into()]
      )
      .await,
    0,
    "the room block it read is gone"
  );
  assert_eq!(
    round
      .scratch
      .count(
        r#"SELECT count(*) FROM "bmo_session_notes" WHERE "session_id" = $1"#,
        vec![SESSION.into()]
      )
      .await,
    0,
    "the note versions it held are gone"
  );
  assert_eq!(
    settings::get(round.scratch.db(), &state_key(AGENT, SESSION)).await.unwrap(),
    None,
    "the cache record it was opened against is gone"
  );
  assert!(
    round.standing.agency.loads_forgotten().is_empty(),
    "the receiver does not delete loads the agency already released: {:?}",
    round.standing.agency.loads_forgotten()
  );
  assert_eq!(round.folds(), [(BRAIN, SESSION.to_owned())], "the fold is reported to the brain");

  // The road it stood on is the receiver's own record and is no part of a generation.
  assert!(
    settings::get(round.scratch.db(), &bmo_receiver::chat::cache::active_key(AGENT))
      .await
      .unwrap()
      .is_some()
  );
  active_route(round.scratch.db(), AGENT)
    .await
    .unwrap()
    .expect("the road outlives the session");
}

#[tokio::test]
async fn a_handoff_kept_ahead_of_a_fresh_generation_rides_its_first_turn() {
  const NOTE: &str = "what the session before the fresh generation was in the middle of";

  let round = Round::new(StandInSession::new(SESSION, ANSWER).at_generation(1), false).await;

  round.round(1).await;

  // The agency asks for the handoff as it hands the room's session over, then opens the fresh generation the
  // receiver's next turn goes on.
  let kept = round._data.path().join("kept-handoff.md");

  std::fs::write(&kept, NOTE).unwrap();
  settings::set(round.scratch.db(), &pending_key(AGENT), &kept.to_string_lossy())
    .await
    .unwrap();
  round.opens_generation(2);
  round.round(2).await;

  let prompt = &round.session.prompts()[1];

  assert!(prompt.starts_with(&HANDOVER.chat.handed_marker), "{prompt}");
  assert!(prompt.contains(NOTE), "the handoff is not carried: {prompt}");
  assert_eq!(
    settings::get(round.scratch.db(), &pending_key(AGENT)).await.unwrap(),
    None,
    "the handoff is taken once"
  );
}

// The `session-handed-over` lines a room's record keeps, in the order kept, each as the line it was said from.
async fn handed_lines(round: &Round, chat_id: i64) -> Vec<Value> {
  round
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

#[tokio::test]
async fn a_generation_the_agency_opens_under_a_rooms_session_tells_the_room_nothing_of_itself() {
  let round = Round::new(StandInSession::new(SESSION, ANSWER).at_generation(1), false).await;

  round.round(1).await;

  let sent = round.chat.sent().len();

  // The agency hands the room's session over and opens the next generation; it tells that handover itself, so the
  // turn that finds the generation releases what the ended one held and tells nothing.
  round.opens_generation(2);
  round.round(2).await;
  round.opens_generation(3);
  round.round(3).await;

  assert!(handed_lines(&round, OWNER).await.is_empty(), "no line is told of a generation alone");

  let carried: Vec<String> = round.chat.sent()[sent..].iter().map(|sent| format!("{sent:?}")).collect();

  assert!(
    carried.iter().all(|sent| !sent.contains("session-handed-over")),
    "nothing of a handover went to the chat service: {carried:?}"
  );
}

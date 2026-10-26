//! A round's opening and what a turn carries of the brain (docs/wake.md, 「The room's queue and the round」;
//! docs/bmo-window.md, 「What every turn carries」): words the open window alone let in are judged by the agent
//! first, a skip there sending nothing and asking no gate, a named call goes straight to the round, and the turn
//! carries the brain's index — unread counts by group, the session's reads counted out — and no piece's body.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::conversation::Timings;
use bmo_receiver::chat::runner::Runner;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::js::bun_hash;
use bmo_receiver::receiver::{Inbound, Room};
use receiver_policy::chat_line::CHAT_LINE;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_ports::agency::Load;
use receiver_ports::gate::{Entry, Source};

use crate::common::Scratch;
use crate::common::session::StandInSession;
use crate::common::units::{ADDRESS, AGENT, StandInChat, Standing, deployment, registration, unrun, woken};

const SESSION: &str = "agent-a@room";
const ROOM: i64 = -500;
const IDENTITY: i64 = 7;

fn group() -> Room {
  Room {
    owner_room: false,
    identity: Some(IDENTITY),
    private_to: None,
  }
}

fn said(message_id: i64, text: &str) -> Inbound {
  Inbound {
    chat_id: ROOM,
    message_id,
    sender_id: Some(31),
    sender_name: "member".into(),
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

struct Group {
  scratch: Scratch,
  _data: tempfile::TempDir,
  standing: Standing,
  session: Arc<StandInSession>,
  runner: Runner,
}

impl Group {
  async fn new(session: StandInSession) -> Self {
    let scratch = Scratch::new().await;

    crate::common::session::no_drafting(scratch.db()).await;

    let data = tempfile::tempdir().unwrap();
    let chat = Arc::new(StandInChat::new("road-a").agent_in(ROOM, IDENTITY));

    documents(data.path());

    let standing = woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat], deployment()));
    let session = Arc::new(session);
    let receiver = standing.receiver.clone();
    let timings = Timings {
      queue: Duration::from_millis(1),
      wait: Duration::from_secs(60),
      window: Some(Duration::from_secs(60)),
      look: Duration::from_millis(5),
    };
    let runner = Runner::new(receiver.clone(), Arc::new(Around::new(receiver.clone())), timings, AGENT);

    standing
      .agency
      .registering(vec![registration(AGENT, "Agent A", vec![ADDRESS], None, true)]);
    standing.agency.carrying_on(session.clone());
    receiver.hold_runner(Arc::new(runner.clone()));

    Group {
      scratch,
      _data: data,
      standing,
      session,
      runner,
    }
  }

  // Words handed to the agent's conversations as the wake hands them: a named call opening the window, or words
  // the open window alone let in.
  async fn hand(&self, message_id: i64, text: &str, named: bool) {
    self.runner.of(AGENT).enqueue(said(message_id, text), group(), named, !named).await;
  }

  async fn rounds_over(&self, turns: usize) {
    until("the round's end", || {
      self.session.prompts().len() >= turns && self.runner.idle() && !self.session.working()
    })
    .await;
    tokio::time::sleep(Duration::from_millis(100)).await;
  }
}

#[tokio::test]
async fn words_the_window_alone_let_in_are_judged_first_and_a_skip_sends_nothing_and_asks_no_gate() {
  let round =
    Group::new(StandInSession::new(SESSION, "unused").answering(&["<<SKIP>>", "never asked"])).await;

  round.hand(1, "did anyone see the match last night", false).await;
  round.rounds_over(1).await;

  let prompts = round.session.prompts();

  assert_eq!(prompts.len(), 1, "the judgement is the round's only turn: {prompts:#?}");
  assert!(prompts[0].contains(&CHAT_ROUND.supervisor.question), "{}", prompts[0]);
  assert!(prompts[0].contains("did anyone see the match last night"));
  assert!(!prompts[0].contains("TURN SCOPE") && !prompts[0].contains(&CHAT_ROUND.index.head));
  assert_eq!(*round.standing.gate.referred.lock().unwrap(), 0, "no reference was asked");
  assert!(round.standing.gate.read.lock().unwrap().is_empty(), "nothing passed the sanitiser");

  let log = round
    .scratch
    .texts(r#"SELECT "kind" || ' ' || "number" || ' ' || "outcome" FROM "draft_rounds" ORDER BY "id""#)
    .await;

  assert_eq!(log, ["supervisor 0 skip"]);
}

#[tokio::test]
async fn an_answer_to_the_judgement_opens_the_ordinary_round_and_a_named_call_skips_the_judgement() {
  let round =
    Group::new(StandInSession::new(SESSION, "the round's answer").answering(&["They asked me.", "Here."]))
      .await;

  round.hand(1, "agent-a what time is it", true).await;
  round.rounds_over(1).await;

  let prompts = round.session.prompts();

  assert!(
    prompts[0].contains("TURN SCOPE"),
    "a named call opens the round at once: {}",
    prompts[0]
  );
  assert!(!prompts[0].contains(&CHAT_ROUND.supervisor.question));
  assert_eq!(*round.standing.gate.referred.lock().unwrap(), 1);

  round.hand(2, "and what about tomorrow then", false).await;
  round.rounds_over(3).await;

  let prompts = round.session.prompts();

  assert!(prompts[1].contains(&CHAT_ROUND.supervisor.question), "{}", prompts[1]);
  assert!(
    prompts[2].contains("TURN SCOPE"),
    "the judgement answered, so the round went on: {}",
    prompts[2]
  );
  assert_eq!(*round.standing.gate.referred.lock().unwrap(), 2);
}

#[tokio::test]
async fn the_turn_carries_the_gates_core_whole_beside_the_brains_index() {
  let round = Group::new(StandInSession::new(SESSION, "Here.")).await;
  let entry = |path: &str, text: &str, source: Source| Entry {
    path: path.to_owned(),
    text: text.to_owned(),
    source,
    fragment: None,
  };

  // What attaches without a pick: a speaker's sheet head, the room's due memories and a row an event woke.
  *round.standing.gate.entries.lock().unwrap() = vec![
    entry("rooms/-500/people/31/sheet.md", "BODY OF A PERSON", Source::Person),
    entry("memories/-500", "BODY OF THE DUE MEMORIES", Source::Memory),
    entry("situations/7", "BODY OF A WOKEN ROW", Source::Situation),
  ];
  // The session already read the person's sheet head at this version.
  round.standing.agency.loads.lock().unwrap().insert(
    SESSION.to_owned(),
    vec![Load {
      path: "rooms/-500/people/31/sheet.md".to_owned(),
      version: bun_hash("BODY OF A PERSON").to_string(),
    }],
  );

  round.hand(1, "agent-a what is due today", true).await;
  round.rounds_over(1).await;

  let prompt = &round.session.prompts()[0];
  let index = &CHAT_ROUND.index;

  assert!(prompt.contains("Reference for this message (3):"), "{prompt}");

  for body in ["BODY OF THE DUE MEMORIES", "BODY OF A WOKEN ROW"] {
    assert!(prompt.contains(body), "the core goes in whole: {prompt}");
  }

  assert!(
    !prompt.contains("BODY OF A PERSON")
      && prompt.contains("[rooms/-500/people/31/sheet.md] (already loaded)"),
    "an entry the session holds at its version goes as a mark: {prompt}"
  );
  assert!(prompt.contains(&format!("- {}: 1 not read", index.room_memories)), "{prompt}");
  assert!(prompt.contains(&format!("- {}: 0 not read", index.knowledge)), "{prompt}");
  assert!(prompt.contains(&index.hint));
  assert!(
    prompt.find("agent-a what is due today") < prompt.find(&index.head),
    "the index stands after the room's words"
  );
}

#[tokio::test]
async fn the_turns_index_is_the_gates_with_its_hint_and_the_sessions_reads_counted_out_there() {
  use receiver_ports::gate::IndexGroup;

  let round = Group::new(StandInSession::new(SESSION, "Here.")).await;
  let group = |group: &str, unread: u64, hint: Option<&str>| IndexGroup {
    group: group.to_owned(),
    unread,
    hint: hint.map(str::to_owned),
  };

  round.standing.gate.index.lock().unwrap().insert(
    ROOM,
    vec![
      group("system-documents", 0, None),
      group("room-rules", 4, Some("2 entries changed in the last three days.")),
      group("knowledge", 7, None),
      group("people", 1, None),
      group("room-memories", 3, None),
    ],
  );
  round.hand(1, "agent-a what do the rules say", true).await;
  round.rounds_over(1).await;

  let prompt = &round.session.prompts()[0];
  let index = &CHAT_ROUND.index;

  assert!(
    prompt.contains(&format!(
      "- {}: 4 not read; lately: 2 entries changed in the last three days.",
      index.room_rules
    )),
    "{prompt}"
  );
  assert!(prompt.contains(&format!("- {}: 7 not read", index.knowledge)), "{prompt}");
  assert!(prompt.contains(&index.hint), "the nudge stands in the turn");

  let asked = round.standing.gate.indexed.lock().unwrap().clone();

  assert_eq!(
    asked[0],
    (ROOM, Some(SESSION.to_owned()), Some(AGENT.to_owned())),
    "the index is the session's"
  );
}

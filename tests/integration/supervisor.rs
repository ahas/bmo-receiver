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
  chat: Arc<StandInChat>,
  session: Arc<StandInSession>,
  runner: Runner,
}

impl Group {
  async fn new(session: StandInSession) -> Self {
    let scratch = Scratch::new().await;

    crate::common::session::no_drafting(scratch.db()).await;

    let data = tempfile::tempdir().unwrap();
    let chat = Arc::new(StandInChat::new("road-a").holding(&[ROOM]).agent_in(ROOM, IDENTITY));

    documents(data.path());

    let mut standing =
      woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat.clone()], deployment()));

    standing.receiver = standing.receiver.with_situation_updates(true);

    let session = Arc::new(session.showing_situations());
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
      chat,
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

  // The turns up to `least` asked, without waiting for the round that asks them to end: the prompt a test reads
  // before the message is written.
  async fn prompts_over(&self, least: usize) {
    until("the round's prompts", || self.session.prompts().len() >= least).await;
  }
}

#[tokio::test]
async fn the_judgement_opens_on_the_catalogues_supervisor_text_which_no_list_may_name() {
  use bmo_receiver::chat::draft::table::{self, SUPERVISOR};

  let round = Group::new(StandInSession::new(SESSION, "unused").answering(&[
    "We talked about the match.",
    "<<SKIP>>",
    "never asked",
  ]))
  .await;
  let catalogue = table::questions(round.scratch.db()).await.unwrap();

  assert_eq!(catalogue[0].name, SUPERVISOR);
  assert_eq!(
    catalogue[0].text,
    table::supervisor_default(),
    "the policy's head and question where none is set"
  );
  assert!(!catalogue[0].set);

  table::set_question(round.scratch.db(), SUPERVISOR, "Decide first, briefly.")
    .await
    .unwrap();
  round.hand(1, "did anyone see the match last night", false).await;
  round.rounds_over(2).await;

  let prompts = round.session.prompts();

  assert!(
    prompts[0].contains(&CHAT_ROUND.situation.previous),
    "the update turn reads the note as it stands: {}",
    prompts[0]
  );

  let opening = prompts[1].find("Decide first, briefly.\n");

  assert!(
    opening.is_some() && opening < prompts[1].find(&CHAT_ROUND.supervisor.recent),
    "the judging turn opens on the set text: {}",
    prompts[1]
  );
  assert!(!prompts[1].contains(&CHAT_ROUND.supervisor.question));
  assert!(
    prompts[1].contains(&CHAT_ROUND.supervisor.situation)
      && prompts[1].contains("We talked about the match."),
    "the judgement reads the note: {}",
    prompts[1]
  );

  let catalogue = table::questions(round.scratch.db()).await.unwrap();

  assert_eq!(
    (catalogue[0].name.as_str(), catalogue[0].set),
    (SUPERVISOR, true),
    "the console's text stands at its place"
  );
  assert_eq!(catalogue.iter().filter(|question| question.name == SUPERVISOR).count(), 1);

  let process = table::Process {
    provider: "*".into(),
    tier: "*".into(),
    agent: "*".into(),
    rounds: vec![SUPERVISOR.into(), "final".into()],
    send_from: String::new(),
    light: vec![],
    light_chars: 0,
    ceiling_seconds: 60,
    past_ceiling: table::PastCeiling::Send,
    set: true,
  };

  assert!(table::refusal(&process, &catalogue).is_some_and(|why| why.contains(SUPERVISOR)));
}

#[tokio::test]
async fn words_the_window_alone_let_in_are_judged_first_and_a_skip_sends_nothing_and_asks_no_gate() {
  let round = Group::new(StandInSession::new(SESSION, "unused").answering(&[
    "They talked about the match.",
    "<<SKIP>>",
    "never asked",
  ]))
  .await;

  round.hand(1, "did anyone see the match last night", false).await;
  round.rounds_over(2).await;

  let prompts = round.session.prompts();

  assert_eq!(prompts.len(), 2, "the update and the judgement are the round's turns: {prompts:#?}");
  assert!(
    prompts[0].contains(&CHAT_ROUND.situation.previous)
      && !prompts[0].contains(&CHAT_ROUND.supervisor.question),
    "the first turn updates the note: {}",
    prompts[0]
  );
  assert!(prompts[1].contains(&CHAT_ROUND.supervisor.question), "{}", prompts[1]);
  assert!(prompts[1].contains("did anyone see the match last night"));
  assert!(prompts[1].contains("They talked about the match."));
  assert!(!prompts[1].contains("TURN SCOPE") && !prompts[1].contains(&CHAT_ROUND.index.head));
  assert_eq!(*round.standing.gate.referred.lock().unwrap(), 0, "no reference was asked");
  assert_eq!(
    round
      .scratch
      .texts(r#"SELECT "text" FROM "room_situations" WHERE "chat_id" = -500"#)
      .await,
    ["They talked about the match."]
  );

  let log = round
    .scratch
    .texts(r#"SELECT "kind" || ' ' || "number" || ' ' || "outcome" FROM "draft_rounds" ORDER BY "id""#)
    .await;

  assert_eq!(log, ["supervisor 0 skip"]);
}

#[tokio::test]
async fn a_line_that_resumes_an_earlier_thread_with_the_agent_is_answered_with_its_note() {
  let round = Group::new(StandInSession::new(SESSION, "Here.").answering(&[
    "They asked you to send the file; you promised it, and now ask when it will arrive.",
    "They asked me again.",
    "Here.",
  ]))
  .await;

  round
    .scratch
    .run(
      r#"INSERT INTO "room_situations" ("chat_id", "text", "updated_at") VALUES ($1, $2, $3)"#,
      vec![ROOM.into(), "They asked you to send the file; you promised it.".into(), 1i64.into()],
    )
    .await;
  round
    .scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via")
         VALUES ($1, $2, 31, 'member', 1760000000000, 'agent-a can you send the file?', 'road-a')"#,
      vec![ROOM.into(), 1i64.into()],
    )
    .await;

  round.hand(2, "and when will it arrive?", false).await;
  round.prompts_over(3).await;

  let prompts = round.session.prompts();

  assert!(
    prompts[0].contains("They asked you to send the file; you promised it."),
    "the update turn reads the earlier thread's note: {}",
    prompts[0]
  );
  assert!(prompts[1].contains(&CHAT_ROUND.supervisor.question), "{}", prompts[1]);
  assert!(prompts[1].contains("now ask when it will arrive"), "{}", prompts[1]);
  assert!(prompts[2].contains("TURN SCOPE"), "{}", prompts[2]);
  assert!(
    prompts[2].contains("now ask when it will arrive"),
    "the round's draft carries the note: {}",
    prompts[2]
  );
}

#[tokio::test]
async fn the_turn_carries_the_gates_core_whole_beside_the_brains_index() {
  let round = Group::new(StandInSession::new(SESSION, "Here.")).await;
  let entry = |path: &str, text: &str, source: Source| Entry {
    path: path.to_owned(),
    text: text.to_owned(),
    source,
    fragment: None,
    holds: None,
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
  round.rounds_over(2).await;

  let prompts = round.session.prompts();
  let prompt = &prompts[1];
  let index = &CHAT_ROUND.index;

  assert!(prompt.contains("Reference for this message (3):"), "{prompt}");
  assert!(
    prompt.starts_with(&format!("{}\nHere.", CHAT_ROUND.situation.front)),
    "the note stands first, as the answer's instruction: {prompt}"
  );
  assert!(
    prompts[0].contains(&receiver_policy::chat_line::CHAT_LINE.room.voice),
    "the situation update carries the room block: {}",
    prompts[0]
  );
  assert!(
    !prompt.contains(&CHAT_ROUND.supervisor.question),
    "a named call skips the judgement: {prompt}"
  );

  for body in ["BODY OF THE DUE MEMORIES", "BODY OF A WOKEN ROW"] {
    assert!(prompt.contains(body), "the core goes in whole: {prompt}");
  }

  assert!(
    !prompt.contains("BODY OF A PERSON")
      && prompt.contains("[rooms/-500/people/31/sheet.md] (already loaded)"),
    "an entry the session holds at its version goes as a mark: {prompt}"
  );
  assert!(prompt.contains(&format!("{} 1", index.room_memories)), "{prompt}");
  assert!(prompt.contains(&format!("{} 0", index.knowledge)), "{prompt}");
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
    entries: vec![],
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
  round.rounds_over(2).await;

  let prompt = &round.session.prompts()[1];
  let index = &CHAT_ROUND.index;

  assert!(
    prompt.contains(&format!("{} 4 (2 entries changed in the last three days)", index.room_rules)),
    "{prompt}"
  );
  assert!(prompt.contains(&format!("{} 7", index.knowledge)), "{prompt}");
  assert!(prompt.contains(&index.hint), "the nudge stands in the turn");

  let asked = round.standing.gate.indexed.lock().unwrap().clone();

  assert_eq!(
    asked[0],
    (ROOM, Some(SESSION.to_owned()), Some(AGENT.to_owned())),
    "the index is the session's"
  );
}

#[tokio::test]
async fn a_line_that_asks_the_agent_is_answered_with_the_situation_note() {
  let round = Group::new(StandInSession::new(SESSION, "Here.").answering(&[
    "They asked you to send the file.",
    "They asked me.",
    "Here.",
    "Here.",
  ]))
  .await;

  // The line as the room's record holds it, which is where a judged round keeps the note it read under.
  round.scratch.message(ROOM, 1, 31).await;
  round.hand(1, "can you send the file?", false).await;
  round.prompts_over(3).await;

  let prompts = round.session.prompts();

  assert!(prompts.len() >= 3, "{prompts:#?}");
  assert!(prompts[0].contains(&CHAT_ROUND.situation.previous), "{}", prompts[0]);
  assert!(prompts[1].contains(&CHAT_ROUND.supervisor.situation), "{}", prompts[1]);
  assert!(prompts[1].contains("They asked you to send the file."), "{}", prompts[1]);
  assert!(prompts[2].contains("TURN SCOPE"), "{}", prompts[2]);
  assert!(
    prompts[2].contains("They asked you to send the file."),
    "the round's draft carries the note: {}",
    prompts[2]
  );
  assert_eq!(
    round
      .scratch
      .texts(r#"SELECT "text" FROM "room_situations" WHERE "chat_id" = -500"#)
      .await,
    ["They asked you to send the file."]
  );
  // The note stands on the line the round judged, marked as the round's own change, so the console's judgement
  // record shows the situation the round read it under.
  assert_eq!(
    round
      .scratch
      .texts(r#"SELECT "situation" FROM "messages" WHERE "chat_id" = -500 AND "message_id" = 1"#)
      .await,
    ["They asked you to send the file."]
  );
  assert_eq!(
    round
      .scratch
      .count(
        r#"SELECT COUNT(*) FROM "messages" WHERE "chat_id" = -500 AND "message_id" = 1 AND "situation_changed""#,
        vec![],
      )
      .await,
    1,
    "the round wrote the note afresh"
  );
}

#[tokio::test]
async fn a_line_between_two_people_is_left_alone_though_the_note_names_them() {
  let round = Group::new(
    StandInSession::new(SESSION, "unused")
      .answering(&["Mina and Joon are talking about the match; nobody waits on you.", "<<SKIP>>"]),
  )
  .await;

  round.hand(1, "Mina, did Joon see the match?", false).await;
  round.rounds_over(2).await;

  let prompts = round.session.prompts();

  assert_eq!(prompts.len(), 2, "{prompts:#?}");
  assert!(
    prompts[1].contains("Mina and Joon are talking about the match"),
    "the judgement reads the note: {}",
    prompts[1]
  );
  assert_eq!(
    round.scratch.count(r#"SELECT COUNT(*) FROM "messages""#, vec![]).await,
    0,
    "nothing was sent"
  );
  assert_eq!(
    round
      .scratch
      .texts(r#"SELECT "kind" || ' ' || "outcome" FROM "draft_rounds" ORDER BY "id""#)
      .await,
    ["supervisor skip"]
  );
}

#[tokio::test]
async fn the_note_stays_under_its_ceiling_and_the_update_is_one_light_turn() {
  let ceiling = CHAT_ROUND.situation.ceiling_chars;
  let long = "x".repeat(ceiling + 100);
  let round =
    Group::new(StandInSession::new(SESSION, "unused").answering(&[long.as_str(), "<<SKIP>>"])).await;

  round.hand(1, "did anyone see the match last night", false).await;
  round.rounds_over(2).await;

  let note = round
    .scratch
    .texts(r#"SELECT "text" FROM "room_situations" WHERE "chat_id" = -500"#)
    .await;

  assert_eq!(note.len(), 1);
  assert!(
    bmo_receiver::js::units(&note[0]) <= ceiling,
    "the note is {} units, over the ceiling of {ceiling}",
    bmo_receiver::js::units(&note[0])
  );
  assert!(note[0].ends_with(&CHAT_ROUND.situation.cut));

  let prompts = round.session.prompts();

  assert!(
    prompts[0].contains(&ceiling.to_string()),
    "the update turn names the ceiling: {}",
    prompts[0]
  );
  println!(
    "situation: update prompt {} chars, judgement prompt {} chars, note {} chars, extra {} chars",
    prompts[0].len(),
    prompts[1].len(),
    note[0].len(),
    prompts[0].len() + note[0].len()
  );
}

#[tokio::test]
async fn a_reply_mark_in_the_supervisors_answer_pins_the_older_line_it_names() {
  let round = Group::new(StandInSession::new(SESSION, "unused").answering(&[
    "They asked me.",
    "<<REPLY #1>> They asked me.",
    "Here.",
  ]))
  .await;

  round
    .scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via")
         VALUES ($1, 1, 31, 'member', 1760000000, 'a voice note', 'road-a')"#,
      vec![ROOM.into()],
    )
    .await;

  round.hand(2, "did you hear the recording?", false).await;
  round.rounds_over(3).await;

  let sent = round.chat.sent();

  assert_eq!(sent.len(), 1, "{sent:?}");
  let pinned = serde_json::to_string(&sent[0])
    .ok()
    .and_then(|line| serde_json::from_str::<serde_json::Value>(&line).ok())
    .and_then(|line| line.get("replyToMessageId").and_then(serde_json::Value::as_i64));

  assert_eq!(pinned, Some(1), "the supervisor's mark pins the older line: {sent:?}");

  assert_eq!(
    round
      .scratch
      .texts(
        r#"SELECT coalesce("reply_to_message_id"::text, 'none') FROM "messages" WHERE "message_id" = -1"#
      )
      .await,
    ["1"],
    "the record names the older message"
  );
}

#[tokio::test]
async fn the_supervisor_prompt_is_the_golden_file() {
  let round = Group::new(StandInSession::new(SESSION, "Here.")).await;

  round.hand(1, "are you there", false).await;
  round.prompts_over(2).await;

  let prompt = &round.session.prompts()[1];

  assert_eq!(prompt, include_str!("../fixtures/supervisor-prompt.txt"));
}

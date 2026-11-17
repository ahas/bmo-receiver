//! A round's opening and what a turn carries of the brain (docs/wake.md, 「The room's queue and the round」;
//! docs/bmo-window.md, 「What every turn carries」): words the open window alone let in are judged by the agent
//! first, a skip there sending nothing and asking no gate, and a named call goes straight to the round. The
//! brain's index — unread counts by group, the session's reads counted out — rides the room's block at session
//! opening, never the turn, which carries no piece's body.

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
use receiver_policy::drafting::DRAFTING;
use receiver_ports::agency::Load;
use receiver_ports::gate::{Entry, IndexGroup, Source};

use crate::integration::common::Scratch;
use crate::integration::common::session::StandInSession;
use crate::integration::common::units::{
  ADDRESS, AGENT, StandInChat, Standing, deployment, registration, unrun, woken,
};

const SESSION: &str = "agent-a@room";
const ROOM: i64 = -500;
const IDENTITY: i64 = 7;

fn group() -> Room {
  Room {
    owner_room: false,
    identity: Some(IDENTITY),
    private_to: None,
    sealed: false,
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

// A prompt's own turn: what follows the room's block and the brain's index of a session's opening, where the
// prompt carries them.
fn turn_of(prompt: &str) -> &str {
  let turn = prompt
    .rsplit_once(&CHAT_LINE.room.end)
    .map_or(prompt, |(_, turn)| turn)
    .trim_start_matches('\n');

  match turn.split_once("\n\n") {
    Some((index, rest))
      if index.starts_with(&CHAT_ROUND.index.head) || index.ends_with(CHAT_ROUND.index.hint.trim()) =>
    {
      rest
    }
    _ => turn,
  }
}

async fn until(what: &str, holds: impl Fn() -> bool) {
  // The suite runs the whole file's tests beside the machine's other work, so a round's two turns can be
  // starved for longer than a quiet run needs; the bound is long enough that slowness is not a failure.
  let waited = tokio::time::timeout(Duration::from_secs(90), async {
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

    crate::integration::common::session::no_drafting(scratch.db()).await;

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

    standing.agency.registering(vec![registration(AGENT, "Agent A", vec![ADDRESS], None)]);
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

  // The room's situation note as it stands before the round.
  async fn noted(&self, note: &str) {
    self
      .scratch
      .run(
        r#"INSERT INTO "room_situations" ("chat_id", "text", "updated_at") VALUES ($1, $2, 1)"#,
        vec![ROOM.into(), note.into()],
      )
      .await;
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

  let round = Group::new(
    StandInSession::new(SESSION, "unused").answering(&["<<SKIP: nobody here was asked>>", "never asked"]),
  )
  .await;
  let catalogue = table::questions(round.scratch.db(), Some(AGENT)).await.unwrap();

  assert_eq!(catalogue[0].name, SUPERVISOR);
  assert_eq!(
    catalogue[0].text,
    table::supervisor_default(),
    "the policy's head and question where none is set"
  );
  assert!(!catalogue[0].set);

  table::set_question(round.scratch.db(), AGENT, SUPERVISOR, "Decide first, briefly.")
    .await
    .unwrap();
  round.noted("We talked about the match.").await;
  round.hand(1, "did anyone see the match last night", false).await;
  round.rounds_over(1).await;

  let prompts = round.session.prompts();
  let judging = turn_of(&prompts[0]);

  assert!(
    !prompts.iter().any(|prompt| prompt.contains(&CHAT_ROUND.situation.previous)),
    "no turn of the round updates the note: {prompts:#?}"
  );
  assert!(
    prompts[0].contains("Decide first, briefly."),
    "the room's block carries the set supervisor text: {}",
    prompts[0]
  );
  assert!(
    judging.starts_with(&CHAT_ROUND.supervisor.marker) && !judging.contains("Decide first, briefly."),
    "the judging turn carries the marker, not the question: {judging}"
  );
  assert!(!judging.contains(&CHAT_ROUND.supervisor.question));
  assert!(
    judging.contains(&format!("{}\nWe talked about the match.", CHAT_ROUND.situation.front)),
    "the judgement reads the note the session has not read: {judging}"
  );

  let catalogue = table::questions(round.scratch.db(), Some(AGENT)).await.unwrap();

  assert_eq!(
    (catalogue[0].name.as_str(), catalogue[0].set),
    (SUPERVISOR, true),
    "the console's text stands at its place"
  );
  assert_eq!(catalogue.iter().filter(|question| question.name == SUPERVISOR).count(), 1);

  let process = table::Process {
    provider: "*".into(),
    tier: "*".into(),
    agent: AGENT.into(),
    rounds: vec![SUPERVISOR.into(), "final".into()],
    send_from: String::new(),
    off: vec![],
    stages_off: vec![],
    ceiling_seconds: 60,
    past_ceiling: table::PastCeiling::Send,
    set: true,
  };

  assert!(table::refusal(&process, &catalogue).is_some_and(|why| why.contains(SUPERVISOR)));
}

#[tokio::test]
async fn words_the_window_alone_let_in_are_judged_first_and_a_skip_sends_nothing_and_asks_no_gate() {
  let round = Group::new(
    StandInSession::new(SESSION, "unused").answering(&["<<SKIP: nobody here was asked>>", "never asked"]),
  )
  .await;

  round.hand(1, "did anyone see the match last night", false).await;
  round.rounds_over(1).await;

  let prompts = round.session.prompts();
  let judging = turn_of(&prompts[0]);

  assert_eq!(prompts.len(), 1, "the judgement is the round's one turn: {prompts:#?}");
  assert!(
    prompts[0].contains(&CHAT_ROUND.supervisor.question),
    "the room's block carries the judgement's question: {}",
    prompts[0]
  );
  assert!(judging.starts_with(&CHAT_ROUND.supervisor.marker), "{judging}");
  assert!(judging.contains("did anyone see the match last night"));
  assert!(
    !judging.contains(&CHAT_ROUND.situation.front),
    "a room with no note hands the judgement none: {judging}"
  );
  assert!(!judging.contains("TURN SCOPE") && !judging.contains(&CHAT_ROUND.index.head));
  assert_eq!(*round.standing.gate.referred.lock().unwrap(), 0, "no reference was asked");
  assert!(
    round
      .scratch
      .texts(r#"SELECT "text" FROM "room_situations" WHERE "chat_id" = -500"#)
      .await
      .is_empty(),
    "a round writes no note"
  );

  let log = round
    .scratch
    .texts(r#"SELECT "kind" || ' ' || "number" || ' ' || "outcome" FROM "draft_rounds" ORDER BY "id""#)
    .await;

  assert_eq!(log, ["supervisor 0 skip"]);
}

#[tokio::test]
async fn a_line_that_resumes_an_earlier_thread_with_the_agent_is_answered_with_its_note() {
  let round =
    Group::new(StandInSession::new(SESSION, "Here.").answering(&["They asked me again.", "Here."])).await;

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
  round.prompts_over(2).await;

  let prompts = round.session.prompts();
  let judging = turn_of(&prompts[0]);

  assert!(
    prompts[0].contains(&CHAT_ROUND.supervisor.question),
    "the room's block carries the judgement's question: {}",
    prompts[0]
  );
  assert!(judging.starts_with(&CHAT_ROUND.supervisor.marker), "{judging}");
  assert!(judging.contains("and when will it arrive?"), "{judging}");
  assert!(
    judging.contains(&format!(
      "{}\nThey asked you to send the file; you promised it.",
      CHAT_ROUND.situation.front
    )),
    "the judgement carries the earlier thread's note whole: {judging}"
  );
  assert!(prompts[1].contains("TURN SCOPE"), "{}", prompts[1]);
  assert!(
    !prompts[1].contains("They asked you to send the file; you promised it")
      && !prompts[1].contains(&CHAT_ROUND.situation.unchanged),
    "the session read the note in its judgement, and the draft turn carries none of it: {}",
    prompts[1]
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
  // The brain's index for the room and the session.
  round.standing.gate.index.lock().unwrap().insert(
    ROOM,
    vec![
      IndexGroup {
        group: "system-documents".to_owned(),
        unread: 0,
        hint: None,
        entries: vec![],
      },
      IndexGroup {
        group: "room-rules".to_owned(),
        unread: 0,
        hint: None,
        entries: vec![],
      },
      IndexGroup {
        group: "knowledge".to_owned(),
        unread: 0,
        hint: None,
        entries: vec![],
      },
      IndexGroup {
        group: "people".to_owned(),
        unread: 1,
        hint: None,
        entries: vec![],
      },
      IndexGroup {
        group: "room-memories".to_owned(),
        unread: 1,
        hint: None,
        entries: vec![],
      },
    ],
  );

  round.noted("Here.").await;
  round.hand(1, "agent-a what is due today", true).await;
  round.rounds_over(1).await;

  let prompts = round.session.prompts();
  let block = &prompts[0];
  let prompt = turn_of(&prompts[0]);
  let index = &CHAT_ROUND.index;

  assert!(prompt.contains("Reference for this message (3):"), "{prompt}");
  assert!(
    prompt.starts_with(&format!("{}\nHere.", CHAT_ROUND.situation.front)),
    "the note stands first, read once by the session: {prompt}"
  );
  assert!(
    prompts[0].contains(&receiver_policy::chat_line::CHAT_LINE.room.voice),
    "the turn carries the room block: {}",
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
  assert!(block.contains(&format!("{} 1", index.room_memories)), "{block}");
  assert!(block.contains(&format!("{} 0", index.knowledge)), "{block}");
  assert!(block.contains(&index.hint));
  assert!(
    !prompt.contains(&index.head) && !prompt.contains(&index.hint),
    "the turn itself carries no index line: {prompt}"
  );
}

#[tokio::test]
async fn a_note_read_once_rides_no_later_turn_and_a_memory_reference_rides_as_its_mark() {
  let round = Group::new(
    StandInSession::new(SESSION, "unused").answering(&["the first answer", "the second answer"]),
  )
  .await;

  round.noted("the room is quiet").await;

  *round.standing.gate.entries.lock().unwrap() = vec![Entry {
    path: "memories/-500".to_owned(),
    text: "BODY OF THE DUE MEMORIES".to_owned(),
    source: Source::Memory,
    fragment: None,
    holds: None,
  }];

  round.hand(1, "first words", true).await;
  round.rounds_over(1).await;
  round.hand(2, "second words", true).await;
  round.rounds_over(2).await;

  let prompts = round.session.prompts();

  assert_eq!(prompts.len(), 2, "{prompts:#?}");
  assert!(prompts[0].contains("BODY OF THE DUE MEMORIES"), "{}", prompts[0]);
  assert!(prompts[0].contains("the room is quiet"), "{}", prompts[0]);
  assert!(
    prompts[1].contains("[memories/-500] (already loaded)"),
    "the memory reference rides as its mark: {}",
    prompts[1]
  );
  assert!(
    !prompts[1].contains("BODY OF THE DUE MEMORIES"),
    "the memory reference is not repeated: {}",
    prompts[1]
  );
  assert!(
    !prompts[1].contains("the room is quiet") && !prompts[1].contains(&CHAT_ROUND.situation.unchanged),
    "a note the session read rides no later turn, whole or as a marker: {}",
    prompts[1]
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
  round.rounds_over(1).await;
  round.hand(2, "agent-a and what else", true).await;
  round.rounds_over(2).await;

  let prompts = round.session.prompts();
  let block = &prompts[0];
  let turn = &prompts[1];
  let index = &CHAT_ROUND.index;

  assert!(
    block.contains(&format!("{} 4 (2 entries changed in the last three days)", index.room_rules)),
    "{block}"
  );
  assert!(block.contains(&format!("{} 7", index.knowledge)), "{block}");
  assert!(block.contains(&index.hint), "the nudge stands in the room's block");
  assert!(
    !turn.contains(&index.head) && !turn.contains(&index.hint),
    "a turn after the session's opening carries no index line: {turn}"
  );

  let asked = round.standing.gate.indexed.lock().unwrap().clone();

  assert_eq!(
    asked[0],
    (ROOM, Some(SESSION.to_owned()), Some(AGENT.to_owned())),
    "the index is the session's"
  );
}

#[tokio::test]
async fn a_line_that_asks_the_agent_is_answered_with_the_situation_note() {
  let round =
    Group::new(StandInSession::new(SESSION, "Here.").answering(&["They asked me.", "Here.", "Here."]))
      .await;

  // The line as the room's record holds it, which is where a judged round keeps the note it read under.
  round.scratch.message(ROOM, 1, 31).await;
  round.noted("They asked you to send the file.").await;
  round.hand(1, "can you send the file?", false).await;
  round.prompts_over(2).await;

  let prompts = round.session.prompts();
  let judging = turn_of(&prompts[0]);

  assert!(prompts.len() >= 2, "{prompts:#?}");
  assert!(
    judging.contains(&format!("{}\nThey asked you to send the file.", CHAT_ROUND.situation.front)),
    "the judgement reads the room's note: {judging}"
  );
  assert!(prompts[1].contains("TURN SCOPE"), "{}", prompts[1]);
  assert!(
    !prompts[1].contains("They asked you to send the file.")
      && !prompts[1].contains(&CHAT_ROUND.situation.unchanged),
    "the note is read once, and the draft turn carries none of it: {}",
    prompts[1]
  );
  assert_eq!(
    round
      .scratch
      .texts(r#"SELECT "text" FROM "room_situations" WHERE "chat_id" = -500"#)
      .await,
    ["They asked you to send the file."],
    "the round leaves the note as it stood"
  );
  // The note stands on the line the round judged, so the console's judgement record shows the situation the
  // round found.
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
    0,
    "no round writes the note"
  );
}

#[tokio::test]
async fn a_line_between_two_people_is_left_alone_though_the_note_names_them() {
  let round = Group::new(
    StandInSession::new(SESSION, "unused").answering(&["<<SKIP: the two of them are talking>>"]),
  )
  .await;

  round.noted("Mina and Joon are talking about the match; nobody waits on you.").await;
  round.hand(1, "Mina, did Joon see the match?", false).await;
  round.rounds_over(1).await;

  let prompts = round.session.prompts();

  assert_eq!(prompts.len(), 1, "{prompts:#?}");
  assert!(
    prompts[0].contains("Mina and Joon are talking about the match"),
    "the judgement reads the note: {}",
    prompts[0]
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
async fn a_line_passed_over_is_told_as_skipped_with_the_reason_the_skip_named() {
  let round = Group::new(
    StandInSession::new(SESSION, "unused").answering(&["<<SKIP: the two of them are talking>>"]),
  )
  .await;

  round.noted("Mina and Joon are talking about the match; nobody waits on you.").await;
  round.hand(1, "Mina, did Joon see the match?", false).await;
  round.rounds_over(1).await;

  let skipped = round.standing.heard.events("skipped");

  assert_eq!(skipped.len(), 1, "{skipped:#?}");
  assert_eq!(skipped[0]["verdict"], "SKIP");
  assert_eq!(skipped[0]["detail"], "the two of them are talking");
}

#[tokio::test]
async fn the_note_stays_under_its_ceiling_which_the_rooms_block_names() {
  let ceiling = CHAT_ROUND.situation.ceiling_chars;
  let long = "x".repeat(ceiling + 100);
  let note =
    bmo_receiver::chat::situation::note_from_answer(&long, &[]).expect("a long note is cut, not lost");

  assert!(
    bmo_receiver::js::units(&note) <= ceiling,
    "the note is {} units, over the ceiling of {ceiling}",
    bmo_receiver::js::units(&note)
  );
  assert!(note.ends_with(&CHAT_ROUND.situation.cut));

  let round = Group::new(
    StandInSession::new(SESSION, "unused").answering(&["<<SKIP: nothing here waits on the line>>"]),
  )
  .await;

  round.hand(1, "did anyone see the match last night", false).await;
  round.rounds_over(1).await;

  let prompts = round.session.prompts();

  assert!(
    prompts[0].contains(&format!("under {ceiling} characters")),
    "the room's block names the ceiling: {}",
    prompts[0]
  );
}

#[tokio::test]
async fn a_reply_mark_in_the_supervisors_answer_pins_the_older_line_it_names() {
  let round =
    Group::new(StandInSession::new(SESSION, "unused").answering(&["<<REPLY #1>> They asked me.", "Here."]))
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
  round.rounds_over(2).await;

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
        r#"SELECT coalesce("reply_to_message_id"::text, 'none') FROM "messages" WHERE "agent_name" IS NOT NULL"#
      )
      .await,
    ["1"],
    "the record names the older message"
  );
}

#[tokio::test]
async fn the_supervisor_prompt_is_the_golden_file() {
  let round = Group::new(StandInSession::new(SESSION, "Here.")).await;

  round.noted("Here.").await;
  round.hand(1, "are you there", false).await;
  round.prompts_over(1).await;

  let prompts = round.session.prompts();

  assert_eq!(turn_of(&prompts[0]), include_str!("../fixtures/supervisor-prompt.txt"));
}

#[tokio::test]
async fn the_judgement_row_keeps_the_reason_its_skip_named() {
  let round = Group::new(
    StandInSession::new(SESSION, "unused").answering(&["<<SKIP: a line of theirs, not for me>>"]),
  )
  .await;

  round.hand(1, "did anyone see the match last night", false).await;
  round.rounds_over(1).await;

  assert_eq!(round.session.prompts().len(), 1, "a skip naming its reason is the judgement's one turn");
  assert_eq!(
    round
      .scratch
      .texts(
        r#"SELECT "kind" || ' ' || "number" || ' ' || "outcome" || ' ' || coalesce("detail", '-')
             FROM "draft_rounds" ORDER BY "id""#
      )
      .await,
    ["supervisor 0 skip a line of theirs, not for me"],
    "the row says why the round ended"
  );
  assert!(round.chat.sent().is_empty(), "nothing goes out");
}

#[tokio::test]
async fn a_judgement_that_skips_bare_is_asked_once_over_and_keeps_none_given() {
  let round =
    Group::new(StandInSession::new(SESSION, "unused").answering(&["<<SKIP>>", "<<SKIP>>", "never asked"]))
      .await;

  round.hand(1, "did anyone see the match last night", false).await;
  round.rounds_over(2).await;

  let prompts = round.session.prompts();

  assert_eq!(prompts.len(), 2, "the same turn is asked once more: {prompts:#?}");
  assert!(
    turn_of(&prompts[1]).starts_with(&CHAT_ROUND.supervisor.marker),
    "the asking over names the judgement it asks over: {}",
    turn_of(&prompts[1])
  );
  assert!(
    prompts[1].contains(&DRAFTING.skip_again) && prompts[1].contains("<<SKIP: <why>>>"),
    "the line states the form: {}",
    prompts[1]
  );
  assert_eq!(
    round
      .scratch
      .texts(
        r#"SELECT "kind" || ' ' || "number" || ' ' || "outcome" || ' ' || coalesce("detail", '-')
             FROM "draft_rounds" ORDER BY "id""#
      )
      .await,
    ["supervisor 0 no-verdict -", "supervisor 1 skip none given"],
    "the round log says why, or that none was given"
  );
  assert!(round.chat.sent().is_empty(), "a round that ends at its judgement sends nothing");
}

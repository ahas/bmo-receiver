//! The room's situation note written when the agent's one session leaves a room (docs/drafting.md, 「The situation
//! note」): no round writes it; words in another room write the note of the room last answered in first, a handoff
//! and a keepalive touch write the note of every room answered in, and a session reads a room's note once.

use std::sync::Arc;
use std::time::Duration;

use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::cache::force_reset;
use bmo_receiver::chat::conversation::Timings;
use bmo_receiver::chat::runner::Runner;
use bmo_receiver::chat::situation::{owe, owed, update_of, written};
use bmo_receiver::chat::touch::{TouchOutcome, touch};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::{Context, Inbound, Room};
use receiver_codes::SessionReason;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::keepalive::KEEPALIVE;

use crate::integration::common::Scratch;
use crate::integration::common::session::{StandInSession, handoff_path, writing_handoff};
use crate::integration::common::units::{
  ADDRESS, AGENT, OWNER, StandInChat, Standing, deployment, registration, unrun, woken,
};

/// The id the agent's one session goes by, the agent's name alone.
const SESSION: &str = AGENT;
const ROOM: i64 = -500;
const IDENTITY: i64 = 7;

const OWNERS_NOTE: &str = "The owner asked what the plan holds; nothing is open.";
const ROOMS_NOTE: &str = "A member asked whether the agent is there; nothing is open.";

fn said(chat_id: i64, sender: i64, message_id: i64, text: &str) -> Inbound {
  Inbound {
    chat_id,
    message_id,
    sender_id: Some(sender),
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

async fn until(what: &str, holds: impl Fn() -> bool) {
  let waited = tokio::time::timeout(Duration::from_secs(60), async {
    while !holds() {
      tokio::time::sleep(Duration::from_millis(5)).await;
    }
  })
  .await;

  assert!(waited.is_ok(), "{what} never came");
}

/// An agent on its one session across two rooms, the owner's chat and a group, with the note's updates on.
struct Two {
  scratch: Scratch,
  _data: tempfile::TempDir,
  standing: Standing,
  session: Arc<StandInSession>,
  runner: Runner,
}

impl Two {
  async fn on(session: StandInSession) -> Self {
    let scratch = Scratch::new().await;

    crate::integration::common::session::no_drafting(scratch.db()).await;

    let data = tempfile::tempdir().unwrap();
    let chat = Arc::new(StandInChat::new("road-a").owners(OWNER).holding(&[ROOM]).agent_in(ROOM, IDENTITY));

    crate::integration::common::session::prompt_documents(data.path());

    let mut standing = woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat], deployment()));

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
    let mut agent = registration(AGENT, "Agent A", vec![ADDRESS], None);

    agent.row.insert("sessionScope".to_owned(), serde_json::json!("agent"));
    standing.agency.registering(vec![agent]);
    standing.agency.carrying_on(session.clone());
    receiver.hold_runner(Arc::new(runner.clone()));

    Two {
      scratch,
      _data: data,
      standing,
      session,
      runner,
    }
  }

  // The owner's words in his own chat, and the round they open run to its end.
  async fn owner(&self, message_id: i64, text: &str, turns: usize) {
    let room = Room {
      owner_room: true,
      identity: Some(IDENTITY),
      private_to: None,
      sealed: false,
    };

    self
      .standing
      .receiver
      .message(&said(OWNER, OWNER, message_id, text), &Context::default(), room)
      .await
      .unwrap();
    self.over(turns).await;
  }

  // A member's call in the group, naming the agent, and the round it opens run to its end.
  async fn group(&self, message_id: i64, text: &str, turns: usize) {
    let room = Room {
      owner_room: false,
      identity: Some(IDENTITY),
      private_to: None,
      sealed: false,
    };

    self.runner.of(AGENT).enqueue(said(ROOM, 31, message_id, text), room, true, false).await;
    self.over(turns).await;
  }

  async fn over(&self, turns: usize) {
    until("the round's end", || {
      self.session.prompts().len() >= turns && self.runner.idle() && !self.session.working()
    })
    .await;
    // The round's last writes land after its turn ends.
    tokio::time::sleep(Duration::from_millis(100)).await;
  }

  async fn note_of(&self, chat_id: i64) -> Vec<String> {
    self
      .scratch
      .texts(&format!(r#"SELECT "text" FROM "room_situations" WHERE "chat_id" = {chat_id}"#))
      .await
  }
}

fn is_note_turn(prompt: &str) -> bool {
  prompt.contains(&CHAT_ROUND.situation.marker) && prompt.contains(&CHAT_ROUND.situation.since)
}

#[tokio::test]
async fn words_in_another_room_write_the_note_of_the_room_last_answered_in_first() {
  let two = Two::on(StandInSession::new(SESSION, "unused").answering(&[
    "answer one",
    "answer two",
    OWNERS_NOTE,
    "answer in the group",
    ROOMS_NOTE,
    "answer three",
  ]))
  .await;
  let db = two.scratch.db();

  two.owner(1, "what does the plan hold", 1).await;
  two.owner(2, "and what after that", 2).await;

  let prompts = two.session.prompts();

  assert_eq!(prompts.len(), 2, "a round in the room last answered in is its one turn: {prompts:#?}");
  assert!(
    !prompts.iter().any(|prompt| is_note_turn(prompt)),
    "no round writes the note: {prompts:#?}"
  );
  assert!(two.note_of(OWNER).await.is_empty());
  assert_eq!(owed(db, AGENT).await, [OWNER], "the room answered in is owed its note");

  // Words in another room: the note of the room last answered in is written first, in that room.
  two.group(3, "agent-a are you there", 4).await;

  let prompts = two.session.prompts();
  let rooms = two.session.rooms();

  assert_eq!(prompts.len(), 4, "{prompts:#?}");
  assert!(is_note_turn(&prompts[2]), "{}", prompts[2]);
  assert_eq!(rooms[2], Some(OWNER), "the note's turn is a turn of the room the note is of");
  assert!(
    prompts[2].contains("what does the plan hold") && prompts[2].contains("answer two"),
    "the update reads the room's record, the agent's own answers among it: {}",
    prompts[2]
  );
  assert_eq!(rooms[3], Some(ROOM), "then the round goes on in the new room");
  assert!(prompts[3].contains("agent-a are you there") && !is_note_turn(&prompts[3]));
  assert_eq!(two.note_of(OWNER).await, [OWNERS_NOTE]);
  assert_eq!(owed(db, AGENT).await, [ROOM], "the room left is owed none, and the room answered in is");

  // Back in the first room: the group's note is written first, and the session reads no note it wrote itself.
  two.owner(4, "back to the plan", 6).await;

  let prompts = two.session.prompts();
  let rooms = two.session.rooms();

  assert_eq!(prompts.len(), 6, "{prompts:#?}");
  assert!(is_note_turn(&prompts[4]) && rooms[4] == Some(ROOM), "{}", prompts[4]);
  assert_eq!(two.note_of(ROOM).await, [ROOMS_NOTE]);
  assert!(
    !prompts[5].contains(OWNERS_NOTE) && !prompts[5].contains(&CHAT_ROUND.situation.front),
    "a session holds the note it wrote and is handed it by no round: {}",
    prompts[5]
  );
  assert_eq!(owed(db, AGENT).await, [OWNER]);
}

const HANDOFF: &str = "In the middle of the owner's question about the plan.";

#[tokio::test]
async fn a_handoff_writes_the_notes_owed_first_and_the_fresh_session_reads_them_once() {
  let two = Two::on(
    StandInSession::new(SESSION, "unused")
      .answering(&["answer one", "answer two", OWNERS_NOTE, "<<SKIP>>", "answer three", "answer four"])
      .acting(writing_handoff(HANDOFF)),
  )
  .await;
  let db = two.scratch.db();

  two.owner(1, "what does the plan hold", 1).await;
  force_reset(db, AGENT, SessionReason::Asked).await.unwrap();
  // The round that finds the word goes on in the session as it stands; the replacement is made once it has
  // ended, the note owed written first (docs/unified-bmo.md, 「Replacing the session」).
  two.owner(2, "and what after that", 4).await;

  let prompts = two.session.prompts();

  assert_eq!(prompts.len(), 4, "{prompts:#?}");
  assert!(
    prompts[1].contains("and what after that") && !is_note_turn(&prompts[1]),
    "the round goes on in the session as it stands: {}",
    prompts[1]
  );
  assert!(
    is_note_turn(&prompts[2]),
    "the session about to end writes the note it owes first: {}",
    prompts[2]
  );
  assert!(handoff_path(&prompts[3]).is_some(), "then it is asked for its handoff: {}", prompts[3]);
  assert_eq!(two.standing.agency.ended(), [SESSION]);
  assert_eq!(two.note_of(OWNER).await, [OWNERS_NOTE]);

  two.owner(3, "one more thing", 5).await;

  let prompts = two.session.prompts();

  assert!(prompts[4].contains(HANDOFF), "the fresh session opens on the handoff: {}", prompts[4]);
  assert!(
    prompts[4].contains(&format!("{}\n{OWNERS_NOTE}", CHAT_ROUND.situation.front)),
    "and reads the room's note in its first round there: {}",
    prompts[4]
  );
  assert!(
    !prompts[4].contains(&format!("{HANDOFF}\n{OWNERS_NOTE}")),
    "the note is the room's own and no part of the handoff document"
  );

  two.owner(4, "yet another thing", 6).await;

  let prompts = two.session.prompts();

  assert!(
    !prompts[5].contains(OWNERS_NOTE) && !prompts[5].contains(&CHAT_ROUND.situation.unchanged),
    "the fresh session reads the note once: {}",
    prompts[5]
  );
}

#[tokio::test]
async fn a_touch_writes_the_notes_owed_and_the_keepalive_prompt_goes_in_only_where_none_is() {
  let two = Two::on(StandInSession::new(SESSION, "unused").on_provider("claude-code").answering(&[
    "answer one",
    OWNERS_NOTE,
    "nothing",
  ]))
  .await;
  let db = two.scratch.db();
  let receiver = two.standing.receiver.clone();

  two.owner(1, "what does the plan hold", 1).await;
  assert_eq!(owed(db, AGENT).await, [OWNER]);

  let touched = touch(&receiver, &two.runner.of(AGENT), AGENT, Some(0)).await;
  let prompts = two.session.prompts();

  assert_eq!(touched.outcome, TouchOutcome::Warmed, "{touched:?}");
  assert_eq!(prompts.len(), 2, "{prompts:#?}");
  assert!(
    is_note_turn(&prompts[1]) && !prompts[1].contains(&KEEPALIVE.prompt),
    "the note's turn is the touch: {}",
    prompts[1]
  );
  assert_eq!(two.note_of(OWNER).await, [OWNERS_NOTE]);
  assert!(owed(db, AGENT).await.is_empty(), "the touch wrote every note owed");

  let again = touch(&receiver, &two.runner.of(AGENT), AGENT, Some(0)).await;
  let prompts = two.session.prompts();

  assert_eq!(again.outcome, TouchOutcome::Warmed, "{again:?}");
  assert_eq!(prompts.len(), 3, "{prompts:#?}");
  assert!(
    prompts[2].contains(&KEEPALIVE.prompt) && !is_note_turn(&prompts[2]),
    "with no note owed the keepalive prompt goes in: {}",
    prompts[2]
  );
}

#[tokio::test]
async fn a_room_whose_record_holds_nothing_past_its_note_is_owed_none_and_asked_no_turn() {
  let two = Two::on(StandInSession::new(SESSION, "unused")).await;
  let db = two.scratch.db();

  owe(db, AGENT, ROOM).await;
  owe(db, AGENT, OWNER).await;
  owe(db, AGENT, ROOM).await;
  assert_eq!(owed(db, AGENT).await, [OWNER, ROOM], "the rooms stand in the order of the last answer");

  let written = two.runner.of(AGENT).owed_notes(Some(OWNER), None).await.unwrap();

  assert_eq!(written, 0, "an empty record is updated by no turn");
  assert!(two.session.prompts().is_empty());
  assert_eq!(owed(db, AGENT).await, [OWNER], "the room a round stays in is left owed");
}

#[tokio::test]
async fn a_note_far_behind_its_room_reads_the_newest_messages_and_moves_its_mark() {
  let two = Two::on(StandInSession::new(SESSION, "unused")).await;
  let db = two.scratch.db();

  // One message the note has read, then a gap longer than the policy's bound behind it.
  for id in 1..=202i64 {
    two
      .scratch
      .run(
        r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text")
           VALUES (42, $1, 42, 'tester', $2, $3)"#,
        vec![id.into(), (1000 + id).into(), format!("m-{id:03}").into()],
      )
      .await;
  }

  let row_of = |id: i64| {
    format!(r#"SELECT "rowid"::text FROM "messages" WHERE "chat_id" = 42 AND "message_id" = {id}"#)
  };
  let read_to: i64 = two.scratch.texts(&row_of(1)).await.remove(0).parse().unwrap();

  two
    .scratch
    .run(
      r#"INSERT INTO "room_situations" ("chat_id", "text", "updated_at", "read_through")
         VALUES (42, 'a standing note', 0, $1)"#,
      vec![read_to.into()],
    )
    .await;

  let update = update_of(&two.standing.receiver, OWNER, OWNER)
    .await
    .unwrap()
    .expect("the record moved");

  assert!(
    !update.prompt.contains("m-001") && !update.prompt.contains("m-002"),
    "a gap longer than the bound is not read whole: {}",
    update.prompt
  );
  assert!(update.prompt.contains("m-003") && update.prompt.contains("m-202"), "{}", update.prompt);
  assert!(update.prompt.contains("a standing note"), "{}", update.prompt);

  written(db, &update, None, "<<SKIP>>").await.unwrap();

  assert_eq!(
    two.note_of(OWNER).await,
    ["a standing note"],
    "a skip leaves the note's words as they stood"
  );
  assert_eq!(
    two
      .scratch
      .texts(r#"SELECT "read_through"::text FROM "room_situations" WHERE "chat_id" = 42"#)
      .await,
    two.scratch.texts(&row_of(202)).await,
    "the mark moves to the newest message read"
  );
  assert!(
    update_of(&two.standing.receiver, OWNER, OWNER).await.unwrap().is_none(),
    "nothing is left to read"
  );
}

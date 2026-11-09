//! A working supervisor's thinking sent into the owner's private chat as its own messages (docs/receiver.md,
//! 「The event stream」; docs/messages.md): each block as the run wrote it, marked as thinking and cut past the
//! policy's ceiling, while the turn works; no other room is sent to; and the turn's answer and stored record are
//! what they were.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use bmo_chat_client::{ChatSend, ChatSendRequest};
use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::conversation::Timings;
use bmo_receiver::chat::runner::Runner;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::{Context, Inbound, Room};
use receiver_policy::chat_line::CHAT_LINE;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_ports::gate::{Entry, Source};
use serde_json::json;

use crate::integration::common::Scratch;
use crate::integration::common::session::{StandInSession, no_drafting};
use crate::integration::common::units::{
  ADDRESS, AGENT, OWNER, StandInChat, Standing, deployment, registration, unrun, woken,
};

const ANSWER: &str = "Here is what was asked.";
const SESSION: &str = "agent-a@agent";
const OTHER: i64 = -7;

fn owners_room() -> Room {
  Room {
    owner_room: true,
    identity: Some(7),
    private_to: None,
  }
}

fn a_group() -> Room {
  Room {
    owner_room: false,
    identity: None,
    private_to: None,
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
  let waited = tokio::time::timeout(Duration::from_secs(30), async {
    while !holds() {
      tokio::time::sleep(Duration::from_millis(5)).await;
    }
  })
  .await;

  assert!(waited.is_ok(), "{what} never came");
}

fn text_of(request: &ChatSendRequest) -> &str {
  match &request.send {
    ChatSend::Text { text, .. } => text,
    _ => panic!("a thinking line is text"),
  }
}

/// Every send into a room that is not the answer: the thinking messages, which carry no label in their words.
fn thinking_sends(chat: &StandInChat, chat_id: i64) -> Vec<ChatSendRequest> {
  chat
    .sent()
    .into_iter()
    .filter(|request| request.room_id == chat_id && text_of(request) != ANSWER)
    .collect()
}

/// The default answerer's conversations standing on the stand-in units, its turn answering as the test scripts.
struct Round {
  scratch: Scratch,
  _data: tempfile::TempDir,
  standing: Standing,
  session: Arc<StandInSession>,
  chat: Arc<StandInChat>,
  runner: Runner,
}

impl Round {
  async fn new(session: StandInSession, unified: bool) -> Self {
    let scratch = Scratch::new().await;

    no_drafting(scratch.db()).await;

    let data = tempfile::tempdir().unwrap();
    let chat = Arc::new(StandInChat::new("road-a").owners(OWNER).holding(&[OTHER]));

    documents(data.path());

    let standing = woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat.clone()], deployment()));

    *standing.gate.entries.lock().unwrap() = vec![Entry {
      path: "knowledge/plans".to_owned(),
      text: "the plan for the week".to_owned(),
      source: Source::Knowledge,
      fragment: None,
      holds: None,
    }];

    let receiver = standing.receiver.clone();
    let runner = Runner::new(receiver.clone(), Arc::new(Around::new(receiver.clone())), timings(), AGENT);
    let mut agent = registration(AGENT, "Agent A", vec![ADDRESS], None);

    if unified {
      agent.row.insert("sessionScope".to_owned(), json!("agent"));
    }

    standing.agency.registering(vec![agent]);

    let session = Arc::new(session);

    standing.agency.carrying_on(session.clone());
    receiver.hold_runner(Arc::new(runner.clone()));

    Round {
      scratch,
      _data: data,
      standing,
      session,
      chat,
      runner,
    }
  }

  /// A thinking block written into the working turn's run log.
  fn thought(&self, at: i64, text: &str) {
    let line = json!({ "id": at, "kind": "thinking", "text": text });

    self.session.logged.lock().unwrap().push(line.as_object().unwrap().clone());
  }

  /// The words in `chat_id`, waited on until the turn stands working.
  async fn working(&self, chat_id: i64, room: Room, message_id: i64, text: &str) {
    self
      .standing
      .receiver
      .message(&said(chat_id, message_id, text), &Context::default(), room)
      .await
      .unwrap();
    until("the turn to stand working", || self.session.working()).await;
  }

  /// The words in `chat_id`, the round run to its end and its answer sent.
  async fn run(&self, chat_id: i64, room: Room, message_id: i64, text: &str) {
    let prompts = self.session.prompts().len();
    let answers = self.answers();

    self
      .standing
      .receiver
      .message(&said(chat_id, message_id, text), &Context::default(), room)
      .await
      .unwrap();
    until("the round's end", || {
      self.session.prompts().len() > prompts && self.runner.idle() && !self.session.working()
    })
    .await;
    until("the answer to go out", || self.answers() > answers).await;
    // The round's writes land after its turn ends, as the other round tests settle them.
    tokio::time::sleep(Duration::from_millis(100)).await;
  }

  fn answers(&self) -> usize {
    self.chat.sent().iter().filter(|request| text_of(request) == ANSWER).count()
  }

  /// The held turn let go, the round run to its end, and every send it made.
  async fn ended(&self) -> Vec<ChatSendRequest> {
    self.session.let_go();
    until("the round's end", || self.runner.idle() && !self.session.working()).await;

    let expected = ANSWER.to_owned();

    until("the answer to go out", || {
      self.chat.sent().iter().any(|request| text_of(request) == expected)
    })
    .await;

    self.chat.sent()
  }
}

#[tokio::test]
async fn a_working_turns_thinking_goes_into_the_owners_chat_cut_and_marked_in_the_record() {
  const SHORT: &str = "weighing what the room asked";
  const LONG: &str = "thinking a very long thought";
  // Assembled at run time, so no tracked file holds a secret-shaped value.
  let secret = format!("{}{}", "sk-", "abcdefghijklmnopqrstuvwxyz012345");
  let long: String = LONG.repeat(400);
  let round = Round::new(StandInSession::new(SESSION, ANSWER).holding_the_first(), true).await;

  round.working(OWNER, owners_room(), 1, "what is new").await;
  round.thought(1, SHORT);
  round.thought(2, &long);
  round.thought(3, &format!("the key {secret} is in the file"));

  until("the thinking to go out while the turn works", || {
    thinking_sends(&round.chat, OWNER).len() == 3
  })
  .await;

  let sent = thinking_sends(&round.chat, OWNER);

  assert_eq!(sent[0].room_id, OWNER, "the owner's private chat alone");
  assert_eq!(text_of(&sent[0]), SHORT, "the words carry no label of their own");
  assert!(round.session.working(), "the thinking went out while the turn worked");
  assert!(text_of(&sent[1]).ends_with('…'), "a long block is cut and the cut marked");
  assert!(
    text_of(&sent[1]).chars().count() <= CHAT_ROUND.thinking.ceiling_chars + 1,
    "the block is cut at the ceiling"
  );
  assert!(
    !text_of(&sent[2]).contains(&secret),
    "a secret-shaped run is taken out before it reaches the room"
  );

  let all = round.ended().await;
  let answer: Vec<&ChatSendRequest> = all.iter().filter(|request| text_of(request) == ANSWER).collect();

  assert_eq!(answer.len(), 1, "the answer goes out once, as before");
  assert_eq!(answer[0].room_id, OWNER);

  // The words carry no label; the room's record marks each thinking message with the policy's kind, and the
  // answer alone is an answer.
  assert_eq!(
    round
      .scratch
      .count(
        r#"SELECT count(*) FROM "messages" WHERE "chat_id" = $1 AND "kind" = $2"#,
        vec![OWNER.into(), CHAT_ROUND.thinking.kind.clone().into()],
      )
      .await,
    3,
    "each thinking message is kept under the thinking kind"
  );
  assert_eq!(
    round
      .scratch
      .count(
        r#"SELECT count(*) FROM "messages" WHERE "chat_id" = $1 AND "answer_seconds" IS NOT NULL"#,
        vec![OWNER.into()],
      )
      .await,
    1,
    "one answer stands in the room"
  );
  assert_eq!(
    round
      .scratch
      .count(
        r#"SELECT count(*) FROM "messages" WHERE "chat_id" = $1 AND "text" = $2 AND "kind" IS NULL"#,
        vec![OWNER.into(), ANSWER.into()],
      )
      .await,
    1,
    "the answer as it was asked for stands in the record, under no thinking kind"
  );
}

#[tokio::test]
async fn a_turn_in_another_room_sends_no_thinking() {
  let round = Round::new(StandInSession::new(SESSION, ANSWER).at_generation(0), true).await;

  round.run(OWNER, owners_room(), 1, "what is new").await;
  round.thought(1, "a thought the owner's chat alone may hear");
  round.run(OTHER, a_group(), 2, "agent-a, what is new in this room").await;

  assert!(
    thinking_sends(&round.chat, OTHER).is_empty(),
    "no thinking goes into a group room: {:?}",
    round.chat.sent()
  );
  assert_eq!(
    round
      .scratch
      .count(
        r#"SELECT count(*) FROM "messages" WHERE "chat_id" = $1 AND "kind" = $2"#,
        vec![OTHER.into(), CHAT_ROUND.thinking.kind.clone().into()],
      )
      .await,
    0,
    "no thinking message stands in the group room's record"
  );
  assert!(
    round
      .chat
      .sent()
      .iter()
      .any(|request| request.room_id == OTHER && text_of(request) == ANSWER),
    "the answer still goes out"
  );
}

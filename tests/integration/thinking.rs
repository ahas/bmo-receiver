//! A working supervisor's thinking carried in its turn's picture (docs/receiver.md, 「The event stream」;
//! docs/messages.md, 「A working turn's thinking」): the run's last block, redacted and cut past the policy's
//! ceiling, on a turn working in the owner's private chat alone; never sent as a message; and a message of kind
//! thinking a record holds left out of what a turn is handed.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use bmo_chat_client::{ChatSend, ChatSendRequest};
use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::conversation::Timings;
use bmo_receiver::chat::runner::Runner;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::{Context, Inbound, Room, rows};
use receiver_policy::chat_line::CHAT_LINE;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_ports::gate::{Entry, Source};
use serde_json::json;

use crate::integration::common::Scratch;
use crate::integration::common::session::{StandInSession, no_drafting};
use crate::integration::common::units::{
  ADDRESS, AGENT, Heard, OWNER, StandInChat, Standing, deployment, registration, unrun, woken,
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
    _ => panic!("the round's sends are text"),
  }
}

/// The thinking block each picture the stream was told of a room's turn carried, in order, a picture carrying none
/// left out.
fn thinking_shown(heard: &Heard, chat_id: i64) -> Vec<String> {
  heard
    .events("run-progress")
    .iter()
    .filter(|event| event["chatId"] == chat_id)
    .filter_map(|event| event["progress"]["thinking"].as_str().map(str::to_owned))
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
async fn a_working_turns_picture_carries_its_last_thinking_block_and_no_message_is_sent() {
  const SHORT: &str = "weighing what the room asked";
  const LONG: &str = "thinking a very long thought";
  // Assembled at run time, so no tracked file holds a secret-shaped value.
  let secret = format!("{}{}", "sk-", "abcdefghijklmnopqrstuvwxyz012345");
  let long: String = LONG.repeat(400);
  let leaked = format!("the key {secret} is in the file");
  let round = Round::new(StandInSession::new(SESSION, ANSWER).holding_the_first(), true).await;
  let heard = round.standing.heard.clone();
  let last = |heard: &Heard| thinking_shown(heard, OWNER).last().cloned().unwrap_or_default();

  round.working(OWNER, owners_room(), 1, "what is new").await;

  round.thought(1, SHORT);
  until("the first block to stand in the picture", || last(&heard) == SHORT).await;
  assert!(round.session.working(), "the picture carried it while the turn worked");

  round.thought(2, &long);
  until("the newer block to take its place", || last(&heard).starts_with(LONG)).await;
  assert!(last(&heard).ends_with('…'), "a long block is cut and the cut marked");
  assert!(
    last(&heard).chars().count() <= CHAT_ROUND.thinking.ceiling_chars + 1,
    "the block is cut at the ceiling"
  );

  round.thought(3, &leaked);
  until("the last block to take its place", || last(&heard).starts_with("the key ")).await;
  assert!(
    !last(&heard).contains(&secret),
    "a secret-shaped run is taken out before the picture carries it"
  );

  let all = round.ended().await;

  assert_eq!(all.len(), 1, "the answer alone is sent: {all:?}");
  assert_eq!(text_of(&all[0]), ANSWER);
  assert_eq!(all[0].room_id, OWNER);

  // No thinking block is a message: the room's record holds the words it was said and the answer.
  assert_eq!(
    round
      .scratch
      .count(
        r#"SELECT count(*) FROM "messages" WHERE "chat_id" = $1 AND "agent_name" IS NOT NULL"#,
        vec![OWNER.into()]
      )
      .await,
    1,
    "the answer is the agent's one message in the room"
  );
  assert_eq!(
    round
      .scratch
      .count(r#"SELECT count(*) FROM "messages" WHERE "kind" = 'thinking'"#, vec![])
      .await,
    0,
    "no thinking message is kept"
  );
}

#[tokio::test]
async fn the_picture_of_a_turn_in_another_room_carries_no_thinking() {
  let round = Round::new(StandInSession::new(SESSION, ANSWER).holding_the_first(), true).await;

  round.working(OTHER, a_group(), 1, "agent-a, what is new in this room").await;
  round.thought(1, "a thought the owner's chat alone may see");

  let all = round.ended().await;

  assert!(
    thinking_shown(&round.standing.heard, OTHER).is_empty(),
    "no picture of a group room's turn carries thinking"
  );
  assert!(
    !round.standing.heard.events("run-progress").is_empty(),
    "the turn's picture still went out"
  );
  assert!(all.iter().any(|request| text_of(request) == ANSWER), "the answer still goes out");
}

// A message of kind thinking a record holds is no part of the room's conversation: no turn is handed it.
#[tokio::test]
async fn a_thinking_message_a_record_holds_is_none_of_the_rooms_recent_messages() {
  let scratch = Scratch::new().await;

  for (id, kind, text) in
    [(1, None, "the room's own words"), (2, Some("thinking"), "a block the record holds")]
  {
    scratch
      .run(
        r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_name", "sent_at", "text", "kind")
           VALUES ($1, $2, 'agent-a', $2, $3, $4)"#,
        vec![OWNER.into(), i64::from(id).into(), text.into(), kind.map(str::to_owned).into()],
      )
      .await;
  }

  let recent = rows::recent_messages(scratch.db(), OWNER, 50, "").await.unwrap();

  assert_eq!(recent.iter().map(|message| message.message_id).collect::<Vec<_>>(), [1]);
  assert_eq!(rows::messages_after(scratch.db(), OWNER, 0, "").await.unwrap().len(), 1);
  assert_eq!(rows::count_since_row(scratch.db(), OWNER, 0).await.unwrap(), 1);
}

//! The default answerer's road closed by the rest (docs/receiver.md; docs/wake.md, 「Who reaches the session」;
//! docs/bmo-window.md, 「Quiet instruction」): while its rounds fail, words that wake a supervisor on an open road
//! in a room other than the owner's still start that supervisor's round, and a quiet instruction for the rested
//! road waits for it rather than failing in rounds and being let go.

use std::sync::Arc;
use std::time::Duration;

use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::conversation::Timings;
use bmo_receiver::chat::instructions::{Queued, deliver_pending, queue};
use bmo_receiver::chat::runner::Runner;
use bmo_receiver::chat::talk::{TalkFailure, TalkRequest};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::now_ms;
use bmo_receiver::receiver::{Context, Inbound, Room, rows};
use receiver_ports::agency::{Rest, RestKind};
use serde_json::json;

use crate::integration::common::Scratch;
use crate::integration::common::session::{StandInSession, no_stages, prompt_documents};
use crate::integration::common::units::{
  ADDRESS, AGENT, OWNER, StandInChat, Standing, deployment, registration, unrun, woken,
};

/// The supervisor on an open road, owned by the person's account.
const SUPERVISOR: &str = "agent-b";

/// The person whose own BMO the supervisor is, and the account the account model links them to.
const PERSON: i64 = 9601;
const ACCOUNT: i64 = 96;

/// The person's private room with their own BMO, and a group room naming the supervisor.
const PRIVATE: i64 = -161;
const GROUP: i64 = -162;

/// The owner's words in his own chat, whose round the default answerer's session fails.
const ASKED: &str = "is anyone there";

/// How long a wait on what the rounds do stands before the test fails: each comes within milliseconds.
const BOUND: Duration = Duration::from_secs(3);

fn said(chat_id: i64, message_id: i64, sender: i64, text: &str) -> Inbound {
  Inbound {
    chat_id,
    message_id,
    sender_id: Some(sender),
    sender_name: "tester".into(),
    sent_at: (1_760_000_000 + message_id) * 1_000,
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

fn room(owner_room: bool, private_to: Option<i64>) -> Room {
  Room {
    owner_room,
    identity: None,
    private_to,
    sealed: false,
  }
}

// Waits, bounded, until `holds` does.
async fn until(what: &str, holds: impl Fn() -> bool) {
  let waited = tokio::time::timeout(BOUND, async {
    while !holds() {
      tokio::time::sleep(Duration::from_millis(5)).await;
    }
  })
  .await;

  assert!(waited.is_ok(), "{what} never came");
}

/// The receiver's own wake and conversations over the stand-in units, with the rest standing and only the
/// supervisor's road answering through it.
struct Rested {
  scratch: Scratch,
  _data: tempfile::TempDir,
  standing: Standing,
  session: Arc<StandInSession>,
  runner: Runner,
}

impl Rested {
  async fn new() -> Self {
    let scratch = Scratch::new().await;

    no_stages(scratch.db()).await;

    let data = tempfile::tempdir().unwrap();

    prompt_documents(data.path());

    let chat = Arc::new(
      StandInChat::new("road-a")
        .owners(OWNER)
        .holding(&[PRIVATE, GROUP])
        .privating(PRIVATE, PERSON),
    );
    let standing = woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat], deployment()));
    let session = Arc::new(StandInSession::new("session-a", "Here."));
    let receiver = standing.receiver.clone();
    let timings = Timings {
      queue: Duration::from_millis(1),
      wait: Duration::from_secs(60),
      window: Some(Duration::from_secs(60)),
      look: Duration::from_millis(5),
    };
    let runner = Runner::new(receiver.clone(), Arc::new(Around::new(receiver.clone())), timings, AGENT);
    let mut supervisor = registration(SUPERVISOR, "Agent B", vec![SUPERVISOR], None);

    supervisor.row.insert("ownerAccount".to_owned(), json!(ACCOUNT));
    standing
      .agency
      .registering(vec![registration(AGENT, "Agent A", vec![ADDRESS], None), supervisor]);
    standing.agency.carrying_on(session.clone());
    standing.agency.at_rest(Rest {
      resting: true,
      until: Some(4_102_444_800),
      kind: Some(RestKind::Limit),
    });
    standing.agency.answering_on_their_roads(vec![SUPERVISOR.to_owned()]);
    standing.web.linking(PERSON, ACCOUNT.to_string());
    receiver.hold_runner(Arc::new(runner.clone()));
    // Each room's first launch is paced behind the last on the real clock; these rooms count as launched, so the
    // supervisor's round is not held behind the default answerer's.
    receiver.launches().lock().await.rooms.extend([OWNER, PRIVATE, GROUP]);

    for chat_id in [PRIVATE, GROUP] {
      rows::allow_chat(scratch.db(), chat_id).await.unwrap();
    }

    scratch
      .run(
        r#"INSERT INTO "room_supervisors" ("chat_id", "supervisor") VALUES ($1, $2)"#,
        vec![GROUP.into(), SUPERVISOR.into()],
      )
      .await;

    Rested {
      scratch,
      _data: data,
      standing,
      session,
      runner,
    }
  }

  async fn say(&self, message: Inbound, room: Room) {
    self.standing.receiver.message(&message, &Context::default(), room).await.unwrap();
  }

  fn turned_in(&self, chat_id: i64) -> bool {
    self.session.rooms().contains(&Some(chat_id))
  }

  // The default answerer's round in the owner's chat, failed as a session on a rested road fails its turn; the
  // turns after it answer again.
  async fn failed_in_the_owners_chat(&self) {
    self.session.end_as(Some(json!({ "outcome": "failed", "reason": "the road rests" })));
    self.say(said(OWNER, 1, OWNER, ASKED), room(true, None)).await;
    until("the default answerer's failed round", || {
      self
        .standing
        .heard
        .events("run-failed")
        .iter()
        .any(|event| event["chatId"] == json!(OWNER))
    })
    .await;
    self.session.end_as(None);
  }

  // The row's standing as `handed|failures|let go`.
  async fn row(&self, id: i64) -> Vec<String> {
    self
      .scratch
      .texts(&format!(
        r#"SELECT (CASE WHEN "handed_at" IS NULL THEN 'pending' ELSE 'handed' END) || '|' || "failures" || '|' || COALESCE("let_go", '-') FROM "instructions" WHERE "id" = {id}"#
      ))
      .await
  }
}

#[tokio::test]
async fn a_persons_words_in_their_own_supervisors_room_start_its_round_while_the_default_road_rests() {
  let rested = Rested::new().await;

  rested.failed_in_the_owners_chat().await;
  rested.say(said(PRIVATE, 2, PERSON, "are you there"), room(false, Some(PERSON))).await;

  until("the supervisor's turn in the private room", || rested.turned_in(PRIVATE)).await;
}

#[tokio::test]
async fn the_owners_words_in_a_group_room_naming_a_supervisor_start_its_round_while_the_default_road_rests()
{
  let rested = Rested::new().await;

  rested.failed_in_the_owners_chat().await;
  rested.say(said(GROUP, 2, OWNER, "what is planned for today"), room(false, None)).await;

  until("the supervisor's turn in the group room", || rested.turned_in(GROUP)).await;
}

#[tokio::test]
async fn an_instruction_for_a_rested_road_waits_for_it_uncounted_and_goes_once_the_road_answers() {
  let rested = Rested::new().await;
  let receiver = &rested.standing.receiver;
  let text = "a notice for the owner";
  let id = queue(receiver.db(), AGENT, OWNER, text, &Queued::default()).await.unwrap();
  let mut last = 0;

  deliver_pending(receiver, &rested.runner, &mut last, now_ms(), 0).await.unwrap();

  assert_eq!(rested.row(id).await, ["pending|0|-"], "the rested road's row waits, uncounted");
  assert!(rested.session.prompts().is_empty());

  rested.standing.agency.answering_on_their_roads(vec![AGENT.to_owned()]);
  deliver_pending(receiver, &rested.runner, &mut last, now_ms(), 0).await.unwrap();

  until("the row's turn", || rested.session.prompts().iter().any(|prompt| prompt.contains(text))).await;
}

// A console talk is a round of the owner's chat on the default answerer's session: while the rest closes that
// road it is refused as resting before any turn goes out, and once the road answers it goes out.
#[tokio::test]
async fn a_talk_while_the_rest_closes_the_default_road_is_refused_as_resting_with_no_turn() {
  let rested = Rested::new().await;
  let talk = TalkRequest {
    head: vec!["A document".to_owned()],
    turns: vec![],
    said: "what does this say".to_owned(),
  };
  let conversations = rested.runner.default_answerer();

  assert_eq!(conversations.talk(&talk).await, Err(TalkFailure::Resting));
  assert!(rested.session.prompts().is_empty(), "no turn went out");

  rested.standing.agency.answering_on_their_roads(vec![AGENT.to_owned()]);

  assert_ne!(conversations.talk(&talk).await, Err(TalkFailure::Resting));
  assert!(!rested.session.prompts().is_empty(), "the turn went out");
}

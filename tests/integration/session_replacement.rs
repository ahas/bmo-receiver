//! Replacing the chat agent's one session (docs/unified-bmo.md, 「Replacing the session」): a turn on an unmoved
//! head resumes the session whatever its own composition carries, and a head the agency says moved replaces it.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::conversation::Timings;
use bmo_receiver::chat::runner::Runner;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::{Context, Inbound, Room};
use receiver_policy::chat_line::CHAT_LINE;

use crate::common::Scratch;
use crate::common::session::StandInSession;
use crate::common::units::{
  ADDRESS, AGENT, OWNER, StandInChat, Standing, deployment, registration, unrun, woken,
};

const ANSWER: &str = "Here is what was asked.";

/// The id the agent's one session goes by, the agent's name alone.
const SESSION: &str = AGENT;

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

/// The chat agent on its one session across its rooms, in the owner's chat.
struct Unified {
  _scratch: Scratch,
  _data: tempfile::TempDir,
  standing: Standing,
  session: Arc<StandInSession>,
  runner: Runner,
}

impl Unified {
  async fn on(session: StandInSession) -> Self {
    let scratch = Scratch::new().await;

    crate::common::session::no_drafting(scratch.db()).await;
    let data = tempfile::tempdir().unwrap();
    let chat = Arc::new(StandInChat::new("road-a").owners(OWNER));

    documents(data.path());

    let standing = woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat], deployment()));
    let session = Arc::new(session);
    let receiver = standing.receiver.clone();
    let runner = Runner::new(receiver.clone(), Arc::new(Around::new(receiver.clone())), timings(), AGENT);
    let mut agent = registration(AGENT, "Agent A", vec![ADDRESS], None, true);

    agent.row.insert("sessionScope".to_owned(), serde_json::json!("agent"));
    standing.agency.registering(vec![agent]);
    standing.agency.carrying_on(session.clone());
    receiver.hold_runner(Arc::new(runner.clone()));

    Unified {
      _scratch: scratch,
      _data: data,
      standing,
      session,
      runner,
    }
  }

  // The owner's words, and the round they open run to its end.
  async fn round(&self, message_id: i64, text: &str) {
    let before = self.session.prompts().len();

    self
      .standing
      .receiver
      .message(&said(message_id, text), &Context::default(), owners_room())
      .await
      .unwrap();
    until("the round's end", || {
      self.session.prompts().len() > before && self.runner.idle() && !self.session.working()
    })
    .await;
  }
}

#[tokio::test]
async fn two_rounds_of_a_room_on_an_unmoved_head_go_on_the_one_session() {
  let unified = Unified::on(StandInSession::new(SESSION, ANSWER)).await;

  unified.round(1, "the first words").await;
  unified.round(2, "the second words").await;

  let prompts = unified.session.prompts();

  // The two turns differ in their scope line and their words, and neither replaces the session.
  assert_eq!(prompts.len(), 2, "{prompts:?}");
  assert_ne!(prompts[0], prompts[1]);
  assert!(prompts[0].contains("TURN SCOPE") && prompts[1].contains("TURN SCOPE"));
  assert!(unified.standing.agency.ended().is_empty(), "no session was ended");
  assert!(
    prompts.iter().all(|prompt| !prompt.contains("previous session was closed")),
    "no turn is told of a replacement: {prompts:?}"
  );
}

#[tokio::test]
async fn a_head_the_agency_says_moved_replaces_the_session() {
  let unified = Unified::on(StandInSession::new(SESSION, ANSWER)).await;

  unified.round(1, "the first words").await;
  unified.session.prefix_changed("reload: a prompt document");
  unified.round(2, "the second words").await;

  let ended = unified.standing.agency.ended();

  assert_eq!(ended, [SESSION], "a reason the last turn did not keep replaces the session once");

  let count = unified.session.prompts().len();

  unified.round(3, "the third words").await;
  assert_eq!(unified.standing.agency.ended(), ended, "the reason kept moves nothing more");
  assert!(unified.session.prompts().len() > count);
}

#[tokio::test]
async fn a_session_opened_before_with_no_record_of_its_head_is_replaced_and_one_never_opened_is_not() {
  let opened = Unified::on(StandInSession::new(SESSION, ANSWER).at_generation(3)).await;

  opened.round(1, "the first words").await;
  assert_eq!(opened.standing.agency.ended(), [SESSION]);

  let fresh = Unified::on(StandInSession::new(SESSION, ANSWER).at_generation(0)).await;

  fresh.round(1, "the first words").await;
  assert!(fresh.standing.agency.ended().is_empty());
}

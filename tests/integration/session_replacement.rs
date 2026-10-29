//! Replacing the chat agent's one session (docs/unified-bmo.md, 「Replacing the session」): a turn resumes the
//! session whatever its own composition carries, and a moved head or a session with no record of its head resumes
//! it too; only a session past its cache lifetime, or the owner's own word, replaces it. Before it is ended the old
//! session writes its handoff file, which the new session's first turn carries; where it writes nothing the
//! receiver's transcript of it stands in, and only where neither stands is nothing recovered and the owner told so.

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

/// The chat agent on its one session across its rooms, in the owner's chat.
struct Unified {
  scratch: Scratch,
  data: tempfile::TempDir,
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
      scratch,
      data,
      standing,
      session,
      runner,
    }
  }

  // The notices the owner's chat was queued of a handoff lost.
  async fn lost_notices(&self) -> Vec<String> {
    self
      .scratch
      .texts(&format!(
        r#"SELECT "text" FROM "instructions" WHERE "chat_id" = {OWNER} AND "kind" = 'handoff-lost'"#
      ))
      .await
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
async fn a_head_the_agency_says_moved_resumes_the_session() {
  let unified = Unified::on(StandInSession::new(SESSION, ANSWER)).await;

  unified.round(1, "the first words").await;
  unified.session.prefix_changed("reload: a prompt document");
  unified.round(2, "the second words").await;

  let prompts = unified.session.prompts();

  assert_eq!(prompts.len(), 2, "{prompts:?}");
  assert!(unified.standing.agency.ended().is_empty(), "a moved head never replaces the session");
  assert!(
    prompts.iter().all(|prompt| !prompt.contains("previous session was closed")),
    "no turn is told of a replacement: {prompts:?}"
  );
}

#[tokio::test]
async fn a_session_with_no_record_of_its_head_is_resumed() {
  let opened = Unified::on(StandInSession::new(SESSION, ANSWER).at_generation(3)).await;

  opened.round(1, "the first words").await;
  assert!(
    opened.standing.agency.ended().is_empty(),
    "a session with no record of its head is resumed"
  );

  let fresh = Unified::on(StandInSession::new(SESSION, ANSWER).at_generation(0)).await;

  fresh.round(1, "the first words").await;
  assert!(fresh.standing.agency.ended().is_empty());
}

const NOTE: &str = "In the middle of the owner's question about the deployment; promised a reply by noon.";

#[tokio::test]
async fn a_replacement_asks_the_old_session_for_its_handoff_file_and_the_new_session_opens_on_it() {
  let unified =
    Unified::on(StandInSession::new(SESSION, ANSWER).acting(crate::common::session::writing_handoff(NOTE)))
      .await;

  unified.round(1, "the first words").await;
  bmo_receiver::chat::cache::force_reset(
    unified.scratch.db(),
    AGENT,
    receiver_codes::SessionReason::DowngradeForced,
  )
  .await
  .unwrap();
  unified.round(2, "the second words").await;

  let prompts = unified.session.prompts();

  assert_eq!(prompts.len(), 3, "{prompts:?}");

  // The old session was asked, before it was ended, to write its handoff at a file under the data root.
  let path = crate::common::session::handoff_path(&prompts[1]).expect("the handoff names its file");
  let folder = unified.data.path().join("handoffs").join(AGENT);

  assert!(path.starts_with(&folder.display().to_string()), "{path}");
  assert_eq!(std::fs::read_to_string(&path).unwrap(), NOTE);
  assert_eq!(
    std::fs::read_to_string(folder.join("latest.md")).unwrap(),
    NOTE,
    "the newest is copied beside it"
  );
  assert_eq!(unified.standing.agency.ended(), [SESSION]);

  // The new session's first turn opens on the file's path and its whole text, ahead of the round's own words.
  let first = &prompts[2];

  assert!(first.contains(&path) && first.contains(NOTE), "{first}");
  assert!(first.find(NOTE) < first.find("the second words"), "{first}");
  assert!(!first.contains("could not be recovered"), "{first}");
  assert!(unified.lost_notices().await.is_empty());
}

#[tokio::test]
async fn a_session_that_writes_no_handoff_leaves_the_transcript_of_what_it_answered() {
  let unified = Unified::on(StandInSession::new(SESSION, ANSWER)).await;

  unified.round(1, "the first words").await;
  bmo_receiver::chat::cache::force_reset(
    unified.scratch.db(),
    AGENT,
    receiver_codes::SessionReason::DowngradeForced,
  )
  .await
  .unwrap();
  unified.round(2, "the second words").await;

  let prompts = unified.session.prompts();
  let first = prompts.last().unwrap();

  assert_eq!(prompts.len(), 3, "{prompts:?}");
  assert!(first.contains("WHAT YOU ANSWERED BEFORE"), "{first}");
  assert!(first.contains(&format!("{AGENT} (room {OWNER}): {ANSWER}")), "{first}");
  assert!(!first.contains("could not be recovered"), "{first}");
  assert!(unified.lost_notices().await.is_empty());
}

#[tokio::test]
async fn nothing_is_recovered_only_where_neither_a_handoff_nor_a_transcript_stands_and_the_owner_is_told() {
  let unified = Unified::on(StandInSession::new(SESSION, ANSWER)).await;

  bmo_receiver::chat::cache::force_reset(
    unified.scratch.db(),
    AGENT,
    receiver_codes::SessionReason::DowngradeForced,
  )
  .await
  .unwrap();
  unified.round(1, "the first words").await;

  let prompts = unified.session.prompts();
  let first = prompts.last().unwrap();

  assert_eq!(prompts.len(), 2, "the handoff was asked, then the round: {prompts:?}");
  assert!(first.contains("could not be recovered"), "{first}");

  let told = unified.lost_notices().await;

  assert_eq!(told.len(), 1, "{told:?}");
  assert!(told[0].contains("neither its handoff nor a transcript"), "{told:?}");

  // The owner's word to start afresh is spent by the one replacement.
  unified.round(2, "the second words").await;
  assert_eq!(unified.standing.agency.ended(), [SESSION]);
}

#[tokio::test]
async fn a_full_session_handed_over_by_its_note_is_replaced_at_its_next_turn_through_the_same_handoff() {
  let unified =
    Unified::on(StandInSession::new(SESSION, ANSWER).acting(crate::common::session::writing_handoff(NOTE)))
      .await;

  unified.round(1, "the first words").await;
  assert!(bmo_receiver::chat::fold::hand_off(&unified.standing.receiver, AGENT).await);
  unified.round(2, "the second words").await;

  let prompts = unified.session.prompts();

  assert_eq!(prompts.len(), 3, "{prompts:?}");
  assert!(crate::common::session::handoff_path(&prompts[1]).is_some(), "{}", prompts[1]);
  assert!(prompts[2].contains(NOTE), "{}", prompts[2]);
  assert_eq!(unified.standing.agency.ended(), [SESSION]);
}

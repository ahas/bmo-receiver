//! The restart's backlog (docs/messages.md, 「Backlog」): every room the receiver listens to is served one at a
//! time, whatever its road's screen shows, each agent handed the unread lines from the first that wakes it, a line
//! older than the policy's while waking nothing, marked read so a line is never served twice.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::conversation::Timings;
use bmo_receiver::chat::restart::resume_backlog;
use bmo_receiver::chat::runner::Runner;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::rows;
use receiver_policy::chat_line::CHAT_LINE;
use receiver_policy::chat_round::CHAT_ROUND;

use crate::integration::common::Scratch;
use crate::integration::common::session::StandInSession;
use crate::integration::common::units::{
  ADDRESS, AGENT, OWNER, StandInChat, Standing, deployment, registration, unrun, woken,
};

const SESSION: &str = "agent-a@room";
const ROOM: i64 = -500;
const ROOM_B: i64 = -501;
const IDENTITY: i64 = 7;

fn documents(data: &Path) {
  let chat = data.join("prompts").join("chat");

  std::fs::create_dir_all(&chat).unwrap();
  std::fs::write(chat.join(&CHAT_LINE.room.owner_document), "The owner's own chat.").unwrap();
  std::fs::write(chat.join(&CHAT_LINE.room.group_document), "A group room.").unwrap();
  std::fs::write(chat.join(&CHAT_LINE.capabilities.document), "What the agent can do.").unwrap();
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

    crate::integration::common::session::no_stages(scratch.db()).await;

    let data = tempfile::tempdir().unwrap();
    let chat = Arc::new(
      StandInChat::new("road-a")
        .holding(&[ROOM, ROOM_B])
        .agent_in(ROOM, IDENTITY)
        .agent_in(ROOM_B, IDENTITY),
    );

    documents(data.path());

    let standing = woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat.clone()], deployment()));
    // The rooms stand on the console's room list, so the receiver listens to them (docs/wake.md).
    bmo_receiver::receiver::rows::allow_chat(scratch.db(), ROOM).await.unwrap();
    bmo_receiver::receiver::rows::allow_chat(scratch.db(), ROOM_B).await.unwrap();
    let session = Arc::new(session);
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
    // Each room's first launch is paced behind the last on the real clock; both rooms count as launched, so the
    // second room's round is not held behind the first room's.
    receiver.launches().lock().await.rooms.extend([ROOM, ROOM_B]);

    Group {
      scratch,
      _data: data,
      standing,
      session,
      runner,
    }
  }
}

#[tokio::test]
async fn a_restart_serves_each_room_from_the_line_that_wakes_its_agent_once_and_in_order() {
  let round = Group::new(
    StandInSession::new(SESSION, "unscripted")
      .answering(&["here", "there"])
      .hiding_situations(),
  )
  .await;
  let now = bmo_receiver::now_ms();
  let cold = now - (CHAT_ROUND.backlog.within_minutes + 1) * 60_000;

  round.standing.web.watching(&[ROOM_B, ROOM]);
  // Both rooms name the agent and another beside it, since a chat service's group room naming nobody is answered
  // by nobody, and two answerers wait for their calls as a group room's do.
  round.standing.agency.registering(vec![
    registration(AGENT, "Agent A", vec![ADDRESS], None),
    registration("agent-b", "Agent B", vec!["agent-b"], None),
  ]);
  round
    .scratch
    .run(
      r#"INSERT INTO "room_supervisors" ("chat_id", "supervisor")
         VALUES ($1, 'agent-a'), ($2, 'agent-a'), ($1, 'agent-b'), ($2, 'agent-b')"#,
      vec![ROOM.into(), ROOM_B.into()],
    )
    .await;
  round
    .scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via")
         VALUES ($1, 2, $3, 'owner', $4, 'agent-a, a call gone cold', 'road-a'),
                ($1, 3, $3, 'owner', $5, 'agent-a, in the first room', 'road-a'),
                ($2, 2, $3, 'owner', $6, 'chatter before the call', 'road-a'),
                ($2, 3, $3, 'owner', $7, 'agent-a, in the second room', 'road-a'),
                ($2, 4, $3, 'owner', $5, 'and its second line', 'road-a')"#,
      vec![
        ROOM.into(),
        ROOM_B.into(),
        OWNER.into(),
        cold.into(),
        (now - 3_000).into(),
        (now - 5_000).into(),
        (now - 4_000).into(),
      ],
    )
    .await;
  rows::set_last_read(round.scratch.db(), ROOM, 1).await.unwrap();
  rows::set_last_read(round.scratch.db(), ROOM_B, 1).await.unwrap();

  resume_backlog(&round.standing.receiver, &round.runner).await.unwrap();

  let prompts = round.session.prompts();

  assert_eq!(prompts.len(), 2, "{prompts:#?}");
  assert!(prompts[0].contains("in the second room"), "the lower room id is served first");
  assert!(prompts[1].contains("in the first room"));
  // The second room's turn answers the waking line and the one after it, never the chatter before it; the first
  // room's answers its waking line alone, never the call gone cold before the while.
  assert_eq!(round.session.answered(), [vec![3, 4], vec![3]]);

  assert_eq!(rows::last_read(round.scratch.db(), ROOM).await.unwrap(), Some(3));
  assert_eq!(rows::last_read(round.scratch.db(), ROOM_B).await.unwrap(), Some(4));

  // A line already served is not served again.
  resume_backlog(&round.standing.receiver, &round.runner).await.unwrap();

  assert_eq!(round.session.prompts().len(), 2, "the read marks hold every line served");

  // A room the console does not watch is served too: the backlog is every room's, not the open windows'.
  round
    .scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via")
         VALUES ($1, 5, $2, 'owner', $3, 'agent-a, said while the console looked elsewhere', 'road-a')"#,
      vec![ROOM_B.into(), OWNER.into(), now.into()],
    )
    .await;
  round.standing.web.watching(&[ROOM]);

  resume_backlog(&round.standing.receiver, &round.runner).await.unwrap();

  assert_eq!(round.session.prompts().len(), 3, "the unread room is served");
  assert!(round.session.prompts()[2].contains("said while the console looked elsewhere"));
  assert_eq!(rows::last_read(round.scratch.db(), ROOM_B).await.unwrap(), Some(5));

  round.runner.default_answerer().close(ROOM).await;
  round.runner.default_answerer().close(ROOM_B).await;
}

//! The restart's backlog (docs/receiver.md, 「Where it stands」; docs/wake.md, 「The room's queue and the
//! round」): every room with unread lines is served one at a time, each room's unread lines as one turn, marked
//! read so a line is never served twice, and a room with nothing waiting gets no turn. It runs alone in a process of
//! its own (`integration::common::alone`): a room's first launch in a process is spaced from every other room's, so
//! the two fresh rooms it opens would hold back the rounds the other tests of the process wait on.

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

use crate::integration::common::Scratch;
use crate::integration::common::alone::alone;
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

    crate::integration::common::session::no_drafting(scratch.db()).await;

    let data = tempfile::tempdir().unwrap();
    let chat = Arc::new(
      StandInChat::new("road-a")
        .holding(&[ROOM, ROOM_B])
        .showing(ROOM)
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
async fn a_restart_serves_each_room_once_and_in_order() {
  let Some(_run) = alone(module_path!(), "a_restart_serves_each_room_once_and_in_order") else {
    return;
  };

  let round = Group::new(
    StandInSession::new(SESSION, "unscripted")
      .answering(&["here", "there"])
      .hiding_situations(),
  )
  .await;

  round.standing.web.watching(&[ROOM_B, ROOM]);
  round
    .scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via")
         VALUES ($1, 2, $3, 'owner', 1760000002, 'in the first room', 'road-a'),
                ($2, 2, $3, 'owner', 1760000002, 'in the second room', 'road-a'),
                ($2, 3, $3, 'owner', 1760000003, 'and its second line', 'road-a')"#,
      vec![ROOM.into(), ROOM_B.into(), OWNER.into()],
    )
    .await;
  rows::set_last_read(round.scratch.db(), ROOM, 1).await.unwrap();
  rows::set_last_read(round.scratch.db(), ROOM_B, 1).await.unwrap();

  resume_backlog(&round.standing.receiver, &round.runner).await.unwrap();

  let prompts = round.session.prompts();

  assert_eq!(prompts.len(), 2, "{prompts:#?}");
  assert!(prompts[0].contains("in the second room"), "the lower room id is served first");
  assert!(prompts[0].contains("and its second line"));
  assert!(prompts[1].contains("in the first room"));

  assert_eq!(rows::last_read(round.scratch.db(), ROOM).await.unwrap(), Some(2));
  assert_eq!(rows::last_read(round.scratch.db(), ROOM_B).await.unwrap(), Some(3));

  // A line already served is not served again.
  resume_backlog(&round.standing.receiver, &round.runner).await.unwrap();

  assert_eq!(round.session.prompts().len(), 2, "the read marks hold every line served");

  // A room the console does not watch is served too: the backlog is every room's, not the open windows'.
  round
    .scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via")
         VALUES ($1, 4, $2, 'owner', 1760000004, 'said with its window closed', 'road-a')"#,
      vec![ROOM_B.into(), OWNER.into()],
    )
    .await;
  round.standing.web.watching(&[ROOM]);

  resume_backlog(&round.standing.receiver, &round.runner).await.unwrap();

  assert_eq!(round.session.prompts().len(), 3, "the unread room is served");
  assert!(round.session.prompts()[2].contains("said with its window closed"));
  assert_eq!(rows::last_read(round.scratch.db(), ROOM_B).await.unwrap(), Some(4));

  round.runner.default_answerer().close(ROOM).await;
  round.runner.default_answerer().close(ROOM_B).await;
}

//! A session past its cache lifetime is woken only by what someone waits on (docs/unified-bmo.md, 「Replacing the
//! session」; docs/wake.md, 「Quiet instructions」): the touch answers it expired, no housekeeping instruction and no
//! spoken script opens a turn on it, and a room's own words or a room reply open a round that resumes it as it
//! stands; the lifetime alone replaces nothing.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::cache::{CacheState, Route, expired, set_active_route, state_key};
use bmo_receiver::chat::conversation::Timings;
use bmo_receiver::chat::instructions::{Cause, Queued, Rooms, deliver_pending, queue};
use bmo_receiver::chat::runner::Runner;
use bmo_receiver::chat::touch::{TouchOutcome, touch};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::now_ms;
use bmo_receiver::receiver::{Context, Inbound, Room};
use bmo_receiver::store::settings;
use futures_util::future::BoxFuture;
use receiver_codes::SkipReason;
use receiver_policy::chat_line::CHAT_LINE;
use receiver_ports::gate::TurnEvent;
use serde_json::json;

use crate::integration::common::Scratch;
use crate::integration::common::session::{StandInSession, writing_handoff};
use crate::integration::common::units::{
  ADDRESS, AGENT, OWNER, StandInChat, Standing, deployment, registration, unrun, woken,
};

const SESSION: &str = "agent-a@agent";
const NOTE: &str = "what the expired session was in the middle of";

// The agent's one session on Claude Code, its last turn `idle_minutes` ago.
async fn kept(scratch: &Scratch, idle_minutes: i64) {
  let route = Route {
    provider: "claude-code".to_owned(),
    model: "model-a".to_owned(),
    effort: String::new(),
    session: Some(SESSION.to_owned()),
    ..Route::default()
  };
  let state = CacheState {
    at: now_ms() - idle_minutes * 60_000,
    ..CacheState::default()
  };

  set_active_route(scratch.db(), AGENT, &route).await.unwrap();
  settings::set(scratch.db(), &state_key(AGENT, SESSION), &serde_json::to_string(&state).unwrap())
    .await
    .unwrap();
}

fn agent_standing(scratch: &Scratch, data: &std::path::Path, session: &Arc<StandInSession>) -> Standing {
  let chat = Arc::new(StandInChat::new("road-a").owners(OWNER));
  let prompts = data.join("prompts").join("chat");

  std::fs::create_dir_all(&prompts).unwrap();
  std::fs::write(prompts.join(&CHAT_LINE.room.owner_document), "The owner's own chat.").unwrap();
  std::fs::write(prompts.join(&CHAT_LINE.room.group_document), "A group room.").unwrap();
  std::fs::write(prompts.join(&CHAT_LINE.capabilities.document), "What the agent can do.").unwrap();

  let standing = woken(unrun(scratch.db(), DataRoot::new(data), vec![chat], deployment()));
  let mut agent = registration(AGENT, "Agent A", vec![ADDRESS], None);

  agent.row.insert("sessionScope".to_owned(), json!("agent"));
  standing.agency.registering(vec![agent]);
  standing.agency.carrying_on(session.clone());
  standing
}

fn timings() -> Timings {
  Timings {
    queue: Duration::from_millis(1),
    wait: Duration::from_secs(60),
    window: Some(Duration::from_secs(60)),
    look: Duration::from_millis(5),
  }
}

/// Rooms that record each row handed to them and stand no conversation.
#[derive(Default)]
struct Recorded(Mutex<Vec<i64>>);

impl Rooms for Recorded {
  fn instruct(
    &self,
    _agent: &str,
    _chat_id: i64,
    _text: String,
    id: i64,
    _via: Option<String>,
    _events: Vec<TurnEvent>,
  ) -> BoxFuture<'_, ()> {
    self.0.lock().unwrap().push(id);
    Box::pin(async {})
  }

  fn is_active(&self, _agent: &str, _chat_id: i64) -> bool {
    false
  }

  fn standing(&self, _agent: &str) -> bool {
    true
  }

  fn road_rests(&self, _agent: &str) -> BoxFuture<'_, bool> {
    Box::pin(async { false })
  }

  fn busy(&self, _agent: &str) -> bool {
    false
  }
}

#[tokio::test]
async fn an_expired_session_is_not_touched_and_no_housekeeping_wakes_it() {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let session = Arc::new(StandInSession::new(SESSION, "unused"));
  let standing = agent_standing(&scratch, data.path(), &session);
  let receiver = standing.receiver.clone();
  let runner = Runner::new(receiver.clone(), Arc::new(Around::new(receiver.clone())), timings(), AGENT);
  let rooms = Recorded::default();
  let mut last = 0;

  kept(&scratch, 61).await;
  assert!(expired(scratch.db(), AGENT).await, "past its cache lifetime the session is expired");

  let touched = touch(&receiver, &runner.of(AGENT), AGENT, None).await;

  assert_eq!((touched.outcome, touched.code), (TouchOutcome::Skipped, Some(SkipReason::Expired)));
  assert!(session.prompts().is_empty(), "nothing went into the session");
  assert_eq!(
    runner.of(AGENT).spoken(OWNER, "the job is done").await,
    None,
    "no spoken script wakes it"
  );
  assert!(session.prompts().is_empty(), "nothing went into the session for a script either");

  let housekeeping = Queued {
    cause: Cause::Housekeeping,
    ..Queued::default()
  };
  let id = queue(scratch.db(), AGENT, OWNER, "a record for the agent alone", &housekeeping)
    .await
    .unwrap();
  let only = queue(
    scratch.db(),
    AGENT,
    OWNER,
    "a tool list's change",
    &Queued {
      agent_only: true,
      ..Queued::default()
    },
  )
  .await
  .unwrap();

  deliver_pending(&receiver, &rooms, &mut last, now_ms(), 0).await.unwrap();
  assert!(
    rooms.0.lock().unwrap().is_empty(),
    "housekeeping waits for a round something else opens"
  );

  // Inside its lifetime the same row is handed over, and the session is not expired, past its touch's due time.
  kept(&scratch, 45).await;
  assert!(!expired(scratch.db(), AGENT).await);
  deliver_pending(&receiver, &rooms, &mut last, now_ms(), 0).await.unwrap();
  assert_eq!(*rooms.0.lock().unwrap(), [id, only]);
}

#[tokio::test]
async fn a_room_reply_landing_on_an_expired_session_is_handed_at_once() {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let session = Arc::new(StandInSession::new(SESSION, "unused"));
  let standing = agent_standing(&scratch, data.path(), &session);
  let rooms = Recorded::default();
  let mut last = 0;

  kept(&scratch, 600).await;
  assert!(expired(scratch.db(), AGENT).await);

  let id = queue(
    scratch.db(),
    AGENT,
    OWNER,
    "the research run asked in this room has ended",
    &Queued {
      kind: Some("research-ended:218"),
      sort: Some("notice"),
      ..Queued::default()
    },
  )
  .await
  .unwrap();

  deliver_pending(&standing.receiver, &rooms, &mut last, now_ms(), 0).await.unwrap();
  assert_eq!(*rooms.0.lock().unwrap(), [id], "nobody's words are waited for");
}

#[tokio::test]
async fn a_room_reply_on_an_expired_session_starts_a_turn_that_resumes_it() {
  let scratch = Scratch::new().await;

  crate::integration::common::session::no_drafting(scratch.db()).await;

  let data = tempfile::tempdir().unwrap();
  let session = Arc::new(
    StandInSession::new(SESSION, "The research is back.")
      .on_provider("claude-code")
      .at_generation(2)
      .acting(writing_handoff(NOTE)),
  );
  let standing = agent_standing(&scratch, data.path(), &session);
  let receiver = standing.receiver.clone();
  let runner = Runner::new(receiver.clone(), Arc::new(Around::new(receiver.clone())), timings(), AGENT);
  let mut last = 0;

  receiver.hold_runner(Arc::new(runner.clone()));
  kept(&scratch, 600).await;

  let id = queue(
    scratch.db(),
    AGENT,
    OWNER,
    "the research run asked in this room has ended",
    &Queued {
      sort: Some("notice"),
      ..Queued::default()
    },
  )
  .await
  .unwrap();

  deliver_pending(&receiver, &runner, &mut last, now_ms(), 0).await.unwrap();

  let waited = tokio::time::timeout(Duration::from_secs(20), async {
    while session.prompts().is_empty() || !runner.idle() || session.working() {
      tokio::time::sleep(Duration::from_millis(5)).await;
    }
  })
  .await;

  assert!(waited.is_ok(), "the reply's round never ended: {:#?}", session.prompts());

  let prompts = session.prompts();

  assert_eq!(prompts.len(), 1, "the expired session takes the turn as it stands: {prompts:#?}");
  assert!(
    !prompts[0].contains("/handoffs/"),
    "no handoff is asked for the lifetime: {}",
    prompts[0]
  );
  assert!(prompts[0].contains("the research run asked in this room has ended"), "{}", prompts[0]);
  assert!(standing.agency.ended().is_empty(), "the session is not replaced for its lifetime");

  let settled = tokio::time::timeout(Duration::from_secs(5), async {
    let sql = r#"SELECT count(*) FROM "instructions" WHERE "id" = $1 AND "delivered_at" IS NOT NULL"#;

    while scratch.count(sql, vec![id.into()]).await == 0 {
      tokio::time::sleep(Duration::from_millis(5)).await;
    }
  })
  .await;

  assert!(settled.is_ok(), "the reply's row is settled by the round that took it");
}

#[tokio::test]
async fn a_real_message_after_expiry_resumes_the_session_as_it_stands() {
  let scratch = Scratch::new().await;

  crate::integration::common::session::no_drafting(scratch.db()).await;

  let data = tempfile::tempdir().unwrap();
  let session = Arc::new(
    StandInSession::new(SESSION, "Here.")
      .on_provider("claude-code")
      .at_generation(2)
      .acting(writing_handoff(NOTE)),
  );
  let standing = agent_standing(&scratch, data.path(), &session);
  let receiver = standing.receiver.clone();
  let runner = Runner::new(receiver.clone(), Arc::new(Around::new(receiver.clone())), timings(), AGENT);

  receiver.hold_runner(Arc::new(runner.clone()));
  kept(&scratch, 61).await;

  let message = Inbound {
    chat_id: OWNER,
    message_id: 6,
    sender_id: Some(OWNER),
    sender_name: "tester".into(),
    sent_at: 1_760_000_000_100,
    text: "are you there".into(),
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
  };
  let room = Room {
    owner_room: true,
    identity: Some(7),
    private_to: None,
    sealed: false,
  };

  receiver.message(&message, &Context::default(), room).await.unwrap();

  let waited = tokio::time::timeout(Duration::from_secs(20), async {
    while session.prompts().is_empty() || !runner.idle() || session.working() {
      tokio::time::sleep(Duration::from_millis(5)).await;
    }
  })
  .await;

  assert!(waited.is_ok(), "the round never ended: {:#?}", session.prompts());

  let prompts = session.prompts();

  assert_eq!(prompts.len(), 1, "the expired session takes the turn as it stands: {prompts:#?}");
  assert!(
    !prompts[0].contains("/handoffs/"),
    "no handoff is asked for the lifetime: {}",
    prompts[0]
  );
  assert!(prompts[0].contains("are you there"), "{}", prompts[0]);
  assert!(standing.agency.ended().is_empty(), "the session is not replaced for its lifetime");
}

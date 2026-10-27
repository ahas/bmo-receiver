//! A handoff asked for ahead of a fresh session (docs/api.md, 「Handoffs」; docs/unified-bmo.md, 「Replacing the
//! session」): the route has the agent's standing session write its handoff file and keeps it, a second asking while
//! one runs is refused, an agent with no opened session writes nothing, and the next fresh session's first turn
//! opens on the file, then the room's words, then the brain.

use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::conversation::Timings;
use bmo_receiver::chat::handoff::{Asking, pending_key};
use bmo_receiver::chat::runner::Runner;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::{Context, Inbound, Room};
use bmo_receiver::store::settings;
use receiver_policy::chat_line::CHAT_LINE;
use receiver_policy::chat_round::CHAT_ROUND;
use serde_json::json;

use crate::common::Scratch;
use crate::common::queue::{post, stood};
use crate::common::session::{StandInSession, writing_handoff};
use crate::common::units::{ADDRESS, AGENT, OWNER, StandInChat, deployment, registration, unrun, woken};

const NOTE: &str = "what the closing session was in the middle of";

#[tokio::test]
async fn the_route_has_the_standing_session_write_its_handoff_keeps_it_and_refuses_a_second_asking() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;
  let session = Arc::new(
    StandInSession::new("agent-a@agent", "unused")
      .at_generation(3)
      .acting(writing_handoff(NOTE)),
  );
  let mut agent = registration(AGENT, "Agent A", vec![ADDRESS], None, true);

  agent.row.insert("sessionScope".to_owned(), json!("agent"));
  front.standing.agency.registering(vec![agent]);
  front.standing.agency.carrying_on(session.clone());

  let (status, body) = post(&front, "/handoffs/agent-a", json!({ "reason": "the agency restarted" })).await;

  assert_eq!(status, StatusCode::OK, "{body}");
  assert_eq!(body["bytes"], NOTE.len());

  let path = body["path"].as_str().unwrap().to_owned();

  assert_eq!(std::fs::read_to_string(&path).unwrap(), NOTE);
  assert_eq!(settings::get(scratch.db(), &pending_key(AGENT)).await.unwrap(), Some(path));

  // A handoff of the same agent under way refuses the second asking.
  let under_way = Asking::begin(AGENT).unwrap();
  let (status, body) = post(&front, "/handoffs/agent-a", json!({ "reason": "again" })).await;

  assert_eq!((status, body["code"].as_str()), (StatusCode::CONFLICT, Some("conflict")));
  drop(under_way);

  let (status, _) = post(&front, "/handoffs/agent-a", json!({ "reason": "" })).await;

  assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn an_agent_whose_session_never_opened_has_nothing_to_hand_over() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;
  let session = Arc::new(StandInSession::new("agent-c@agent", "unused").acting(writing_handoff(NOTE)));
  let mut agent = registration("agent-c", "Agent X", vec![ADDRESS], None, true);

  agent.row.insert("sessionScope".to_owned(), json!("agent"));
  front.standing.agency.registering(vec![agent]);
  front.standing.agency.carrying_on(session.clone());
  session.now_at(None);

  let (status, _) = post(&front, "/handoffs/agent-c", json!({ "reason": "a program stood anew" })).await;

  assert_eq!(status, StatusCode::NO_CONTENT);
  assert!(session.prompts().is_empty(), "no session was asked");
}

#[tokio::test]
async fn a_handoff_asked_again_within_the_span_does_not_reach_the_session_and_answers_the_one_kept() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;
  let session = Arc::new(
    StandInSession::new("agent-b@agent", "unused")
      .at_generation(3)
      .acting(writing_handoff(NOTE)),
  );
  let mut agent = registration("agent-b", "Agent X", vec![ADDRESS], None, true);

  agent.row.insert("sessionScope".to_owned(), json!("agent"));
  front.standing.agency.registering(vec![agent]);
  front.standing.agency.carrying_on(session.clone());

  let (status, first) = post(&front, "/handoffs/agent-b", json!({ "reason": "a fresh session" })).await;

  assert_eq!(status, StatusCode::OK, "{first}");

  let asked = session.prompts().len();

  // The road's program exits after each answer and the agency asks again, and again: the session is asked once.
  for _ in 0..3 {
    let (status, again) = post(&front, "/handoffs/agent-b", json!({ "reason": "a fresh session" })).await;

    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again["path"], first["path"], "the one kept answers");
  }

  assert_eq!(session.prompts().len(), asked, "the standing session was asked once");
}

fn owners_room() -> Room {
  Room {
    owner_room: true,
    identity: Some(7),
    private_to: None,
  }
}

#[tokio::test]
async fn the_next_fresh_sessions_first_turn_opens_on_the_kept_handoff_then_the_rooms_words_then_the_brain()
{
  const MISSED: &str = "words said while the handoff was under way";

  let scratch = Scratch::new().await;

  crate::common::session::no_drafting(scratch.db()).await;

  let data = tempfile::tempdir().unwrap();
  let chat = Arc::new(StandInChat::new("road-a").owners(OWNER));
  let prompts = data.path().join("prompts").join("chat");

  std::fs::create_dir_all(&prompts).unwrap();
  std::fs::write(prompts.join(&CHAT_LINE.room.owner_document), "The owner's own chat.").unwrap();
  std::fs::write(prompts.join(&CHAT_LINE.room.group_document), "A group room.").unwrap();
  std::fs::write(prompts.join(&CHAT_LINE.capabilities.document), "What the agent can do.").unwrap();

  let standing = woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat], deployment()));
  // The fresh session the agency opened: its state says it was never opened.
  let session = Arc::new(StandInSession::new("agent-a@agent", "Here."));
  let receiver = standing.receiver.clone();
  let timings = Timings {
    queue: Duration::from_millis(1),
    wait: Duration::from_secs(60),
    window: Some(Duration::from_secs(60)),
    look: Duration::from_millis(5),
  };
  let runner = Runner::new(receiver.clone(), Arc::new(Around::new(receiver.clone())), timings, AGENT);
  let mut agent = registration(AGENT, "Agent A", vec![ADDRESS], None, true);
  let kept = data.path().join("kept-handoff.md");

  agent.row.insert("sessionScope".to_owned(), json!("agent"));
  standing.agency.registering(vec![agent]);
  standing.agency.carrying_on(session.clone());
  receiver.hold_runner(Arc::new(runner.clone()));
  std::fs::write(&kept, NOTE).unwrap();
  settings::set(scratch.db(), &pending_key(AGENT), &kept.to_string_lossy()).await.unwrap();
  scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via")
         VALUES ($1, 5, $1, 'tester', 1760000000000, $2, 'road-a')"#,
      vec![OWNER.into(), MISSED.into()],
    )
    .await;

  let message = Inbound {
    chat_id: OWNER,
    message_id: 6,
    sender_id: Some(OWNER),
    sender_name: "tester".into(),
    sent_at: 1_760_000_000_100,
    text: "what is new".into(),
    image_path: None,
    transcript: None,
    voice_seconds: None,
    voice_state: None,
    reply_to_message_id: None,
    via: "road-a".into(),
    mode: Some("bot-account".into()),
    via_reader: None,
    album: None,
  };

  receiver.message(&message, &Context::default(), owners_room()).await.unwrap();

  let waited = tokio::time::timeout(Duration::from_secs(20), async {
    while session.prompts().is_empty() || !runner.idle() || session.working() {
      tokio::time::sleep(Duration::from_millis(5)).await;
    }
  })
  .await;

  assert!(waited.is_ok(), "the round never ended");

  let prompt = &session.prompts()[0];
  let note = prompt.find(NOTE).unwrap_or_else(|| panic!("the handoff is not carried: {prompt}"));
  let missed = prompt
    .find(MISSED)
    .unwrap_or_else(|| panic!("the words said meanwhile are not carried: {prompt}"));
  let brain = prompt
    .find(&CHAT_ROUND.index.walk_head)
    .unwrap_or_else(|| panic!("the brain is not walked: {prompt}"));

  assert!(
    note < missed && missed < brain,
    "the handoff, then the room's words, then the brain: {prompt}"
  );
  assert_eq!(
    settings::get(scratch.db(), &pending_key(AGENT)).await.unwrap(),
    None,
    "the handoff is taken once"
  );
}

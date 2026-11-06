//! A handoff asked for ahead of a fresh session (docs/api.md, 「Handoffs」; docs/unified-bmo.md, 「Replacing the
//! session」): the route has the agent's standing session write its handoff file and keeps it, a second asking while
//! one runs waits in line for it, an agent with no opened session writes nothing, and the next fresh session's first turn
//! opens on the file, then the room's words, then the brain. The `handoff` verb asks the same route of a receiver
//! standing on a port, as the running service does, and a supervisor no handoff was asked for keeps its session.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::conversation::Timings;
use bmo_receiver::chat::handoff::{Asking, pending_key};
use bmo_receiver::chat::runner::Runner;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::{Context, Inbound, Room};
use bmo_receiver::routes::routes;
use bmo_receiver::store::settings;
use receiver_policy::chat_line::CHAT_LINE;
use receiver_policy::chat_round::CHAT_ROUND;
use serde_json::json;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{TOKEN, post, stood};
use crate::integration::common::session::{StandInSession, prompt_documents, writing_handoff};
use crate::integration::common::units::{
  ADDRESS, AGENT, OWNER, StandInChat, deployment, registration, unrun, woken,
};

const NOTE: &str = "what the closing session was in the middle of";

#[tokio::test]
async fn the_route_has_the_standing_session_write_its_handoff_keeps_it_and_queues_a_second_asking() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  prompt_documents(front.data.path());
  let session = Arc::new(
    StandInSession::new("agent-a@agent", "unused")
      .at_generation(3)
      .acting(writing_handoff(NOTE)),
  );
  let mut agent = registration(AGENT, "Agent A", vec![ADDRESS], None);

  agent.row.insert("sessionScope".to_owned(), json!("agent"));
  front.standing.agency.registering(vec![agent]);
  front.standing.agency.carrying_on(session.clone());

  let (status, body) = post(&front, "/handoffs/agent-a", json!({ "reason": "the agency restarted" })).await;

  assert_eq!(status, StatusCode::OK, "{body}");
  assert_eq!(body["bytes"], NOTE.len());

  let path = body["path"].as_str().unwrap().to_owned();

  assert_eq!(std::fs::read_to_string(&path).unwrap(), NOTE);
  assert_eq!(settings::get(scratch.db(), &pending_key(AGENT)).await.unwrap(), Some(path));
  // The handoff turn answers the owner's private chat where the asking names no room, carrying its block.
  assert_eq!(session.rooms(), [Some(OWNER)]);

  // A handoff of the same agent under way holds the second asking in line until it ends, which is then answered
  // by the handoff kept.
  let under_way = Asking::begin(AGENT).await;
  let again = post(&front, "/handoffs/agent-a", json!({ "reason": "again" }));

  tokio::pin!(again);
  assert!(
    tokio::time::timeout(Duration::from_millis(200), &mut again).await.is_err(),
    "it waits in line"
  );
  drop(under_way);

  let (status, body) = again.await;

  assert_eq!(status, StatusCode::OK, "{body}");
  assert_eq!(body["bytes"], NOTE.len());

  let (status, _) = post(&front, "/handoffs/agent-a", json!({ "reason": "" })).await;

  assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn an_agent_whose_session_never_opened_has_nothing_to_hand_over() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  prompt_documents(front.data.path());
  let session = Arc::new(StandInSession::new("agent-c@agent", "unused").acting(writing_handoff(NOTE)));
  let mut agent = registration("agent-c", "Agent X", vec![ADDRESS], None);

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

  prompt_documents(front.data.path());
  let session = Arc::new(
    StandInSession::new("agent-b@agent", "unused")
      .at_generation(3)
      .acting(writing_handoff(NOTE)),
  );
  let mut agent = registration("agent-b", "Agent X", vec![ADDRESS], None);

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

  crate::integration::common::session::no_drafting(scratch.db()).await;

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
  let mut agent = registration(AGENT, "Agent A", vec![ADDRESS], None);
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
  let brain = prompt
    .find(&CHAT_ROUND.index.hint)
    .unwrap_or_else(|| panic!("the brain's index is not in the room's block: {prompt}"));
  let missed = prompt
    .find(MISSED)
    .unwrap_or_else(|| panic!("the words said meanwhile are not carried: {prompt}"));

  assert!(
    note < brain && brain < missed,
    "the handoff, then the room's block with the brain's index, then the words: {prompt}"
  );
  assert!(!prompt.contains(&CHAT_ROUND.index.walk_head), "no walk rides the turn: {prompt}");
  assert_eq!(
    settings::get(scratch.db(), &pending_key(AGENT)).await.unwrap(),
    None,
    "the handoff is taken once"
  );
}

// The round's timings cut to what the tests can wait on: the queue handing over at once, the working turn looked
// at often, and no window or wait running out while a test stands.
fn timings() -> Timings {
  Timings {
    queue: Duration::from_millis(1),
    wait: Duration::from_secs(60),
    window: Some(Duration::from_secs(60)),
    look: Duration::from_millis(5),
  }
}

// The configuration the verb reads, naming the receiver stood on `listen` and neutral units it never reaches: a
// handoff asks the receiver alone.
fn settings_file(listen: &str, database: &str, data_root: &Path) -> String {
  format!(
    r#"listen = "{listen}"
token = "{TOKEN}"
database = "{database}"
data_root = "{}"
owner = {OWNER}
agent = "agent-a"
sender_name = "Agent A"
language = "en"

[agency]
base = "http://127.0.0.1:1"
token = "unused"

[gate]
base = "http://127.0.0.1:1"
token = "unused"

[job]
base = "http://127.0.0.1:1"
token = "unused"

[web]
base = "http://127.0.0.1:1"
token = "unused"

[warmer]
base = "http://127.0.0.1:1"
token = "unused"
"#,
    data_root.display()
  )
}

// The receiver's routes standing on a loopback port, as the running service stands; the address it answers at.
async fn listening(app: axum::Router) -> String {
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = listener.local_addr().unwrap().to_string();

  tokio::spawn(async move {
    let _ = axum::serve(listener, app).await;
  });

  address
}

// Waits, bounded, until the session has carried `wanted` turns and the round that carried them has ended.
async fn until_turns(runner: &Runner, session: &StandInSession, wanted: usize) {
  let waited = tokio::time::timeout(Duration::from_secs(20), async {
    while session.prompts().len() < wanted || !runner.idle() || session.working() {
      tokio::time::sleep(Duration::from_millis(5)).await;
    }
  })
  .await;

  assert!(waited.is_ok(), "the round never ended");
}

// The room's message the asking must leave as it stands.
async fn kept_message(scratch: &Scratch) -> Vec<String> {
  scratch
    .texts(&format!(
      r#"SELECT "text" FROM "messages" WHERE "chat_id" = {OWNER} AND "message_id" = 5"#
    ))
    .await
}

fn worded(message_id: i64, text: &str) -> Inbound {
  Inbound {
    chat_id: OWNER,
    message_id,
    sender_id: Some(OWNER),
    sender_name: "tester".into(),
    sent_at: 1_760_000_000_000 + message_id,
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

// A receiver whose default answerer keeps one session across its rooms, stood with documents, a chat service
// holding the owner's chat, the agent registered, the session given, and a runner ready for its rounds.
async fn stood_handing(
  session: Arc<StandInSession>,
) -> (Scratch, tempfile::TempDir, crate::integration::common::units::Standing, Runner) {
  let scratch = Scratch::new().await;

  crate::integration::common::session::no_drafting(scratch.db()).await;

  let data = tempfile::tempdir().unwrap();
  let chat = Arc::new(StandInChat::new("road-a").owners(OWNER));

  prompt_documents(data.path());

  let standing = woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat], deployment()));
  let receiver = standing.receiver.clone();
  let runner = Runner::new(receiver.clone(), Arc::new(Around::new(receiver.clone())), timings(), AGENT);
  let mut agent = registration(AGENT, "Agent A", vec![ADDRESS], None);

  agent.row.insert("sessionScope".to_owned(), json!("agent"));
  standing.agency.registering(vec![agent]);
  standing.agency.carrying_on(session.clone());
  receiver.hold_runner(Arc::new(runner.clone()));

  (scratch, data, standing, runner)
}

#[tokio::test]
async fn the_handoff_verb_asks_a_running_receiver_and_the_next_turn_opens_a_fresh_generation_on_the_note() {
  const NOTE: &str = "what the standing session was in the middle of";
  const KEPT: &str = "a message the room held before the asking";

  let session = Arc::new(
    StandInSession::new("agent-a@standing", "unused")
      .at_generation(3)
      .acting(writing_handoff(NOTE)),
  );
  let (scratch, data, standing, runner) = stood_handing(session.clone()).await;
  let receiver = standing.receiver.clone();

  // A message and a run's line the room held before the asking, which the handoff leaves as they stand.
  scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via")
         VALUES ($1, 5, $1, 'tester', 1760000000000, $2, 'road-a')"#,
      vec![OWNER.into(), KEPT.into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "run_records" ("run", "kind", "text", "at") VALUES ('chat:42:1', 'note', 'kept', 1)"#,
      vec![],
    )
    .await;

  // The receiver stands on a port, as the running service does, and the verb is run against it.
  let address = listening(routes(receiver.clone(), TOKEN)).await;
  let folder = tempfile::tempdir().unwrap();
  let config = folder.path().join("settings.toml");
  let room = OWNER.to_string();

  std::fs::write(&config, settings_file(&address, &scratch.address(), data.path())).unwrap();

  let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_bmo-receiver"))
    .arg("--config")
    .arg(&config)
    .args(["handoff", AGENT, "--reason", "the owner asked for a fresh session", "--room", &room])
    .output()
    .await
    .unwrap();
  let said = String::from_utf8_lossy(&output.stdout).into_owned();
  let refused = String::from_utf8_lossy(&output.stderr).into_owned();

  assert!(output.status.success(), "the verb refused: {refused}");
  assert!(said.contains("asked (the owner asked for a fresh session)"), "{said}");

  let kept = settings::get(scratch.db(), &pending_key(AGENT))
    .await
    .unwrap()
    .expect("the handoff is kept for the next fresh session");

  assert_eq!(std::fs::read_to_string(&kept).unwrap(), NOTE);
  assert!(said.contains(&kept), "the verb prints the file it wrote: {said}");
  assert_eq!(session.rooms(), [Some(OWNER)], "the turn answers the room the asking named");

  // A refusal is the route's own sentence, and the verb ends with 1.
  let refused = tokio::process::Command::new(env!("CARGO_BIN_EXE_bmo-receiver"))
    .arg("--config")
    .arg(&config)
    .args(["handoff", AGENT, "--reason", ""])
    .output()
    .await
    .unwrap();

  assert_eq!(refused.status.code(), Some(1), "an empty reason is refused");
  assert!(
    String::from_utf8_lossy(&refused.stderr).contains("reason (non-empty string) is required"),
    "the route's sentence is printed: {}",
    String::from_utf8_lossy(&refused.stderr)
  );

  // The room's own records stand as they did: the handoff writes its note and changes neither.
  assert_eq!(kept_message(&scratch).await, [KEPT.to_owned()]);
  assert_eq!(scratch.count(r#"SELECT count(*) FROM "run_records""#, vec![]).await, 1);

  // The agency opens the fresh session after the handoff, and its first turn opens on the kept note.
  let fresh = Arc::new(StandInSession::new("agent-a@fresh", "Here."));

  standing.agency.carrying_on(fresh.clone());

  let new_words = "what is new";

  receiver
    .message(&worded(6, new_words), &Context::default(), owners_room())
    .await
    .unwrap();
  until_turns(&runner, &fresh, 1).await;

  let prompt = &fresh.prompts()[0];
  let note = prompt.find(NOTE).unwrap_or_else(|| panic!("the handoff is not carried: {prompt}"));
  let brain = prompt
    .find(&CHAT_ROUND.index.hint)
    .unwrap_or_else(|| panic!("the brain's index is not in the room's block: {prompt}"));
  let words = prompt
    .find(new_words)
    .unwrap_or_else(|| panic!("the room's words are not carried: {prompt}"));

  assert!(
    note < brain && brain < words,
    "the handoff, then the room's block with the brain's index, then the words: {prompt}"
  );
  assert_eq!(
    settings::get(scratch.db(), &pending_key(AGENT)).await.unwrap(),
    None,
    "the handoff is taken once"
  );
  assert_eq!(kept_message(&scratch).await, [KEPT.to_owned()], "the room keeps its messages");
  assert_eq!(
    scratch.count(r#"SELECT count(*) FROM "run_records""#, vec![]).await,
    1,
    "the room keeps its runs"
  );
  assert_eq!(
    standing.agency.ended(),
    Vec::<String>::new(),
    "the fresh session was opened, not replaced"
  );
}

#[tokio::test]
async fn a_supervisor_no_handoff_was_asked_for_keeps_the_session_its_next_turn_resumes() {
  let session = Arc::new(StandInSession::new("agent-a@standing", "Here."));
  let (scratch, _data, standing, runner) = stood_handing(session.clone()).await;
  let receiver = standing.receiver.clone();

  receiver
    .message(&worded(1, "the first words"), &Context::default(), owners_room())
    .await
    .unwrap();
  until_turns(&runner, &session, 1).await;
  receiver
    .message(&worded(2, "the next words"), &Context::default(), owners_room())
    .await
    .unwrap();
  until_turns(&runner, &session, 2).await;

  assert_eq!(standing.agency.ended(), Vec::<String>::new(), "the session was not replaced");
  assert!(
    session.turns.lock().unwrap().iter().all(|turn| turn.session == "agent-a@standing"),
    "every turn went on the standing session"
  );
  assert_eq!(
    settings::get(scratch.db(), &pending_key(AGENT)).await.unwrap(),
    None,
    "no handoff was asked for or kept"
  );
}

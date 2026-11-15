//! A handover asked for an agent's one session (docs/api.md, 「Handovers」; docs/unified-bmo.md, 「Replacing the
//! session」): the route writes the word to start the session afresh with the reason `asked` and runs no turn, an
//! empty reason, an agent the agency does not register and one that keeps a session in each room are refused, and
//! the `handoff` verb asks the same route of a receiver standing on a port, after which the agent's next turn asks
//! the old session for its handoff, ends it and opens the new one on the note.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use bmo_receiver::chat::around::Around;
use bmo_receiver::chat::cache::force_key;
use bmo_receiver::chat::conversation::Timings;
use bmo_receiver::chat::runner::Runner;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::{Context, Inbound, Room};
use bmo_receiver::routes::routes;
use bmo_receiver::store::settings;
use serde_json::json;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{TOKEN, post, stood};
use crate::integration::common::session::{
  StandInSession, handoff_path, prompt_documents, writing_handoff,
};
use crate::integration::common::units::{
  ADDRESS, AGENT, OWNER, StandInChat, Standing, deployment, registration, unrun, woken,
};

const NOTE: &str = "what the standing session was in the middle of";

#[tokio::test]
async fn the_route_writes_the_word_with_the_reason_asked_and_runs_no_turn() {
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
  front
    .standing
    .agency
    .registering(vec![agent, registration("agent-b", "Agent B", vec![ADDRESS], None)]);
  front.standing.agency.carrying_on(session.clone());

  let (status, body) =
    post(&front, "/handovers/agent-a", json!({ "reason": "the agent asked for it" })).await;

  assert_eq!(status, StatusCode::ACCEPTED, "{body}");
  assert_eq!(body, json!({ "agentName": AGENT, "sessionReason": "asked" }));
  assert_eq!(settings::get(scratch.db(), &force_key(AGENT)).await.unwrap().as_deref(), Some("asked"));
  assert!(session.prompts().is_empty(), "no turn was run");
  assert!(front.standing.agency.ended().is_empty(), "no session was ended");

  let (status, body) = post(&front, "/handovers/agent-a", json!({ "reason": "  " })).await;

  assert_eq!(
    (status, body["code"].as_str()),
    (StatusCode::BAD_REQUEST, Some("bad-request")),
    "{body}"
  );

  let (status, body) = post(&front, "/handovers/agent-z", json!({ "reason": "why" })).await;

  assert_eq!((status, body["code"].as_str()), (StatusCode::NOT_FOUND, Some("not-found")), "{body}");

  let (status, body) = post(&front, "/handovers/agent-b", json!({ "reason": "why" })).await;

  assert_eq!((status, body["code"].as_str()), (StatusCode::CONFLICT, Some("conflict")), "{body}");
  assert_eq!(settings::get(scratch.db(), &force_key("agent-b")).await.unwrap(), None);
}

fn owners_room() -> Room {
  Room {
    owner_room: true,
    identity: Some(7),
    private_to: None,
    sealed: false,
  }
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
// handover asks the receiver alone.
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
base = "http://[::1]:1"
token = "unused"

[gate]
base = "http://[::1]:1"
token = "unused"

[job]
base = "http://[::1]:1"
token = "unused"

[web]
base = "http://[::1]:1"
token = "unused"

[warmer]
base = "http://[::1]:1"
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

// A receiver whose default answerer keeps one session across its rooms, stood with documents, a chat service
// holding the owner's chat, the agent registered, the session given, and a runner ready for its rounds.
async fn stood_handing(session: Arc<StandInSession>) -> (Scratch, tempfile::TempDir, Standing, Runner) {
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
async fn the_handoff_verb_asks_a_handover_and_a_quiet_agents_session_is_replaced_at_once_on_its_note() {
  let session = Arc::new(StandInSession::new("agent-a@standing", "Here.").acting(writing_handoff(NOTE)));
  let (scratch, data, standing, runner) = stood_handing(session.clone()).await;
  let receiver = standing.receiver.clone();

  receiver.clone().hold_touching(Arc::new(runner.clone()));

  receiver
    .message(&worded(1, "the first words"), &Context::default(), owners_room())
    .await
    .unwrap();
  until_turns(&runner, &session, 1).await;
  assert!(standing.agency.ended().is_empty(), "the first round resumed the session");

  let before = session.prompts().len();

  // The receiver stands on a port, as the running service does, and the verb is run against it.
  let address = listening(routes(receiver.clone(), TOKEN)).await;
  let folder = tempfile::tempdir().unwrap();
  let config = folder.path().join("settings.toml");

  std::fs::write(&config, settings_file(&address, &scratch.address(), data.path())).unwrap();

  let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_bmo-receiver"))
    .arg("--config")
    .arg(&config)
    .args(["handoff", AGENT, "--reason", "the session is to start afresh"])
    .output()
    .await
    .unwrap();
  let said = String::from_utf8_lossy(&output.stdout).into_owned();
  let refused = String::from_utf8_lossy(&output.stderr).into_owned();

  assert!(output.status.success(), "the verb refused: {refused}");
  assert_eq!(
    said.trim_end(),
    format!(
      "handoff {AGENT}: handover asked (the session is to start afresh); it happens once the agent is quiet"
    ),
  );

  // The agent is in no round, so the session is replaced behind the verb's answer: asked for its handoff and ended.
  until_turns(&runner, &session, before + 1).await;
  assert!(handoff_path(&session.prompts()[before]).is_some(), "{}", session.prompts()[before]);
  assert_eq!(standing.agency.ended(), ["agent-a@standing"]);

  // A refusal is the route's own sentence, and the verb ends with 1.
  let empty = tokio::process::Command::new(env!("CARGO_BIN_EXE_bmo-receiver"))
    .arg("--config")
    .arg(&config)
    .args(["handoff", AGENT, "--reason", ""])
    .output()
    .await
    .unwrap();

  assert_eq!(empty.status.code(), Some(1), "an empty reason is refused");
  assert!(
    String::from_utf8_lossy(&empty.stderr).contains("reason (non-empty string) is required"),
    "the route's sentence is printed: {}",
    String::from_utf8_lossy(&empty.stderr)
  );

  // The next turn opens the new session on the note.
  receiver
    .message(&worded(2, "the second words"), &Context::default(), owners_room())
    .await
    .unwrap();
  until_turns(&runner, &session, before + 2).await;

  let prompts = session.prompts();

  assert_eq!(prompts.len(), before + 2, "the handoff asked, then the next round: {prompts:?}");
  assert!(
    prompts[before + 1].contains(NOTE),
    "the new session opens on the note: {}",
    prompts[before + 1]
  );
  assert_eq!(standing.agency.ended(), ["agent-a@standing"]);
  assert_eq!(
    settings::get(scratch.db(), &force_key(AGENT)).await.unwrap(),
    None,
    "the word is spent by the one replacement"
  );
}

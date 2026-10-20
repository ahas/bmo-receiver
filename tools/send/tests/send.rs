//! The send tool as the tool host calls it (docs/chat-mcp.md, 「How a call runs」): the manifest held to the
//! executable and every entry's schema file, each entry asked of the route it goes out on with the turn it is
//! made in, a turn its agent's reset overtook refused before anything is asked, and the executable answering one
//! request on its standard streams.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::Uri;
use serde_json::{Value, json};
use tempfile::tempdir;

use receiver_ports::brain::{Brain, HttpBrain, Refusal};
use room_calls::sends::{
  ConsultInput, PhotoInput, PinInput, RandomDrawInput, ReactInput, RemindInput, SayInput, StickerInput,
};
use room_calls::turn::{NO_AGENT, NO_ROOM};
use send_tool::{ENTRIES, call};
use tool_call::{Session, differences, input_schema, peers};

const ROOM: i64 = -4_200;
const TOKEN: &str = "quiet";

// The schema file an entry names, as it stands beside the manifest.
fn schema_of(file: &str) -> Value {
  let at = Path::new(env!("CARGO_MANIFEST_DIR")).join(file);
  let read = std::fs::read_to_string(&at).unwrap_or_else(|why| panic!("{file}: {why}"));

  serde_json::from_str(&read).unwrap_or_else(|why| panic!("{file} does not read: {why}"))
}

// A unit stood in for on a loopback port: every request kept in order, each route answered as the test set it.
#[derive(Clone, Default)]
struct Book {
  taken: Arc<Mutex<Vec<(String, Value)>>>,
  answers: Arc<Mutex<BTreeMap<String, Value>>>,
}

async fn stood(book: Book) -> (String, u16) {
  async fn answered(State(book): State<Book>, uri: Uri, body: Bytes) -> Json<Value> {
    let path = uri.path().to_owned();
    let said: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);

    book.taken.lock().unwrap().push((path.clone(), said));
    Json(
      book
        .answers
        .lock()
        .unwrap()
        .get(&path)
        .cloned()
        .unwrap_or(json!({ "lines": ["sent"], "answered": true })),
    )
  }

  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let base = format!("http://{}", listener.local_addr().unwrap());
  let port = base.split(':').next_back().unwrap().parse().unwrap();

  tokio::spawn(async move {
    axum::serve(listener, Router::new().fallback(answered).with_state(book)).await.unwrap();
  });

  (base, port)
}

// Where a placed call finds each unit, as the units' `tool-env` verbs write them under the data root.
fn write_env(root: &Path, base: String, port: u16) {
  std::fs::create_dir_all(root.join("env")).unwrap();

  for (file, body) in [
    ("env/receiver.env", format!("RECEIVER_ADDRESS={base}\nRECEIVER_TOKEN={TOKEN}\n")),
    ("env/brain.env", format!("BRAIN_ADDRESS={base}\nBRAIN_TOKEN={TOKEN}\n")),
    ("env/agency.env", format!("AGENCY_CONTROL_PORT={port}\nAGENCY_CONTROL_TOKEN={TOKEN}\n")),
  ] {
    std::fs::write(root.join(file), body).unwrap();
  }
}

// The variables of a turn made in a room, on a road, begun at a moment.
fn turn_of(agent: &str, chat: i64, via: &str, began: i64) -> BTreeMap<String, String> {
  BTreeMap::from([
    ("BMO_TURN_AGENT".to_owned(), agent.to_owned()),
    ("BMO_TURN_CHAT".to_owned(), chat.to_string()),
    ("BMO_TURN_VIA".to_owned(), via.to_owned()),
    ("BMO_TURN_BEGAN".to_owned(), began.to_string()),
    ("BMO_TURN_MESSAGES".to_owned(), "7".to_owned()),
  ])
}

// A call's record with no turn: made while no run is going, so it reads no room and no agent.
fn bare_record(data_root: &Path) -> Session {
  Session {
    data_root: data_root.to_path_buf(),
    ..Session::default()
  }
}

// A call's record: the data root naming the stood units, and the turn the call is made in.
fn record(data_root: &Path, agent: &str, chat: i64, via: &str, began: i64) -> Session {
  Session {
    data_root: data_root.to_path_buf(),
    run_variables: turn_of(agent, chat, via, began),
    ..Session::default()
  }
}

// The one route the last call went out on, and the body it was asked with.
fn asked_of(book: &Book) -> (String, Value) {
  let taken = book.taken.lock().unwrap();
  let (path, body) = taken.last().unwrap();

  (path.clone(), body.clone())
}

// Every route the calls went out on, in order.
fn routes_of(book: &Book) -> Vec<String> {
  let taken = book.taken.lock().unwrap();

  taken.iter().map(|(path, _)| path.clone()).collect()
}

#[test]
fn the_manifest_declares_each_entry_and_its_schema_as_the_executable_answers_it() {
  assert_eq!(differences(Path::new(env!("CARGO_MANIFEST_DIR")), ENTRIES), Vec::<String>::new());
}

#[tokio::test]
async fn say_is_asked_of_the_receivers_sends_route_with_the_turn_as_its_asker() {
  let book = Book::default();
  let (base, port) = stood(book.clone()).await;
  let data = tempdir().unwrap();
  write_env(data.path(), base, port);
  let session = record(data.path(), "agent-a", ROOM, "road-a", 1_000);

  let answered = call(&session, "say", json!({ "text": "hello there" })).await;

  assert_eq!(schema_of("say.json"), input_schema::<SayInput>());
  assert!(!answered.is_error, "{}", answered.plain());
  let (route, body) = asked_of(&book);

  assert_eq!(route.as_str(), "/sends");
  assert_eq!(
    body,
    json!({
      "asker": { "room": ROOM, "via": "road-a", "agentName": "agent-a", "by": format!("chat:{ROOM}") },
      "tool": "say",
      "input": { "text": "hello there" },
    }),
  );
}

#[tokio::test]
async fn consult_is_asked_of_the_receivers_consult_route_under_the_turns_agent() {
  let book = Book::default();
  let (base, port) = stood(book.clone()).await;
  let data = tempdir().unwrap();
  write_env(data.path(), base, port);
  let session = record(data.path(), "agent-a", ROOM, "road-a", 1_000);

  let answered = call(&session, "consult", json!({ "text": "we take the second one", "reply": 12 })).await;

  assert_eq!(schema_of("consult.json"), input_schema::<ConsultInput>());
  assert!(!answered.is_error, "{}", answered.plain());
  let (route, body) = asked_of(&book);

  assert_eq!(route.as_str(), "/consult-lines");
  assert_eq!(body, json!({ "agent": "agent-a", "text": "we take the second one", "replyTo": 12 }));
}

#[tokio::test]
async fn photo_is_asked_as_its_pictures_and_caption_with_the_room_the_call_names() {
  let book = Book::default();
  let (base, port) = stood(book.clone()).await;
  let data = tempdir().unwrap();
  write_env(data.path(), base, port);
  let session = record(data.path(), "agent-a", ROOM, "road-a", 1_000);

  let answered = call(
    &session,
    "photo",
    json!({
      "chat": -700,
      "pictures": [{ "url": "https://example.org/a.png", "label": "first" }],
      "caption": "the two of them",
    }),
  )
  .await;

  assert_eq!(schema_of("photo.json"), input_schema::<PhotoInput>());
  assert!(!answered.is_error, "{}", answered.plain());
  let (route, body) = asked_of(&book);

  assert_eq!(route.as_str(), "/sends");
  assert_eq!(
    body,
    json!({
      "asker": { "room": -700, "agentName": "agent-a", "by": format!("chat:{ROOM}") },
      "tool": "photo",
      "input": {
        "chat": -700,
        "pictures": [{ "url": "https://example.org/a.png", "label": "first" }],
        "caption": "the two of them",
      },
    }),
  );
}

#[tokio::test]
async fn remind_is_asked_of_the_brains_reminders_set_route_and_not_of_the_receiver() {
  let book = Book::default();
  let (base, port) = stood(book.clone()).await;
  let data = tempdir().unwrap();
  write_env(data.path(), base, port);
  let session = record(data.path(), "agent-a", ROOM, "road-a", 1_000);

  let answered = call(
    &session,
    "remind",
    json!({
      "at": "2027-01-01 09:00",
      "reason": "stand up the brew",
      "memory": "brew-row",
      "user": 7,
      "message": 7,
    }),
  )
  .await;

  assert_eq!(schema_of("remind.json"), input_schema::<RemindInput>());
  assert!(!answered.is_error, "{}", answered.plain());
  let (route, body) = asked_of(&book);

  assert_eq!(route.as_str(), "/reminders/set");
  assert_eq!(
    body,
    json!({
      "agent": "agent-a",
      "chatId": ROOM,
      "at": "2027-01-01 09:00",
      "reason": "stand up the brew",
      "memory": "brew-row",
      "user": 7,
    }),
  );
}

#[tokio::test]
async fn react_is_asked_on_the_message_it_acts_on_with_the_emoji_it_clears_with() {
  let book = Book::default();
  let (base, port) = stood(book.clone()).await;
  let data = tempdir().unwrap();
  write_env(data.path(), base, port);
  let session = record(data.path(), "agent-a", ROOM, "road-a", 1_000);

  let answered = call(&session, "react", json!({ "message": 900, "emoji": "" })).await;

  assert_eq!(schema_of("react.json"), input_schema::<ReactInput>());
  assert!(!answered.is_error, "{}", answered.plain());
  let (route, body) = asked_of(&book);

  assert_eq!(route.as_str(), "/sends");
  assert_eq!(
    body,
    json!({
      "asker": { "room": ROOM, "via": "road-a", "agentName": "agent-a", "by": format!("chat:{ROOM}") },
      "tool": "react",
      "input": { "message": 900, "emoji": "" },
    }),
  );
}

#[tokio::test]
async fn pin_is_asked_on_its_message_or_unpinned_there() {
  let book = Book::default();
  let (base, port) = stood(book.clone()).await;
  let data = tempdir().unwrap();
  write_env(data.path(), base, port);
  let session = record(data.path(), "agent-a", ROOM, "road-a", 1_000);

  let answered = call(&session, "pin", json!({ "message": 900, "unpin": true })).await;

  assert_eq!(schema_of("pin.json"), input_schema::<PinInput>());
  assert!(!answered.is_error, "{}", answered.plain());
  let (route, body) = asked_of(&book);

  assert_eq!(route.as_str(), "/sends");
  assert_eq!(
    body,
    json!({
      "asker": { "room": ROOM, "via": "road-a", "agentName": "agent-a", "by": format!("chat:{ROOM}") },
      "tool": "pin",
      "input": { "message": 900, "unpin": true },
    }),
  );
}

#[tokio::test]
async fn sticker_is_asked_by_its_emoji_alone() {
  let book = Book::default();
  let (base, port) = stood(book.clone()).await;
  let data = tempdir().unwrap();
  write_env(data.path(), base, port);
  let session = record(data.path(), "agent-a", ROOM, "road-a", 1_000);

  let answered = call(&session, "sticker", json!({ "emoji": ":+1:" })).await;

  assert_eq!(schema_of("sticker.json"), input_schema::<StickerInput>());
  assert!(!answered.is_error, "{}", answered.plain());
  let (route, body) = asked_of(&book);

  assert_eq!(route.as_str(), "/sends");
  assert_eq!(
    body,
    json!({
      "asker": { "room": ROOM, "via": "road-a", "agentName": "agent-a", "by": format!("chat:{ROOM}") },
      "tool": "sticker",
      "input": { "emoji": ":+1:" },
    }),
  );
}

#[tokio::test]
async fn a_draw_is_asked_with_its_shape_and_the_mode_its_result_comes_back_in() {
  let book = Book::default();
  let (base, port) = stood(book.clone()).await;
  let data = tempdir().unwrap();
  write_env(data.path(), base, port);
  let session = record(data.path(), "agent-a", ROOM, "road-a", 1_000);

  let answered = call(
    &session,
    "random_draw",
    json!({
      "mode": "sync",
      "shape": "dice",
      "count": 2,
      "sides": 6,
      "modifier": 1,
      "label": "who goes first",
    }),
  )
  .await;

  assert_eq!(schema_of("random_draw.json"), input_schema::<RandomDrawInput>());
  assert!(!answered.is_error, "{}", answered.plain());
  let (route, body) = asked_of(&book);

  assert_eq!(route.as_str(), "/sends");
  assert_eq!(
    body,
    json!({
      "asker": { "room": ROOM, "via": "road-a", "agentName": "agent-a", "by": format!("chat:{ROOM}") },
      "tool": "random_draw",
      "input": {
        "mode": "sync",
        "shape": "dice",
        "count": 2,
        "sides": 6,
        "modifier": 1,
        "label": "who goes first",
      },
    }),
  );
}

#[tokio::test]
async fn the_brains_cancel_and_list_routes_are_asked_as_the_brain_states_them() {
  let book = Book::default();
  let (base, port) = stood(book.clone()).await;
  let data = tempdir().unwrap();
  write_env(data.path(), base, port);
  let session = record(data.path(), "agent-a", ROOM, "road-a", 1_000);
  let brain = HttpBrain::new(&peers::brain(&session.data_root).unwrap());

  book.answers.lock().unwrap().insert(
    "/reminders/cancel".to_string(),
    json!({ "answered": false, "refusal": { "code": "no-reminder", "message": "There is no reminder 9." } }),
  );

  book.answers.lock().unwrap().insert(
    "/reminders/list".to_string(),
    json!({ "reminders": [{ "id": 3, "chatId": ROOM, "reason": "stand up the brew" }], "lines": ["#3 stand up the brew"] }),
  );

  let cancelled = brain.cancel_reminder(9).await;
  let listed = brain.reminders(ROOM, None, true).await;
  let (route, body) = asked_of(&book);

  assert_eq!(route.as_str(), "/reminders/list");
  assert_eq!(body, json!({ "chatId": ROOM, "agent": null, "all": true }));
  assert_eq!(routes_of(&book), Vec::from(["/reminders/cancel", "/reminders/list"]));

  match cancelled.unwrap() {
    Err(Refusal { code, message, .. }) => {
      assert_eq!((code.as_str(), message.as_str()), ("no-reminder", "There is no reminder 9."))
    }
    Ok(reading) => panic!("the reminder was cancelled: {reading:?}"),
  }
  assert!(listed.is_ok());
}

#[tokio::test]
async fn a_turn_its_agent_reset_after_it_began_is_refused_before_anything_is_asked() {
  let book = Book::default();
  let (base, port) = stood(book.clone()).await;
  let data = tempdir().unwrap();
  write_env(data.path(), base, port);
  let session = record(data.path(), "agent-a", ROOM, "road-a", 1_000);

  book
    .answers
    .lock()
    .unwrap()
    .insert("/reset-point".to_string(), json!({ "point": 2_000 }));

  let answered = call(&session, "say", json!({ "text": "late" })).await;

  assert!(answered.is_error);
  assert!(answered.plain().contains("reset after this turn began"));
  assert_eq!(routes_of(&book), Vec::from(["/reset-point"]));
}

#[tokio::test]
async fn a_call_outside_any_room_or_any_agents_turn_is_refused_without_asking() {
  let book = Book::default();
  let (base, port) = stood(book.clone()).await;
  let data = tempdir().unwrap();
  write_env(data.path(), base, port);
  let session = bare_record(data.path());

  let nowhere = call(&session, "say", json!({ "text": "hello" })).await;

  assert!(nowhere.is_error && nowhere.plain().contains(NO_ROOM));

  let no_agent = call(&session, "remind", json!({ "reason": "stand up" })).await;

  assert!(no_agent.is_error && no_agent.plain().contains(NO_AGENT));
  assert!(book.taken.lock().unwrap().is_empty());
}

#[tokio::test]
async fn the_executable_answers_one_request_on_its_standard_streams() {
  let book = Book::default();
  let (base, port) = stood(book.clone()).await;
  let data = tempdir().unwrap();
  write_env(data.path(), base, port);

  let request = json!({
    "protocol": 1,
    "kind": "call",
    "entry": "bmo/sticker",
    "arguments": { "emoji": ":+1:" },
    "session": {
      "dataRoot": data.path(),
      "runVariables": turn_of("agent-a", ROOM, "road-a", 1_000),
    },
  });
  let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_send-tool"))
    .env_remove("RECEIVER_ADDRESS")
    .env_remove("RECEIVER_TOKEN")
    .env_remove("BRAIN_ADDRESS")
    .env_remove("BRAIN_TOKEN")
    .env_remove("AGENCY_CONTROL_PORT")
    .env_remove("AGENCY_CONTROL_TOKEN")
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .spawn()
    .unwrap();

  {
    use tokio::io::AsyncWriteExt;

    let mut input = child.stdin.take().unwrap();

    input.write_all(request.to_string().as_bytes()).await.unwrap();
  }

  let done = child.wait_with_output().await.unwrap();

  assert!(done.status.success());
  assert_eq!(
    serde_json::from_slice::<Value>(&done.stdout).unwrap(),
    json!({ "content": [{ "type": "text", "text": "sent" }], "isError": false }),
  );
  assert_eq!(routes_of(&book), Vec::from(["/reset-point", "/sends"]));
}

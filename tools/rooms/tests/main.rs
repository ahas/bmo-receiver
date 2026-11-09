//! The rooms tool: its entries and the schemas its manifest names, the room added on the owner's word in the one
//! mode the road sets, the receiver's own refusals passed back, and the rooms and contacts listed by name so the
//! owner picks.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{StatusCode, Uri};
use serde_json::{Value, json};
use tempfile::tempdir;

use rooms_tool::{ENTRIES, call};
use tool_call::{Session, differences};

const ROOM: i64 = 4_200;
const TOKEN: &str = "quiet";

// A unit stood in for on a loopback port: every request kept in order, each path answered as the test set it.
#[derive(Clone, Default)]
struct Book {
  taken: Arc<Mutex<Vec<(String, Value)>>>,
  answers: Arc<Mutex<BTreeMap<String, (u16, Value)>>>,
}

// A room as `GET /rooms/{roomId}` reads it, which the receiver answers a room it added with.
fn room() -> Value {
  json!({
    "id": ROOM,
    "title": "a room",
    "messageCount": 0,
    "lastMessageAt": null,
    "pinned": false,
    "senders": [],
    "supervisors": [],
    "ownerRoom": false,
    "registered": true,
    "workRoom": ROOM,
    "mode": "record-only",
  })
}

async fn stood(book: Book) -> String {
  async fn answered(State(book): State<Book>, uri: Uri, body: Bytes) -> (StatusCode, Json<Value>) {
    let path = uri.path().to_owned();
    let said: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);

    book.taken.lock().unwrap().push((path.clone(), said));
    let (status, answer) = book.answers.lock().unwrap().get(&path).cloned().unwrap_or((201, room()));

    (StatusCode::from_u16(status).unwrap(), Json(answer))
  }

  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let base = format!("http://{}", listener.local_addr().unwrap());

  tokio::spawn(async move {
    axum::serve(listener, Router::new().fallback(answered).with_state(book)).await.unwrap();
  });

  base
}

// Where a placed call finds the receiver, as its `tool-env` verb writes it under the data root.
fn write_env(root: &Path, base: String) {
  std::fs::create_dir_all(root.join("env")).unwrap();
  std::fs::write(
    root.join("env/receiver.env"),
    format!("RECEIVER_ADDRESS={base}\nRECEIVER_TOKEN={TOKEN}\n"),
  )
  .unwrap();
}

// The variables of a turn the receiver marked the owner's, or one it did not.
fn turn(privileged: &str) -> BTreeMap<String, String> {
  BTreeMap::from([
    ("BMO_TURN_CHAT".to_owned(), ROOM.to_string()),
    ("BMO_TURN_MESSAGES".to_owned(), "7".to_owned()),
    ("BMO_TURN_PRIVILEGED".to_owned(), privileged.to_owned()),
  ])
}

// A call's record: the data root naming the stood receiver, and the turn the call is made in.
fn record(data_root: &Path, privileged: &str) -> Session {
  Session {
    data_root: data_root.to_path_buf(),
    run_variables: turn(privileged),
    ..Session::default()
  }
}

fn taken(book: &Book) -> Vec<(String, Value)> {
  book.taken.lock().unwrap().clone()
}

#[test]
fn the_entries_the_manifest_names_are_answered_and_their_schemas_stand_beside() {
  for name in ["room_open", "room_list", "room_supervisors", "room_add"] {
    assert!(ENTRIES.iter().any(|entry| entry.name == name), "{name} is answered");
  }

  for entry in ENTRIES {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(entry.input);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|why| panic!("{path:?}: {why}"));
    let schema: serde_json::Value =
      serde_json::from_str(&text).unwrap_or_else(|why| panic!("{path:?}: {why}"));

    assert_eq!(schema["type"], "object", "{path:?}");
    assert_eq!(schema["additionalProperties"], false, "{path:?}");
  }

  assert_eq!(differences(Path::new(env!("CARGO_MANIFEST_DIR")), ENTRIES), Vec::<String>::new());
}

#[tokio::test]
async fn a_call_that_is_not_the_owners_word_is_refused_before_any_room_is_asked() {
  let book = Book::default();
  let base = stood(book.clone()).await;
  let data = tempdir().unwrap();
  write_env(data.path(), base);
  let session = record(data.path(), "0");

  let answered = call(&session, "room_add", json!({ "provider": "telegram", "roomId": 5 })).await;

  assert!(answered.is_error, "{}", answered.plain());
  assert!(answered.plain().contains("not the owner's"), "{}", answered.plain());
  assert!(taken(&book).is_empty(), "a room was asked of the receiver: {:?}", taken(&book));
}

#[tokio::test]
async fn the_only_mode_the_road_sets_is_record_only() {
  let book = Book::default();
  let base = stood(book.clone()).await;
  let data = tempdir().unwrap();
  write_env(data.path(), base);
  let session = record(data.path(), "1");

  let answered = call(&session, "room_add", json!({ "provider": "telegram", "roomId": 5 })).await;

  assert!(!answered.is_error, "{}", answered.plain());
  let asked = taken(&book);

  assert_eq!(asked.len(), 1, "{asked:?}");
  assert_eq!(asked[0].0, "/rooms");
  assert_eq!(asked[0].1, json!({ "provider": "telegram", "roomId": 5, "mode": "record-only" }));

  let avatar =
    call(&session, "room_add", json!({ "provider": "telegram", "roomId": 5, "mode": "avatar" })).await;

  assert!(avatar.is_error, "avatar mode was taken: {}", avatar.plain());
  assert_eq!(taken(&book), asked, "a call naming a mode was asked of the receiver");
}

#[tokio::test]
async fn a_room_added_already_is_refused_in_the_receivers_own_words() {
  let book = Book::default();
  book.answers.lock().unwrap().insert(
    "/rooms".to_owned(),
    (409, json!({ "code": "room-added", "error": "room 5 was added already" })),
  );
  let base = stood(book.clone()).await;
  let data = tempdir().unwrap();
  write_env(data.path(), base);
  let session = record(data.path(), "1");

  let answered = call(&session, "room_add", json!({ "provider": "telegram", "roomId": 5 })).await;

  assert!(answered.is_error, "{}", answered.plain());
  assert!(answered.plain().contains("room 5 was added already"), "{}", answered.plain());
}

#[tokio::test]
async fn a_room_not_named_exactly_lists_the_providers_rooms_and_contacts_by_name() {
  let book = Book::default();
  {
    let mut answers = book.answers.lock().unwrap();

    answers.insert(
      "/providers".to_owned(),
      (
        200,
        json!({
          "items": [{
            "key": "telegram",
            "label": "Telegram",
            "listsRooms": true,
            "listsContacts": true,
            "startsChats": true,
            "startsGroups": true,
            "roomModes": ["record-only", "avatar"],
          }],
          "more": false,
        }),
      ),
    );
    answers.insert(
      "/providers/telegram/rooms".to_owned(),
      (
        200,
        json!({
          "items": [{
            "id": 1,
            "title": "Bongboss",
            "kind": "owner",
            "privateTo": null,
            "held": false,
            "mode": null,
          }],
          "more": false,
        }),
      ),
    );
    answers.insert(
      "/providers/telegram/contacts".to_owned(),
      (
        200,
        json!({
          "items": [{
            "id": 9,
            "name": "Bongboss",
            "kind": "bot",
            "roomId": 1,
            "held": false,
            "mode": null,
          }],
          "more": false,
        }),
      ),
    );
  }
  let base = stood(book.clone()).await;
  let data = tempdir().unwrap();
  write_env(data.path(), base);
  let session = record(data.path(), "1");

  let answered = call(&session, "room_add", json!({ "provider": "telegram" })).await;

  assert!(!answered.is_error, "{}", answered.plain());
  let said = answered.plain();

  assert!(said.contains("Bongboss"), "{said}");
  assert!(said.contains("- 1 \"Bongboss\", owner, not added"), "{said}");
  assert!(said.contains("- 9 Bongboss, bot, room 1, not added"), "{said}");
  assert!(
    taken(&book).iter().all(|(path, _)| path != "/rooms"),
    "a listing added a room: {:?}",
    taken(&book)
  );
}

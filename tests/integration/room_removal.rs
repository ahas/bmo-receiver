//! A room's registration removed from the console (docs/receiver.md, 「Removing a room's registration」): a room
//! filled across the receiver's own record, its files and the units it reaches is shown with nothing written,
//! then taken whole as shown; every other room stands untouched; a caller other than the administrator, the
//! infrastructure room the configuration names and the administrator's own private chat write nothing; and a room
//! that speaks again afterwards is a new registration with an empty record.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use serde_json::{Value, json};
use sqlx::{AnyConnection, Connection};
use tempfile::tempdir;

use bmo_receiver::data_root::DataRoot;
use bmo_receiver::events::Events;
use bmo_receiver::receiver::{Context, Deployment, Inbound, Room};
use bmo_receiver::room_record::chat_tables;
use bmo_receiver::routes::routes;

use crate::common::Scratch;
use crate::common::queue::{Front, Kicked, TOKEN, remove};
use crate::common::units::{AGENT, StandInChat, deployment, standing_on};

const ADMIN: &str = "the-administrator";
const ROOM: i64 = -4_300;
const OTHER: i64 = -4_301;
// The room the chat service says is the administrator's own private chat.
const OWNERS: i64 = -4_302;
// The board for the machines, as the deployment under this test names it.
const BOARD: i64 = -4_303;
// A setting naming a room that no declaration covers, which the removal lists and keeps.
const UNDECLARED: &str = "people_digest_since:bmo";

// A deployment of the test's own, naming the board.
fn stood_deployment(board: i64) -> Deployment {
  let mut given = deployment();

  given.infra_room = Some(board);
  // The administrator's private chat is the room the configuration names as his.
  given.owner_room = Some(OWNERS);
  given
}

/// The front standing as the receiver for an owner, on a data root of its own, reaching one chat service that
/// holds the rooms and names one of them the administrator's private chat, and hearing its news in a stream of
/// its own the test reads.
async fn bench() -> (Scratch, Front, Events) {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let chats = vec![Arc::new(StandInChat::new("road-a").holding(&[ROOM, OTHER, OWNERS]).owners(OWNERS))];
  let news = Events::new();
  let mut standing = standing_on(scratch.db(), DataRoot::new(data.path()), chats, stood_deployment(BOARD));
  let place: Arc<str> = scratch.address().into();

  standing.receiver.hold_place(place);
  standing.web.admin(ADMIN.to_owned());
  standing.receiver = standing.receiver.with_news(Arc::new(news.clone()));

  let kicked = Arc::new(Kicked(Mutex::default()));

  standing.receiver.hold_pass(kicked.clone());

  (
    scratch,
    Front {
      routes: routes(standing.receiver.clone(), TOKEN),
      data,
      standing,
      kicked,
    },
    news,
  )
}

/// A room filled across the receiver's record, its files and the units it reaches: its rows, its title and read
/// position, its picture, what its runs wrote down, a setting naming it that no declaration covers, and the three
/// folders of its files.
async fn fill(scratch: &Scratch, data: &std::path::Path, room: i64) {
  let picture = data.join("avatars").join(format!("room-{room}.jpg"));

  for (sql, values) in [
    (
      r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES ($1, 'a room')"#,
      vec![room.into()],
    ),
    (
      r#"INSERT INTO "provider_rooms" ("provider", "room_id", "chat_id", "title") VALUES ('road-a', $2, $1, 'a room')"#,
      vec![room.into(), format!("r{room}").into()],
    ),
    (
      r#"INSERT INTO "room_answerers" ("chat_id", "agent") VALUES ($1, $2)"#,
      vec![room.into(), AGENT.into()],
    ),
    (
      r#"INSERT INTO "chat_reads" ("chat_id", "last_read_message_id") VALUES ($1, 1)"#,
      vec![room.into()],
    ),
    (r#"INSERT INTO "photo_chats" ("chat_id", "look") VALUES ($1, 1)"#, vec![room.into()]),
    (r#"INSERT INTO "delegate_chats" ("chat_id") VALUES ($1)"#, vec![room.into()]),
    (r#"INSERT INTO "tool_chats" ("chat_id") VALUES ($1)"#, vec![room.into()]),
    (
      r#"INSERT INTO "random_draw_rooms" ("chat_id", "next_seed", "next_commitment") VALUES ($1, 's', 'c')"#,
      vec![room.into()],
    ),
    (
      r#"INSERT INTO "collected_items" ("chat_id", "message_id", "sender_name", "kind", "source", "collected_at")
         VALUES ($1, 1, 'tester', 'document', 'a road', 1)"#,
      vec![room.into()],
    ),
    (
      r#"INSERT INTO "instructions" ("chat_id", "agent", "text", "created_at") VALUES ($1, $2, 'say less', 1)"#,
      vec![room.into(), AGENT.into()],
    ),
    (
      r#"INSERT INTO "avatars" ("kind", "subject_id", "photo_id", "path", "updated_at") VALUES ('room', $1, 'p', $2, 1)"#,
      vec![room.into(), picture.display().to_string().into()],
    ),
    (
      r#"INSERT INTO "run_records" ("run", "kind", "text", "at") VALUES ($1, 'note', 'chat', 1)"#,
      vec![format!("chat:{room}:1:1").into()],
    ),
    (
      r#"INSERT INTO "settings" ("key", "value") VALUES ($1, '1')"#,
      vec![format!("{UNDECLARED}:{room}").into()],
    ),
  ] {
    scratch.run(sql, values).await;
  }

  scratch.message(room, 1, 7).await;
  scratch.message(room, 2, 8).await;
  scratch
    .run(
      r#"INSERT INTO "message_metadata" ("chat_id", "message_id", "metadata") VALUES ($1, 1, '{"kept":true}')"#,
      vec![room.into()],
    )
    .await;

  for folder in ["collected", "images", "outbox"] {
    let at = data.join(folder).join(room.to_string());

    std::fs::create_dir_all(&at).unwrap();
    std::fs::write(at.join("file.txt"), "a file").unwrap();
  }

  std::fs::create_dir_all(picture.parent().unwrap()).unwrap();
  std::fs::write(&picture, "png").unwrap();
}

/// How many rows naming a room each table the database says holds rooms keeps.
async fn rows_of(scratch: &Scratch, room: i64) -> BTreeMap<String, i64> {
  sqlx::any::install_default_drivers();

  let mut archive = AnyConnection::connect(&scratch.address()).await.unwrap();
  let tables = chat_tables(&mut archive).await.unwrap();

  archive.close().await.unwrap();

  let mut counted: BTreeMap<String, i64> = BTreeMap::new();

  for table in tables {
    let rows = scratch
      .count(&format!(r#"SELECT count(*) FROM "{table}" WHERE "chat_id" = $1"#), vec![room.into()])
      .await;

    counted.insert(table, rows);
  }

  counted
}

// How many rows of one table a side of the confirmation counts.
fn shown_rows(list: &[Value], table: &str) -> i64 {
  list
    .iter()
    .filter(|counted| counted["table"] == table)
    .map(|counted| counted["rows"].as_i64().unwrap_or_default())
    .sum::<i64>()
}

#[tokio::test]
async fn a_room_is_shown_then_taken_whole_as_shown_and_every_other_room_left_standing() {
  let (scratch, front, news) = bench().await;
  let data = front.data.path().to_path_buf();

  fill(&scratch, &data, ROOM).await;
  fill(&scratch, &data, OTHER).await;
  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value") VALUES ('console_pinned_rooms', $1)"#,
      vec![format!("[{OTHER},{ROOM}]").into()],
    )
    .await;

  let other_before = rows_of(&scratch, OTHER).await;
  let before = rows_of(&scratch, ROOM).await;

  let (status, shown) = remove(&front, &format!("/rooms/{ROOM}?by={ADMIN}&dryRun=true")).await;

  assert_eq!(status, StatusCode::OK, "{shown}");
  assert!(shown["refused"].is_null(), "{shown}");
  assert_eq!(rows_of(&scratch, ROOM).await, before, "the preview writes nothing");

  let goes = shown["goes"].as_array().unwrap().clone();

  assert_eq!(shown_rows(&goes, "messages"), 2);
  assert_eq!(shown_rows(&goes, "chat_titles"), 1);
  assert_eq!(shown_rows(&goes, "message_metadata"), 1);
  assert_eq!(shown_rows(&goes, "avatars"), 1);
  assert_eq!(shown_rows(&goes, "run_records"), 1);
  assert!(shown["stays"].as_array().unwrap().is_empty());
  assert_eq!(shown["pictures"].as_array().unwrap().len(), 1);
  assert_eq!(shown["folders"].as_array().unwrap().len(), 3);
  assert!(shown["settings"]["taken"].as_array().unwrap().is_empty());
  assert_eq!(shown["settings"]["undeclared"].as_array().unwrap().len(), 1);
  assert_eq!(shown["settings"]["trimmed"], json!(["console_pinned_rooms"]));
  assert!(shown["warns"].as_array().unwrap().is_empty());
  assert_eq!(shown["units"]["agency"]["gone"]["sessions"], 1);
  assert_eq!(
    shown["units"].as_object().unwrap().keys().collect::<Vec<_>>(),
    ["agency"],
    "only the agency is asked"
  );

  let (status, taken) = remove(&front, &format!("/rooms/{ROOM}?by={ADMIN}")).await;

  assert_eq!(status, StatusCode::OK, "{taken}");
  assert_eq!(taken["taken"], shown["goes"], "what the preview counted is what the removal took");
  assert_eq!(taken["cleared"], shown["stays"]);
  assert!(taken["couldNot"].as_array().unwrap().is_empty());
  assert_eq!(taken["pictures"].as_array().unwrap().len(), 1);
  assert_eq!(taken["folders"].as_array().unwrap().len(), 3);
  assert!(taken["holders"].as_array().unwrap().is_empty());

  for (table, rows) in rows_of(&scratch, ROOM).await {
    assert_eq!(rows, 0, "{table} after the removal");
  }

  assert_eq!(
    scratch
      .count(
        r#"SELECT count(*) FROM "settings" WHERE "key" = $1"#,
        vec![format!("{UNDECLARED}:{ROOM}").into()]
      )
      .await,
    1
  );
  assert_eq!(
    scratch
      .texts(r#"SELECT "value" FROM "settings" WHERE "key" = 'console_pinned_rooms'"#)
      .await,
    [format!("[{OTHER}]")],
    "the room is taken out of the pinned order and the rest of it stands",
  );
  assert_eq!(
    scratch
      .count(
        r#"SELECT count(*) FROM "avatars" WHERE "kind" = 'room' AND "subject_id" = $1"#,
        vec![ROOM.into()]
      )
      .await,
    0
  );
  assert_eq!(
    scratch
      .count(
        r#"SELECT count(*) FROM "run_records" WHERE "run" = $1"#,
        vec![format!("chat:{ROOM}:1:1").into()]
      )
      .await,
    0
  );

  for folder in ["collected", "images", "outbox"] {
    assert!(!data.join(folder).join(ROOM.to_string()).exists(), "{folder}");
    assert!(data.join(folder).join(OTHER.to_string()).exists(), "{folder}");
  }

  assert!(!data.join("avatars").join(format!("room-{ROOM}.jpg")).exists());
  assert!(data.join("avatars").join(format!("room-{OTHER}.jpg")).exists());

  assert_eq!(rows_of(&scratch, OTHER).await, other_before);
  assert!(
    news
      .held()
      .iter()
      .any(|event| event["type"] == "room-removed" && event["chatId"] == ROOM)
  );

  assert_eq!(front.standing.agency.forgotten.lock().unwrap().clone(), [(ROOM, true), (ROOM, false)]);
}

#[tokio::test]
async fn anyone_but_the_administrator_is_refused_and_nothing_is_written() {
  let (scratch, front, _news) = bench().await;
  let data = front.data.path().to_path_buf();

  fill(&scratch, &data, ROOM).await;

  let before = rows_of(&scratch, ROOM).await;

  for by in ["stranger", ""] {
    assert_eq!(
      remove(&front, &format!("/rooms/{ROOM}?by={by}&dryRun=true")).await.0,
      StatusCode::FORBIDDEN
    );
    assert_eq!(remove(&front, &format!("/rooms/{ROOM}?by={by}")).await.0, StatusCode::FORBIDDEN);
  }

  assert_eq!(
    remove(&front, &format!("/rooms/not-a-room?by={ADMIN}&dryRun=true")).await.0,
    StatusCode::BAD_REQUEST
  );
  assert_eq!(rows_of(&scratch, ROOM).await, before, "a refusal writes nothing");
  assert!(data.join("collected").join(ROOM.to_string()).exists());
  assert_eq!(remove(&front, &format!("/rooms/{ROOM}?by={ADMIN}&dryRun=true")).await.0, StatusCode::OK);
}

#[tokio::test]
async fn the_board_for_the_machines_and_the_administrators_own_private_chat_are_refused_untouched() {
  let (scratch, front, _news) = bench().await;
  let data = front.data.path().to_path_buf();

  for room in [BOARD, OWNERS] {
    fill(&scratch, &data, room).await;

    let before = rows_of(&scratch, room).await;
    let (status, shown) = remove(&front, &format!("/rooms/{room}?by={ADMIN}&dryRun=true")).await;

    assert_eq!(status, StatusCode::OK);
    assert!(shown["refused"].is_string(), "{room}: {shown}");

    let (status, refused) = remove(&front, &format!("/rooms/{room}?by={ADMIN}")).await;

    assert_eq!(status, StatusCode::CONFLICT, "{room}: {refused}");
    assert_eq!(refused["code"], shown["refused"], "{room}");
    assert_eq!(rows_of(&scratch, room).await, before, "{room}");
    assert!(data.join("collected").join(room.to_string()).exists(), "{room}");
  }
}

#[tokio::test]
async fn a_room_that_speaks_again_afterwards_is_a_new_registration_with_an_empty_record() {
  let (scratch, front, _news) = bench().await;
  let data = front.data.path().to_path_buf();

  fill(&scratch, &data, ROOM).await;

  let (status, taken) = remove(&front, &format!("/rooms/{ROOM}?by={ADMIN}")).await;

  assert_eq!(status, StatusCode::OK, "{taken}");

  let back = Inbound {
    chat_id: ROOM,
    message_id: 3,
    sender_id: Some(7),
    sender_name: "tester".into(),
    sent_at: 1_760_000_000,
    text: "back again".into(),
    image_path: None,
    image_state: None,
    transcript: None,
    voice_seconds: None,
    voice_state: None,
    reply_to_message_id: None,
    via: "road-a".into(),
    mode: None,
    via_reader: None,
    album: None,
  };
  let context = Context {
    room_title: Some("a room again".into()),
    ..Context::default()
  };

  front
    .standing
    .receiver
    .message(
      &back,
      &context,
      Room {
        owner_room: false,
        identity: Some(7),
        private_to: None,
      },
    )
    .await
    .unwrap();

  let after = rows_of(&scratch, ROOM).await;

  assert_eq!(after["messages"], 1);
  assert_eq!(after["chat_titles"], 1);

  for (table, rows) in after {
    match table.as_str() {
      "messages" | "chat_titles" => assert_eq!(rows, 1, "{table}"),
      _ => assert_eq!(rows, 0, "{table}"),
    }
  }
}

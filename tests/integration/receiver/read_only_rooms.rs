//! A read-only room (docs/receiver.md, 「A read-only room」): a room its chat service reads in the owner's account
//! alone, under no identity of the service's own, is kept and read like any room and wakes no agent, whatever
//! calls one; the owner's own words from the console still go out there in his own voice, and an agent's send is
//! refused before anything is sent.

use std::sync::Arc;

use axum::{
  Router,
  body::{Body, to_bytes},
  extract::Request,
  http::StatusCode,
};
use bmo_chat_client::{ChatModeStanding, ChatVoice};
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

use crate::integration::common::Scratch;
use crate::integration::common::queue::as_console;
use crate::integration::common::units::{
  ADDRESS, AGENT, OWNER, SENDER, StandInChat, Standing, registration, standing, woken,
};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::{Context, Inbound};
use bmo_receiver::roads::{Road, providers};
use bmo_receiver::routes::routes;

const TOKEN: &str = "the-token";
// A room the service reads in the owner's account alone, and one its own account is in.
const READER_ROOM: i64 = -170;
const BOT_ROOM: i64 = -171;
// A room on the record alone.
const CONSOLE_ROOM: i64 = -172;
/// The number the service's own account carries in the room it is in.
const BOT_NUMBER: i64 = 7;
/// Somebody in the rooms who is not the agent.
const PERSON: i64 = 501;

// A receiver over a scratch database, judging by its own wake, reaching one service that reads through the
// owner's account and answers with an account of its own, which is in one of its two rooms.
async fn stood() -> (Scratch, TempDir, Standing, Arc<StandInChat>) {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let road = Arc::new(
    StandInChat::new("road-a")
      .holding(&[READER_ROOM, BOT_ROOM])
      .agent_in(BOT_ROOM, BOT_NUMBER),
  );

  road.moding(ChatVoice::BotAccount, ChatModeStanding::Active);
  road.moding(ChatVoice::HumanReader, ChatModeStanding::Active);

  let standing = woken(standing(scratch.db(), DataRoot::new(data.path()), vec![road.clone()]));
  // The rooms stand on the console's room list, so the receiver listens to them (docs/wake.md).
  bmo_receiver::receiver::rows::allow_chat(scratch.db(), READER_ROOM).await.unwrap();
  bmo_receiver::receiver::rows::allow_chat(scratch.db(), BOT_ROOM).await.unwrap();
  let mut agent = registration(AGENT, SENDER, vec![ADDRESS], None);

  agent.row.insert("role".to_owned(), json!("chat"));
  standing.agency.registering(vec![agent]);

  for chat_id in [READER_ROOM, BOT_ROOM] {
    scratch
      .run(
        r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES ($1, 'A room')"#,
        vec![chat_id.into()],
      )
      .await;
  }

  (scratch, data, standing, road)
}

fn said(chat_id: i64, message_id: i64, sender_id: i64, text: &str) -> Inbound {
  Inbound {
    chat_id,
    message_id,
    sender_id: Some(sender_id),
    sender_name: "tester".into(),
    sent_at: 1_760_000_000,
    text: text.into(),
    image_path: None,
    image_state: None,
    transcript: None,
    voice_seconds: None,
    voice_state: None,
    reply_to_message_id: None,
    via: "road-a".into(),
    mode: Some("human-reader".into()),
    via_reader: None,
    album: None,
  }
}

async fn heard(standing: &Standing, message: Inbound) {
  let room = standing.receiver.roads().room_facts(message.chat_id).await;

  standing.receiver.message(&message, &Context::default(), room).await.unwrap();
}

async fn asked(port: &Router, path: &str, body: Value) -> (StatusCode, Value) {
  let request = Request::post(path)
    .header("authorization", format!("Bearer {TOKEN}"))
    .body(Body::from(body.to_string()))
    .unwrap();
  let answer = port.clone().oneshot(request).await.unwrap();
  let status = answer.status();
  let written = to_bytes(answer.into_body(), usize::MAX).await.unwrap();

  (status, serde_json::from_slice(&written).unwrap_or(Value::Null))
}

#[tokio::test]
async fn a_call_by_name_in_a_read_only_room_starts_no_turn_and_is_kept_while_the_same_call_in_a_bot_room_does()
 {
  let (scratch, _data, standing, road) = stood().await;

  // Both rooms name the agent, since a chat service's group room naming nobody is answered by nobody.
  scratch
    .run(
      r#"INSERT INTO "room_supervisors" ("chat_id", "supervisor") VALUES ($1, $3), ($2, $3)"#,
      vec![READER_ROOM.into(), BOT_ROOM.into(), AGENT.into()],
    )
    .await;

  assert!(
    providers::read_only(scratch.db(), standing.receiver.roads(), READER_ROOM)
      .await
      .unwrap()
  );
  assert!(!providers::read_only(scratch.db(), standing.receiver.roads(), BOT_ROOM).await.unwrap());

  // The owner calls the agent by name in both rooms: the read-only room keeps the words and wakes nobody, and
  // the room the service's own account is in wakes the agent by the one rule.
  for chat_id in [READER_ROOM, BOT_ROOM] {
    heard(&standing, said(chat_id, 1, OWNER, &format!("{ADDRESS}, are you there?"))).await;
  }

  let woken: Vec<i64> = standing.taken.handed().iter().map(|handed| handed.chat_id).collect();

  assert_eq!(woken, [BOT_ROOM]);
  assert_eq!(
    scratch
      .count(r#"SELECT COUNT(*) FROM "messages" WHERE "chat_id" = $1"#, vec![READER_ROOM.into()])
      .await,
    1,
    "the read-only room keeps the call in the archive",
  );

  // Neither an open window nor a reply to the agent's own words opens a turn there either.
  scratch
    .run(
      r#"INSERT INTO "room_windows" ("chat_id", "agent", "open_until") VALUES ($1, $2, $3)"#,
      vec![READER_ROOM.into(), AGENT.into(), (bmo_receiver::now_ms() + 600_000).into()],
    )
    .await;
  scratch.message(READER_ROOM, 2, BOT_NUMBER).await;

  let mut reply = said(READER_ROOM, 3, PERSON, "and what about this");

  reply.reply_to_message_id = Some(2);
  heard(&standing, reply.clone()).await;
  assert!(
    !bmo_receiver::receiver::wake::wakes(
      &standing.receiver,
      AGENT,
      &reply,
      standing.receiver.roads().room_facts(READER_ROOM).await
    )
    .await
    .unwrap()
  );
  assert_eq!(standing.taken.handed().len(), 1, "only the bot room's call was handed on");
  assert!(road.sent().is_empty(), "nothing was sent into either room");
}

#[tokio::test]
async fn the_owners_console_words_go_out_in_his_own_voice_in_a_read_only_room_and_an_agents_send_is_refused()
 {
  let (scratch, _data, standing, road) = stood().await;
  let port = routes(standing.receiver.clone(), as_console(TOKEN));
  let (status, answer) =
    asked(&port, "/owner/messages", json!({ "chatId": READER_ROOM, "text": "The game begins." })).await;

  assert_eq!(status, StatusCode::OK, "{answer:?}");
  assert_eq!(
    road.sent().iter().map(|held| (held.room_id, held.mode)).collect::<Vec<_>>(),
    [(READER_ROOM, Some(ChatVoice::HumanReader))],
    "the owner's words go out in his own voice",
  );
  assert!(standing.taken.handed().is_empty(), "his words open no turn there");

  let (status, answer) = asked(
    &port,
    "/sends",
    json!({
      "asker": { "room": READER_ROOM, "agentName": AGENT, "by": format!("chat:{READER_ROOM}") },
      "tool": "pin",
      "input": { "chat": READER_ROOM, "message": 1, "unpin": false },
    }),
  )
  .await;

  assert_eq!(status, StatusCode::OK);
  assert!(
    answer["refusal"].as_str().is_some_and(|refusal| refusal.contains("read-only")),
    "{answer:?}"
  );
  assert_eq!(road.sent().len(), 1, "the agent's send was refused before anything went");
  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "messages" WHERE "chat_id" = $1 AND "sender_id" IS DISTINCT FROM $2"#,
        vec![READER_ROOM.into(), OWNER.into()]
      )
      .await,
    0,
    "nothing but the owner's words stands in the room",
  );
}

#[tokio::test]
async fn a_room_on_the_record_alone_is_not_read_only() {
  let (scratch, _data, standing, _road) = stood().await;

  scratch.message(CONSOLE_ROOM, 1, OWNER).await;
  providers::choose(scratch.db(), CONSOLE_ROOM, &[Road::Record.key().to_owned()])
    .await
    .unwrap();

  assert!(
    !providers::read_only(scratch.db(), standing.receiver.roads(), CONSOLE_ROOM)
      .await
      .unwrap()
  );
}

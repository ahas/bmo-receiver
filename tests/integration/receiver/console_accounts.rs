//! The console's accounts in its rooms (docs/receiver.md, 「Console rooms and accounts」): words another account
//! says carry nothing granted to the owner, the owner's account keeps all of it as his own road does, a provider
//! speaks as the account only where it says it can, a room's members by account come and go with a line saying so,
//! an account's record identity is made once, a BMO's private room wakes on its account's words alone and refuses
//! every other, a read of it leaves nothing behind, and a messenger's member is answered as before. Every room read
//! says whose private room it is, every write another account makes in one is refused while its own go through, a
//! group room every other member has left stays a group room, and a forward stands under the account that forwards.

use std::sync::Arc;

use axum::{
  Router,
  body::{Body, to_bytes},
  extract::Request,
  http::StatusCode,
};
use bmo_chat_client::{ChatModeStanding, ChatSend, ChatVoice};
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

use crate::integration::common::Scratch;
use crate::integration::common::units::{
  ADDRESS, AGENT, OWNER, SENDER, StandInChat, Standing, Woken, registration, standing, woken,
};
use bmo_receiver::chat::conversation::joins_round;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::private_rooms::{self, Asked};
use bmo_receiver::receiver::{Context, How, Inbound, Room};
use bmo_receiver::routes::routes;
use bmo_receiver::store::migration::{Migrator, migrate};
use bmo_receiver::verbs::VerbError;
use bmo_receiver::verbs::room_rules::{self, Change, Form, Subject};
use receiver_codes::RoomGrantRefusal;
use sea_orm_migration::MigratorTrait;

const TOKEN: &str = "the-token";
// The owner's account, which the web unit links to his sender, and two other accounts.
const OWNERS: i64 = 1;
const BEA: i64 = 7;
const CAL: i64 = 8;
// The supervisor Bea owns, with its own room on the record.
const BEAS_BMO: &str = "agent-b";
const BEAS_ROOM: i64 = -9000000000050;
// A room a messenger holds, which says nothing of speakers, and one that speaks as Cal alone.
const LINKED: i64 = -100;
const SPOKEN: i64 = -200;

struct Stood {
  scratch: Scratch,
  _data: TempDir,
  standing: Standing,
  linked: Arc<StandInChat>,
  spoken: Arc<StandInChat>,
  port: Router,
}

// A receiver over a scratch database with two messenger roads, the owner's account linked to his sender, the
// default answerer owned by the owner's account and Bea's supervisor with its own room on the record; judging by
// its own wake where `waking`, and recording what it hands the wake otherwise.
async fn stood(waking: bool) -> Stood {
  stood_linked(waking, true).await
}

// The same receiver, the web unit linking the owner's account to his sender only where `linked`.
async fn stood_linked(waking: bool, linked_owner: bool) -> Stood {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let linked = StandInChat::new("road-a").holding(&[LINKED]).agent_in(LINKED, 900);
  let spoken = StandInChat::new("road-b")
    .holding(&[SPOKEN])
    .agent_in(SPOKEN, 901)
    .speaking_for(&["account:8"]);

  for road in [&linked, &spoken] {
    road.moding(ChatVoice::BotAccount, ChatModeStanding::Active);
    road.moding(ChatVoice::HumanReader, ChatModeStanding::Active);
  }

  let (linked, spoken) = (Arc::new(linked), Arc::new(spoken));
  let made = standing(scratch.db(), DataRoot::new(data.path()), vec![linked.clone(), spoken.clone()]);
  let standing = if waking { woken(made) } else { made };
  let mut own = registration(AGENT, SENDER, vec![ADDRESS], None);
  let mut beas = registration(BEAS_BMO, "Agent B", vec![BEAS_BMO], None);

  own.row.insert("ownerAccount".to_owned(), json!(OWNERS));
  beas.row.insert("ownerAccount".to_owned(), json!(BEA));
  standing.agency.registering(vec![own, beas]);
  if linked_owner {
    standing.web.linking(OWNER, OWNERS.to_string());
  }

  for (room, title) in [(LINKED, "Linked"), (SPOKEN, "Spoken")] {
    scratch
      .run(
        r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES ($1, $2)"#,
        vec![room.into(), title.into()],
      )
      .await;
  }

  // Bea's supervisor's own room, as the receiver writes it when the supervisor is added.
  scratch
    .run(
      r#"INSERT INTO "provider_rooms" ("provider", "room_id", "chat_id", "title") VALUES ('record', $1, $2, 'Bea and her BMO')"#,
      vec![BEAS_BMO.into(), BEAS_ROOM.into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "room_providers" ("chat_id", "provider", "place") VALUES ($1, 'record', 0)"#,
      vec![BEAS_ROOM.into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES ($1, 'Bea and her BMO')"#,
      vec![BEAS_ROOM.into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "room_supervisors" ("chat_id", "supervisor") VALUES ($1, $2)"#,
      vec![BEAS_ROOM.into(), BEAS_BMO.into()],
    )
    .await;

  // Bea's supervisor's number on the record, as its addition writes it, and whose private room each room is
  // settled as the service's first start after the record began keeping them settles it.
  scratch
    .run(
      r#"INSERT INTO "provider_users" ("provider", "user_id", "sender_id") VALUES ('record', $1, -950)"#,
      vec![BEAS_BMO.into()],
    )
    .await;
  private_rooms::at_start(&standing.receiver).await.unwrap();

  let port = routes(standing.receiver.clone(), TOKEN);

  Stood {
    scratch,
    _data: data,
    standing,
    linked,
    spoken,
    port,
  }
}

async fn asked(port: &Router, method: &str, path: &str, body: Option<Value>) -> (StatusCode, Value) {
  let request = Request::builder()
    .method(method)
    .uri(path)
    .header("authorization", format!("Bearer {TOKEN}"))
    .header("content-type", "application/json")
    .body(body.map_or_else(Body::empty, |body| Body::from(body.to_string())))
    .unwrap();
  let answer = port.clone().oneshot(request).await.unwrap();
  let status = answer.status();
  let written = to_bytes(answer.into_body(), usize::MAX).await.unwrap();

  (status, serde_json::from_slice(&written).unwrap_or(Value::Null))
}

// Words an account says in a room, as the console hands them over: the number they were kept under.
async fn says(port: &Router, room: i64, account: i64, name: &str, text: &str) -> (StatusCode, Value) {
  asked(
    port,
    "POST",
    &format!("/rooms/{room}/words"),
    Some(json!({ "account": account, "name": name, "text": text })),
  )
  .await
}

async fn said(port: &Router, room: i64, account: i64, name: &str, text: &str) -> i64 {
  let (status, answer) = says(port, room, account, name, text).await;

  assert_eq!(status, StatusCode::CREATED, "{answer}");
  answer["id"].as_i64().unwrap()
}

// A group room on the record alone answered by the default answerer, opened by `opener`.
async fn group(port: &Router, opener: i64) -> i64 {
  let (status, room) = asked(
    port,
    "POST",
    "/rooms",
    Some(json!({ "title": "A group", "supervisors": [AGENT], "openedBy": opener })),
  )
  .await;

  assert_eq!(status, StatusCode::CREATED, "{room}");
  room["id"].as_i64().unwrap()
}

async fn identity(port: &Router, account: i64) -> i64 {
  let (status, answer) = asked(port, "PUT", &format!("/accounts/{account}/identity"), None).await;

  assert_eq!(status, StatusCode::OK, "{answer}");
  assert_eq!(answer["account"], json!(account));
  answer["sender"].as_i64().unwrap()
}

// Who a kept message stands under: its sender and the name it carries.
async fn kept_as(stood: &Stood, room: i64, message: i64) -> (i64, String) {
  let (_, kept) = asked(&stood.port, "GET", &format!("/rooms/{room}/messages/{message}"), None).await;

  (kept["senderId"].as_i64().unwrap(), kept["senderName"].as_str().unwrap().to_owned())
}

// Whether the room's pictures are switched on: the owner's command's mark.
async fn photos_on(stood: &Stood, room: i64) -> bool {
  stood
    .scratch
    .count(r#"SELECT count(*) FROM "photo_chats" WHERE "chat_id" = $1"#, vec![room.into()])
    .await
    > 0
}

// Whether the wake was handed the message as the owner's.
fn woken_as_owner(standing: &Standing, room: i64, message: i64) -> Option<bool> {
  standing.waking.woken().iter().find_map(|woken| match woken {
    Woken::Words {
      chat_id,
      message_id,
      owner,
      how: How::Words,
    } if *chat_id == room && *message_id == message => Some(*owner),
    _ => None,
  })
}

// The sends a road was handed: each one's words, voice and speaker.
fn carried(road: &StandInChat) -> Vec<(String, Option<ChatVoice>, Option<String>)> {
  road
    .sent()
    .iter()
    .map(|held| match &held.send {
      ChatSend::Text { text, .. } => (text.clone(), held.mode, held.speaker.clone()),
      other => (format!("{other:?}"), held.mode, held.speaker.clone()),
    })
    .collect()
}

#[tokio::test]
async fn words_another_account_says_carry_nothing_granted_to_the_owner() {
  let stood = stood(false).await;
  let room = group(&stood.port, BEA).await;
  let beas = identity(&stood.port, BEA).await;

  stood.standing.web.modelling();

  // The owner's setting command, typed by another account, is words like any other.
  let command = said(&stood.port, room, BEA, "Bea", "photos:on").await;

  assert_eq!(kept_as(&stood, room, command).await, (beas, "Bea".to_owned()));
  assert_ne!(beas, OWNER);
  assert!(!photos_on(&stood, room).await, "no owner command runs on her words");
  assert_eq!(woken_as_owner(&stood.standing, room, command), Some(false), "nor are they woken as his");

  let (_, read) =
    asked(&stood.port, "GET", &format!("/rooms/{room}/messages/{command}/sender-is-owner"), None).await;

  assert_eq!(read, json!({ "owner": false }));

  let (_, spoke) = asked(
    &stood.port,
    "GET",
    &format!(
      "/rooms/{room}/owner-spoke?person={beas}&before={}&windowMs=600000",
      bmo_receiver::now_ms() + 1_000
    ),
    None,
  )
  .await;

  assert_eq!(spoke, json!({ "owner": false, "spoke": false }));

  // No grant is made on her message as the owner's.
  let granted = room_rules::change(
    &stood.standing.receiver,
    Some(stood.standing.receiver.owner_id()),
    Change {
      chat_id: room,
      message_id: command,
      form: Form::Grant("ask".to_owned()),
      subject: Subject::Members,
      words: None,
      expires_at: None,
    },
  )
  .await;

  assert!(
    matches!(granted, Err(VerbError::Refused(why)) if why == RoomGrantRefusal::NotOwnersMessage.to_string()),
    "a grant on her message is refused as not the owner's"
  );
  // A round over her words is no owner's round: they never join one held as his, so no turn of theirs is marked
  // privileged, files work as asked by the owner, or owes an owner's directive.
  assert!(!joins_round(true, &[beas], OWNER));

  // The owner's private room is his alone: her words there, where his answer to a job's question would be read and
  // he is always answered, are refused before anything is kept.
  let (status, refusal) = says(&stood.port, OWNER, BEA, "Bea", "the answer is yes").await;

  assert_eq!(status, StatusCode::FORBIDDEN);
  assert_eq!(refusal["code"], "private-room");

  // On a messenger her words go in the road's own voice with her name before them, never in the owner's.
  said(&stood.port, LINKED, BEA, "Bea", "hello there").await;

  assert_eq!(
    carried(&stood.linked),
    [("Bea: hello there".to_owned(), Some(ChatVoice::BotAccount), None)]
  );
  assert_eq!(stood.linked.sent()[0].account_id, Some(BEA), "in her account, not the room's leader's");
}

#[tokio::test]
async fn the_owners_account_through_the_console_keeps_all_his_own_road_gives_him() {
  let stood = stood(false).await;
  let room = group(&stood.port, OWNERS).await;
  let through_account = said(&stood.port, room, OWNERS, "Owner", "good morning").await;
  let (status, through_road) = asked(
    &stood.port,
    "POST",
    "/owner/messages",
    Some(json!({ "chatId": room, "text": "good evening" })),
  )
  .await;
  let through_road = through_road["id"].as_i64().unwrap();

  assert_eq!(status, StatusCode::OK);
  assert_eq!(
    kept_as(&stood, room, through_account).await,
    kept_as(&stood, room, through_road).await,
    "the same sender and the same name his road files"
  );
  assert_eq!(kept_as(&stood, room, through_account).await.0, OWNER);
  assert_eq!(woken_as_owner(&stood.standing, room, through_account), Some(true));
  assert_eq!(woken_as_owner(&stood.standing, room, through_road), Some(true));
  assert!(joins_round(true, &[OWNER], OWNER));

  said(&stood.port, room, OWNERS, "Owner", "photos:on").await;

  assert!(photos_on(&stood, room).await, "his command runs");

  let owners_room = said(&stood.port, OWNER, OWNERS, "Owner", "the answer is yes").await;

  assert_eq!(woken_as_owner(&stood.standing, OWNER, owners_room), Some(true), "in his own chat too");

  said(&stood.port, LINKED, OWNERS, "Owner", "hello there").await;

  assert_eq!(
    carried(&stood.linked),
    [("hello there".to_owned(), Some(ChatVoice::HumanReader), None)],
    "on a messenger in his own voice, as his road carries it"
  );
}

#[tokio::test]
async fn a_provider_that_takes_speakers_speaks_as_the_account_it_knows_and_names_any_other() {
  let stood = stood(false).await;

  said(&stood.port, SPOKEN, CAL, "Cal", "from Cal").await;
  said(&stood.port, SPOKEN, BEA, "Bea", "from Bea").await;

  assert_eq!(
    carried(&stood.spoken),
    [
      ("from Cal".to_owned(), Some(ChatVoice::BotAccount), Some("account:8".to_owned())),
      ("Bea: from Bea".to_owned(), Some(ChatVoice::BotAccount), None),
    ],
    "Cal's words as Cal; Bea's, refused as a speaker it has no number for, named in its own voice"
  );
}

// A room's members by account, as the route answers them.
async fn members(stood: &Stood, room: i64) -> Value {
  asked(&stood.port, "GET", &format!("/rooms/{room}/accounts"), None).await.1["accounts"].clone()
}

// The words a room's record holds, in the order kept.
async fn lines(stood: &Stood, room: i64) -> Vec<String> {
  stood
    .scratch
    .texts(&format!(r#"SELECT "text" FROM "messages" WHERE "chat_id" = {room} ORDER BY "rowid""#))
    .await
}

#[tokio::test]
async fn an_account_is_let_in_and_taken_out_with_a_line_saying_who() {
  let stood = stood(false).await;
  let room = group(&stood.port, BEA).await;
  let opened = members(&stood, room).await;

  assert_eq!(opened.as_array().unwrap().len(), 1);
  assert_eq!(opened[0]["account"], json!(BEA));
  assert_eq!(opened[0]["addedBy"], json!(BEA));
  assert!(lines(&stood, room).await.is_empty(), "the opener stands with no line");

  let (status, member) =
    asked(&stood.port, "PUT", &format!("/rooms/{room}/accounts/{CAL}"), Some(json!({ "by": BEA }))).await;

  assert_eq!(status, StatusCode::OK);
  assert_eq!((member["account"].clone(), member["addedBy"].clone()), (json!(CAL), json!(BEA)));
  assert_eq!(
    asked(&stood.port, "PUT", &format!("/rooms/{room}/accounts/{CAL}"), Some(json!({ "by": BEA }))).await,
    (StatusCode::OK, member.clone()),
    "let in again, it stands as it was"
  );
  assert_eq!(lines(&stood, room).await, ["account-let-in in en"], "one line, once");
  assert_eq!(members(&stood, room).await.as_array().unwrap().len(), 2);
  assert_eq!(
    asked(&stood.port, "GET", &format!("/accounts/{CAL}/rooms"), None).await.1,
    json!({ "rooms": [room] })
  );

  let (status, _) =
    asked(&stood.port, "DELETE", &format!("/rooms/{room}/accounts/{CAL}?by={BEA}"), None).await;

  assert_eq!(status, StatusCode::NO_CONTENT);
  assert_eq!(
    members(&stood, room).await,
    json!([opened[0]]),
    "a member taken out is no longer listed"
  );
  assert_eq!(
    asked(&stood.port, "GET", &format!("/accounts/{CAL}/rooms"), None).await.1,
    json!({ "rooms": [] }),
    "nor reads the room through its membership"
  );

  asked(&stood.port, "DELETE", &format!("/rooms/{room}/accounts/{BEA}?by={BEA}"), None).await;
  asked(&stood.port, "DELETE", &format!("/rooms/{room}/accounts/{CAL}?by={BEA}"), None).await;

  assert_eq!(
    lines(&stood, room).await,
    ["account-let-in in en", "account-taken-out in en", "account-left in en"],
    "taken out and leaving each said once, and nothing for one who is no member"
  );
  assert_eq!(
    asked(&stood.port, "PUT", &format!("/rooms/{room}/accounts/{CAL}"), Some(json!({})))
      .await
      .0,
    StatusCode::BAD_REQUEST
  );
  assert_eq!(
    asked(&stood.port, "PUT", "/rooms/-9000000000099/accounts/8", Some(json!({ "by": BEA })))
      .await
      .0,
    StatusCode::NOT_FOUND
  );
}

#[tokio::test]
async fn an_accounts_record_identity_is_made_once_and_kept() {
  let stood = stood(false).await;
  let beas = identity(&stood.port, BEA).await;

  assert_eq!(identity(&stood.port, BEA).await, beas);
  assert_ne!(identity(&stood.port, CAL).await, beas);

  let words = said(&stood.port, LINKED, BEA, "Bea", "the same one").await;

  assert_eq!(kept_as(&stood, LINKED, words).await.0, beas, "her words stand under it");
}

#[tokio::test]
async fn a_bmos_private_room_wakes_on_its_accounts_words_alone_and_refuses_every_other() {
  let stood = stood(true).await;
  let beas = identity(&stood.port, BEA).await;

  assert_eq!(stood.standing.receiver.roads().room_facts(BEAS_ROOM).await.private_to, Some(beas));

  let words = said(&stood.port, BEAS_ROOM, BEA, "Bea", "good morning").await;
  let woken: Vec<(String, i64, i64)> = stood
    .standing
    .taken
    .handed()
    .iter()
    .map(|handed| (handed.agent.clone(), handed.chat_id, handed.message_id))
    .collect();

  assert_eq!(woken, [(BEAS_BMO.to_owned(), BEAS_ROOM, words)], "no name and no window needed");

  for other in [CAL, OWNERS] {
    let (status, refusal) = says(&stood.port, BEAS_ROOM, other, "Other", "let me in").await;

    assert_eq!((status, refusal["code"].clone()), (StatusCode::FORBIDDEN, json!("private-room")));
  }

  for (method, path) in [
    ("PUT", format!("/rooms/{BEAS_ROOM}/accounts/{CAL}")),
    ("DELETE", format!("/rooms/{BEAS_ROOM}/accounts/{BEA}?by={OWNERS}")),
  ] {
    let (status, refusal) = asked(&stood.port, method, &path, Some(json!({ "by": OWNERS }))).await;

    assert_eq!((status, refusal["code"].clone()), (StatusCode::FORBIDDEN, json!("private-room")));
  }

  assert_eq!(
    asked(&stood.port, "GET", &format!("/rooms/{BEAS_ROOM}/accounts"), None).await.1,
    json!({ "accounts": [{ "account": BEA, "addedBy": null, "addedAt": null }] })
  );
  assert_eq!(
    asked(&stood.port, "GET", &format!("/accounts/{BEA}/rooms"), None).await.1,
    json!({ "rooms": [BEAS_ROOM] })
  );
  assert_eq!(
    asked(&stood.port, "GET", &format!("/accounts/{OWNERS}/rooms"), None).await.1,
    json!({ "rooms": [OWNER] }),
    "the owner's private room is his account's"
  );
  assert_eq!(stood.standing.taken.handed().len(), 1, "nothing refused woke anyone");
}

// Every row of every table, so a read that wrote anything is seen.
async fn everything(scratch: &Scratch) -> Vec<String> {
  let tables = scratch
    .texts(
      "SELECT table_name::text FROM information_schema.tables
       WHERE table_schema = current_schema() AND table_type = 'BASE TABLE' ORDER BY 1",
    )
    .await;
  let mut rows = Vec::new();

  for table in tables {
    rows.extend(
      scratch
        .texts(&format!(r#"SELECT '{table}:' || to_jsonb(t)::text FROM "{table}" t ORDER BY 1"#))
        .await,
    );
  }

  rows
}

#[tokio::test]
async fn reading_a_private_room_leaves_nothing_behind() {
  let stood = stood(true).await;
  let words = said(&stood.port, BEAS_ROOM, BEA, "Bea", "only for my BMO").await;
  let before = everything(&stood.scratch).await;
  let told = stood.standing.heard.told();
  let handed = stood.standing.taken.handed().len();

  for path in [
    format!("/rooms/{BEAS_ROOM}"),
    format!("/rooms/{BEAS_ROOM}/messages"),
    format!("/rooms/{BEAS_ROOM}/messages?by=any"),
    format!("/rooms/{BEAS_ROOM}/messages/{words}"),
    format!("/rooms/{BEAS_ROOM}/messages/{words}/files"),
    format!("/rooms/{BEAS_ROOM}/title"),
    format!("/rooms/{BEAS_ROOM}/window"),
    format!("/rooms/{BEAS_ROOM}/situation"),
    format!("/rooms/{BEAS_ROOM}/pins"),
    format!("/rooms/{BEAS_ROOM}/answerers"),
    format!("/rooms/{BEAS_ROOM}/accounts"),
    format!("/rooms/{BEAS_ROOM}/figures"),
    format!("/rooms/{BEAS_ROOM}/day-start?from=0"),
    format!("/rooms/{BEAS_ROOM}/pictures"),
    "/rooms".to_owned(),
  ] {
    let (status, answer) = asked(&stood.port, "GET", &path, None).await;

    assert_eq!(status, StatusCode::OK, "{path}: {answer}");
  }

  assert_eq!(everything(&stood.scratch).await, before, "no mark, window or row of who read");
  assert_eq!(stood.standing.heard.told(), told, "no screen is told of it");
  assert_eq!(stood.standing.taken.handed().len(), handed, "no one is woken");
}

#[tokio::test]
async fn an_account_on_its_record_identity_alone_calls_its_own_supervisor_and_its_supervisor_reads_the_room()
 {
  let stood = stood(true).await;
  let room = group(&stood.port, BEA).await;
  let beas = identity(&stood.port, BEA).await;
  let reads = |port: Router| async move {
    asked(&port, "GET", &format!("/supervisors/{BEAS_BMO}/rooms/{room}/messages"), None)
      .await
      .0
  };

  assert_eq!(
    reads(stood.port.clone()).await,
    StatusCode::OK,
    "a member by account, before saying a word"
  );

  // The web unit links her record identity to her account as it links any identity, and the room grants her ask.
  stood.standing.web.linking(beas, BEA.to_string());
  stood.standing.web.granting(room, beas, "ask".to_owned());

  let words = said(&stood.port, room, BEA, "Bea", "agent-b, have a look").await;

  assert!(
    stood
      .standing
      .taken
      .handed()
      .iter()
      .any(|handed| handed.agent == BEAS_BMO && handed.chat_id == room && handed.message_id == words),
    "her call by name reaches her own supervisor"
  );

  // Taken out, her supervisor reads the room by her words alone, as any member's does; one who never spoke there
  // reads it no more.
  let quiet = group(&stood.port, BEA).await;

  asked(&stood.port, "DELETE", &format!("/rooms/{quiet}/accounts/{BEA}?by={BEA}"), None).await;

  let (status, refusal) =
    asked(&stood.port, "GET", &format!("/supervisors/{BEAS_BMO}/rooms/{quiet}/messages"), None).await;

  assert_eq!((status, refusal["code"].clone()), (StatusCode::FORBIDDEN, json!("not-a-member")));
}

#[tokio::test]
async fn a_messengers_member_with_no_console_membership_is_answered_as_before() {
  let stood = stood(true).await;

  stood
    .scratch
    .run(
      r#"INSERT INTO "room_supervisors" ("chat_id", "supervisor") VALUES ($1, $2)"#,
      vec![LINKED.into(), AGENT.into()],
    )
    .await;

  let message = Inbound {
    chat_id: LINKED,
    message_id: 5,
    sender_id: Some(555),
    sender_name: "A member".to_owned(),
    sent_at: bmo_receiver::now_ms(),
    text: format!("{ADDRESS}, are you there?"),
    image_path: None,
    image_state: None,
    transcript: None,
    voice_seconds: None,
    voice_state: None,
    reply_to_message_id: None,
    via: "road-a".to_owned(),
    mode: None,
    via_reader: None,
    album: None,
  };
  let room: Room = stood.standing.receiver.roads().room_facts(LINKED).await;

  stood.standing.receiver.message(&message, &Context::default(), room).await.unwrap();

  assert!(
    stood
      .standing
      .taken
      .handed()
      .iter()
      .any(|handed| handed.agent == AGENT && handed.message_id == 5),
    "the call wakes the room's supervisor"
  );
  assert_eq!(
    asked(&stood.port, "GET", &format!("/rooms/{LINKED}/accounts"), None).await.1,
    json!({ "accounts": [] }),
    "and no account is made a member of the room for it"
  );
  assert!(carried(&stood.linked).is_empty(), "nor is the member's own word sent back to the road");
}

#[tokio::test]
async fn files_another_account_sends_stand_under_its_identity_and_go_out_in_the_roads_own_voice() {
  let stood = stood(false).await;
  let beas = identity(&stood.port, BEA).await;
  let body = format!(
    "--own\r\ncontent-disposition: form-data; name=\"account\"\r\n\r\n{BEA}\r\n\
     --own\r\ncontent-disposition: form-data; name=\"name\"\r\n\r\nBea\r\n\
     --own\r\ncontent-disposition: form-data; name=\"text\"\r\n\r\nlook\r\n\
     --own\r\ncontent-type: text/plain\r\ncontent-disposition: form-data; name=\"file\"; filename=\"notes.txt\"\r\n\r\nsome notes\r\n\
     --own--\r\n"
  );
  let request = Request::post(format!("/rooms/{LINKED}/word-files"))
    .header("authorization", format!("Bearer {TOKEN}"))
    .header("content-type", "multipart/form-data; boundary=own")
    .body(Body::from(body))
    .unwrap();
  let answer = stood.port.clone().oneshot(request).await.unwrap();
  let status = answer.status();
  let answer: Value =
    serde_json::from_slice(&to_bytes(answer.into_body(), usize::MAX).await.unwrap()).unwrap();

  assert_eq!(status, StatusCode::OK, "{answer}");
  assert_eq!(kept_as(&stood, LINKED, answer["id"].as_i64().unwrap()).await, (beas, "Bea".to_owned()));

  let sent = carried(&stood.linked);

  assert!(sent.iter().all(|(_, voice, _)| *voice == Some(ChatVoice::BotAccount)), "{sent:?}");
  assert!(sent.iter().any(|(words, _, _)| words.contains("Bea: look")), "{sent:?}");
}

// The administrator's account, which the web unit links to no sender of the owner's.
const ADMIN: i64 = 5;

// A write as the routes take it: its method, its path with its query, and its body where it has one.
type Write = (&'static str, String, Option<Value>);

// A write the console relay hands over for an account: `Owner-Key` naming it, `Owner-Role` its role.
async fn relayed(
  port: &Router,
  (method, path, body): Write,
  account: i64,
  role: &str,
) -> (StatusCode, Value) {
  let request = Request::builder()
    .method(method)
    .uri(&path)
    .header("authorization", format!("Bearer {TOKEN}"))
    .header("content-type", "application/json")
    .header("owner-key", account.to_string())
    .header("owner-role", role)
    .body(body.map_or_else(Body::empty, |body| Body::from(body.to_string())))
    .unwrap();
  let answer = port.clone().oneshot(request).await.unwrap();
  let status = answer.status();
  let written = to_bytes(answer.into_body(), usize::MAX).await.unwrap();

  (status, serde_json::from_slice(&written).unwrap_or(Value::Null))
}

// Every write act the console makes in a room for an account, each by its name: on the message `message` of the
// room, and a forward into the room of the message `from` of the room `source`.
fn writes(
  room: i64,
  message: i64,
  (source, from): (i64, i64),
  sticker: &str,
  account: i64,
) -> Vec<(&'static str, Write)> {
  let on = format!("/rooms/{room}/messages/{message}");

  vec![
    (
      "reaction",
      (
        "PUT",
        format!("{on}/reactions/mine"),
        Some(json!({ "emoji": sticker, "account": account })),
      ),
    ),
    ("reaction taken off", ("DELETE", format!("{on}/reactions/mine?account={account}"), None)),
    (
      "message pin",
      ("PUT", format!("{on}/pin"), Some(json!({ "on": true, "account": account }))),
    ),
    ("edit", ("PATCH", on.clone(), Some(json!({ "text": "changed", "account": account })))),
    (
      "sticker",
      (
        "POST",
        format!("/rooms/{room}/stickers"),
        Some(json!({ "emoji": sticker, "account": account })),
      ),
    ),
    (
      "forward",
      (
        "POST",
        format!("/rooms/{source}/messages/{from}/forwards"),
        Some(json!({ "to": room, "account": account })),
      ),
    ),
    ("reading", ("POST", format!("{on}/reading?account={account}"), None)),
    (
      "room pin",
      ("POST", "/pinned-rooms".to_owned(), Some(json!({ "room": room, "account": account }))),
    ),
    ("room unpin", ("DELETE", format!("/pinned-rooms/{room}?account={account}"), None)),
    (
      "pinned order",
      (
        "PUT",
        "/pinned-rooms".to_owned(),
        Some(json!({ "roomIds": [room], "account": account })),
      ),
    ),
    (
      "words",
      (
        "POST",
        format!("/rooms/{room}/words"),
        Some(json!({ "account": account, "name": "A", "text": "hi" })),
      ),
    ),
    ("removal", ("DELETE", format!("{on}?account={account}"), None)),
  ]
}

async fn a_sticker(port: &Router) -> String {
  asked(port, "GET", "/stickers", None).await.1["items"][0]["emoji"]
    .as_str()
    .unwrap()
    .to_owned()
}

#[tokio::test]
async fn every_write_another_account_makes_in_a_private_room_is_refused_and_its_own_goes_through() {
  let stood = stood(false).await;
  let sticker = a_sticker(&stood.port).await;
  let beas = identity(&stood.port, BEA).await;

  stood.standing.web.linking(beas, BEA.to_string());

  let source = group(&stood.port, BEA).await;
  let from = said(&stood.port, source, BEA, "Bea", "worth passing on").await;
  let hers = said(&stood.port, BEAS_ROOM, BEA, "Bea", "only for my BMO").await;
  let owners = said(&stood.port, OWNER, OWNERS, "Owner", "only for me").await;
  let before = lines(&stood, BEAS_ROOM).await;

  for (room, message) in [(BEAS_ROOM, hers), (OWNER, owners)] {
    for (other, role) in [(ADMIN, "administrator"), (CAL, "member")] {
      for (act, write) in writes(room, message, (source, from), &sticker, other) {
        let (status, refusal) = relayed(&stood.port, write, other, role).await;

        assert_eq!(
          (status, refusal["code"].clone(), refusal["error"].is_string()),
          (StatusCode::FORBIDDEN, json!("private-room"), true),
          "{act} by account {other} in room {room}: {refusal}"
        );
      }
    }
  }

  // An act on a message names its account by the relay's `Owner-Key` where its call names none.
  let (status, refusal) = relayed(
    &stood.port,
    ("PUT", format!("/rooms/{BEAS_ROOM}/messages/{hers}/pin"), Some(json!({ "on": true }))),
    ADMIN,
    "administrator",
  )
  .await;

  assert_eq!((status, refusal["code"].clone()), (StatusCode::FORBIDDEN, json!("private-room")));
  assert_eq!(lines(&stood, BEAS_ROOM).await, before, "nothing refused was kept");

  for (act, write) in writes(BEAS_ROOM, hers, (source, from), &sticker, BEA) {
    let (status, answer) = relayed(&stood.port, write, BEA, "member").await;

    if act == "reading" {
      // Her own words are no agent's line to read: the ask goes on to the reading's own judgement.
      assert_eq!(status, StatusCode::NOT_FOUND, "{act}: {answer}");
    } else {
      assert!(status.is_success(), "{act} by the room's own account: {status} {answer}");
    }
  }

  let (status, answer) = relayed(
    &stood.port,
    (
      "PUT",
      format!("/rooms/{OWNER}/messages/{owners}/pin"),
      Some(json!({ "on": true, "account": OWNERS })),
    ),
    OWNERS,
    "administrator",
  )
  .await;

  assert_eq!(status, StatusCode::OK, "the owner's account acts in his private room: {answer}");
}

#[tokio::test]
async fn an_act_naming_an_account_owner_key_does_not_is_refused() {
  let stood = stood(false).await;
  let room = group(&stood.port, BEA).await;
  let words = said(&stood.port, room, BEA, "Bea", "hello").await;
  let (status, refusal) = relayed(
    &stood.port,
    (
      "PUT",
      format!("/rooms/{room}/messages/{words}/pin"),
      Some(json!({ "on": true, "account": BEA })),
    ),
    CAL,
    "member",
  )
  .await;

  assert_eq!((status, refusal["code"].clone()), (StatusCode::BAD_REQUEST, json!("bad-request")));
}

#[tokio::test]
async fn a_group_room_every_other_member_has_left_is_no_private_room() {
  let stood = stood(false).await;
  let sticker = a_sticker(&stood.port).await;
  let room = group(&stood.port, BEA).await;

  asked(&stood.port, "PUT", &format!("/rooms/{room}/accounts/{CAL}"), Some(json!({ "by": BEA }))).await;
  asked(&stood.port, "DELETE", &format!("/rooms/{room}/accounts/{BEA}?by={BEA}"), None).await;

  assert_eq!(members(&stood, room).await.as_array().unwrap().len(), 1, "Cal alone stands in it");
  assert_eq!(
    asked(&stood.port, "GET", &format!("/rooms/{room}"), None).await.1["privateOf"],
    Value::Null
  );

  for account in [CAL, ADMIN] {
    let sender = identity(&stood.port, account).await;

    stood.standing.web.linking(sender, account.to_string());
  }

  let cals = said(&stood.port, room, CAL, "Cal", "just me now").await;
  let source = group(&stood.port, BEA).await;
  let from = said(&stood.port, source, BEA, "Bea", "worth passing on").await;

  // The administrator's writes are judged as in any group room: none refused as a private room's.
  for (act, write) in writes(room, cals, (source, from), &sticker, ADMIN) {
    let (status, answer) = relayed(&stood.port, write, ADMIN, "administrator").await;

    assert_ne!(answer["code"], json!("private-room"), "{act}: {status} {answer}");
  }

  assert_eq!(
    asked(&stood.port, "GET", &format!("/accounts/{CAL}/rooms"), None).await.1,
    json!({ "rooms": [room] }),
    "nor is it listed among Cal's private rooms"
  );
}

#[tokio::test]
async fn every_room_read_says_whose_private_room_it_is() {
  let stood = stood(false).await;
  let room = group(&stood.port, BEA).await;

  said(&stood.port, BEAS_ROOM, BEA, "Bea", "mine").await;
  said(&stood.port, OWNER, OWNERS, "Owner", "his").await;
  said(&stood.port, room, BEA, "Bea", "ours").await;

  let listed = asked(&stood.port, "GET", "/rooms", None).await.1;
  let private_of = |chat: i64| {
    listed["items"]
      .as_array()
      .unwrap()
      .iter()
      .find(|item| item["id"] == json!(chat))
      .map(|item| item["privateOf"].clone())
  };

  assert_eq!(private_of(BEAS_ROOM), Some(json!(BEA)));
  assert_eq!(private_of(OWNER), Some(json!(OWNERS)));
  assert_eq!(private_of(room), Some(Value::Null));

  for (chat, own) in [(BEAS_ROOM, json!(BEA)), (OWNER, json!(OWNERS)), (room, Value::Null)] {
    assert_eq!(asked(&stood.port, "GET", &format!("/rooms/{chat}"), None).await.1["privateOf"], own);
  }
}

#[tokio::test]
async fn a_forward_stands_under_the_account_that_forwards_it() {
  let stood = stood(false).await;
  let beas = identity(&stood.port, BEA).await;
  let source = group(&stood.port, BEA).await;
  let into = group(&stood.port, BEA).await;
  let from = said(&stood.port, source, BEA, "Bea", "worth passing on").await;
  let owner_name = stood.standing.receiver.owner_name(into).await;
  let forwarded = |body: Value| {
    let port = stood.port.clone();

    async move {
      let (status, answer) =
        asked(&port, "POST", &format!("/rooms/{source}/messages/{from}/forwards"), Some(body)).await;

      assert_eq!(status, StatusCode::CREATED, "{answer}");
      answer["id"].as_i64().unwrap()
    }
  };

  let unnamed = forwarded(json!({ "to": into, "account": BEA })).await;
  let named = forwarded(json!({ "to": into, "account": BEA, "name": "Bea B" })).await;
  let owners = forwarded(json!({ "to": into, "account": OWNERS })).await;
  let nobodys = forwarded(json!({ "to": into })).await;

  assert_eq!(kept_as(&stood, into, named).await, (beas, "Bea B".to_owned()));
  assert_eq!(
    kept_as(&stood, into, unnamed).await,
    (beas, "Bea".to_owned()),
    "the name she last went by"
  );
  assert_eq!(
    kept_as(&stood, into, owners).await,
    (OWNER, owner_name.clone()),
    "the owner's account is his"
  );
  assert_eq!(
    kept_as(&stood, into, nobodys).await,
    (OWNER, owner_name),
    "a forward naming none is his"
  );
}

// The routes that change something and act for no account but a unit's own under the receiver's token, each with
// why no account's private room is judged there; every other write names its room and is judged by the write guard.
const UNIT_WRITES: [(&str, &str); 28] = [
  ("POST /rooms", "a room added is no one's yet"),
  ("POST /sends", "an agent's own act through its tools"),
  ("POST /instructions", "a unit's instruction"),
  ("DELETE /instructions/{instructionId}", "a unit's instruction withdrawn"),
  ("POST /owner/notices", "the receiver's notice to the owner"),
  ("PUT /room-holders/{holderName}", "a unit declaring what it holds"),
  ("POST /talks", "a talk with no room"),
  ("POST /handoffs/{agentName}", "an agent's handoff"),
  ("POST /handovers/{agentName}", "an agent's handover"),
  ("PUT /voice-briefing", "the deployment's switches"),
  ("POST /events", "a unit's event"),
  ("POST /hooks/{point}", "a unit's hook"),
  ("POST /promise-readings", "a reading that keeps nothing"),
  ("POST /read-mark-counts", "a count that keeps nothing"),
  ("PUT /accounts/{account}/identity", "an account's own identity"),
  ("PUT /draft-processes/{provider}/{tier}/{agentName}", "an agent's setting"),
  ("DELETE /draft-processes/{provider}/{tier}/{agentName}", "an agent's setting"),
  ("PUT /draft-questions/{agentName}/{name}", "an agent's setting"),
  ("DELETE /draft-questions/{agentName}/{name}", "an agent's setting"),
  ("POST /supervisors", "a supervisor added, its room made with it"),
  (
    "DELETE /supervisors/{supervisorName}",
    "a supervisor retired, its room left on the record",
  ),
  ("POST /supervisors/{supervisorName}/resets", "an agent's reset"),
  ("PUT /default-answerer", "the deployment's setting"),
  ("PUT /chat/rooms/{room_id}/messages/{message_id}", "a chat service's hand-over"),
  ("PATCH /chat/rooms/{room_id}/messages/{message_id}", "a chat service's hand-over"),
  ("POST /chat/avatar-changes", "a chat service's hand-over"),
  ("POST /chat/reaction-changes", "a chat service's hand-over"),
  ("POST /chat/deletions", "a chat service's hand-over"),
];

// The writes that name their room in the body or under another name than `roomId`, each as it is asked of `room`.
fn named_elsewhere(route: &str, room: i64) -> Option<Write> {
  let form = |fields: &str| Some(Value::String(fields.to_owned()));

  Some(match route {
    "PUT /pinned-rooms" => ("PUT", "/pinned-rooms".to_owned(), Some(json!({ "roomIds": [room] }))),
    "POST /pinned-rooms" => ("POST", "/pinned-rooms".to_owned(), Some(json!({ "room": room }))),
    "DELETE /pinned-rooms/{room}" => ("DELETE", format!("/pinned-rooms/{room}"), None),
    "PUT /avatars/{kind}/{subject}" => ("PUT", format!("/avatars/room/{room}"), form("")),
    "DELETE /avatars/{kind}/{subject}" => ("DELETE", format!("/avatars/room/{room}"), None),
    "POST /owner/messages" => {
      ("POST", "/owner/messages".to_owned(), Some(json!({ "chatId": room, "text": "hi" })))
    }
    "POST /owner/instructions" => (
      "POST",
      "/owner/instructions".to_owned(),
      Some(json!({ "chatId": room, "text": "do it" })),
    ),
    "POST /owner/message-files" => ("POST", "/owner/message-files".to_owned(), form(&format!("{room}"))),
    _ => return None,
  })
}

// A write asked as the console relay asks it for an account, a multipart form where its body is text: the form
// names the room as `chatId` and carries one file.
async fn relayed_any(port: &Router, (method, path, body): Write, account: i64) -> (StatusCode, Value) {
  let (kind, body) = match body {
    Some(Value::String(room)) => (
      "multipart/form-data; boundary=own",
      Body::from(format!(
        "--own\r\ncontent-disposition: form-data; name=\"chatId\"\r\n\r\n{room}\r\n\
         --own\r\ncontent-type: text/plain\r\ncontent-disposition: form-data; name=\"file\"; filename=\"a.txt\"\r\n\r\nwords\r\n\
         --own--\r\n"
      )),
    ),
    Some(json) => ("application/json", Body::from(json.to_string())),
    None => ("application/json", Body::empty()),
  };
  let request = Request::builder()
    .method(method)
    .uri(&path)
    .header("authorization", format!("Bearer {TOKEN}"))
    .header("content-type", kind)
    .header("owner-key", account.to_string())
    .header("owner-role", "administrator")
    .body(body)
    .unwrap();
  let answer = port.clone().oneshot(request).await.unwrap();
  let status = answer.status();
  let written = to_bytes(answer.into_body(), usize::MAX).await.unwrap();

  (status, serde_json::from_slice(&written).unwrap_or(Value::Null))
}

#[tokio::test]
async fn every_route_that_changes_a_room_refuses_another_accounts_private_room() {
  let stood = stood(false).await;
  let hers = said(&stood.port, BEAS_ROOM, BEA, "Bea", "only for my BMO").await;

  every_write_refused(&stood, hers).await;
}

#[tokio::test]
async fn a_retired_bmos_room_stays_its_accounts_and_every_other_write_is_refused() {
  let stood = stood(false).await;
  let hers = said(&stood.port, BEAS_ROOM, BEA, "Bea", "only for my BMO").await;

  retire_beas_bmo(&stood);

  assert_eq!(
    asked(&stood.port, "GET", &format!("/rooms/{BEAS_ROOM}"), None).await.1["privateOf"],
    json!(BEA),
    "the record keeps whose it is"
  );

  every_write_refused(&stood, hers).await;

  // No other account reads it either: the owner's supervisor is no member's of a private room, and it stands among
  // no other account's rooms.
  let (status, refusal) =
    asked(&stood.port, "GET", &format!("/supervisors/{AGENT}/rooms/{BEAS_ROOM}/messages"), None).await;

  assert_eq!(
    (status, refusal["code"].clone()),
    (StatusCode::FORBIDDEN, json!("not-a-group")),
    "{refusal}"
  );

  for other in [OWNERS, CAL, ADMIN] {
    let rooms = asked(&stood.port, "GET", &format!("/accounts/{other}/rooms"), None).await.1;

    assert!(
      !rooms["rooms"].as_array().unwrap().contains(&json!(BEAS_ROOM)),
      "account {other}: {rooms}"
    );
  }

  assert_eq!(
    asked(&stood.port, "GET", &format!("/accounts/{BEA}/rooms"), None).await.1,
    json!({ "rooms": [BEAS_ROOM] }),
    "it stays hers"
  );
}

// Bea's supervisor retired: the agency lists the owner's own supervisor alone.
fn retire_beas_bmo(stood: &Stood) {
  let mut own = registration(AGENT, SENDER, vec![ADDRESS], None);

  own.row.insert("ownerAccount".to_owned(), json!(OWNERS));
  stood.standing.agency.registering(vec![own]);
  stood.standing.receiver.forget_agents();
}

// Every route that changes a room asked for the administrator's account in Bea's private room, each refused, and
// nothing of them kept.
async fn every_write_refused(stood: &Stood, hers: i64) {
  let before = lines(stood, BEAS_ROOM).await;
  let mut judged = 0;

  for route in bmo_receiver::routes::served() {
    let (method, path) = route.split_once(' ').unwrap();

    if method == "GET" || route == "POST /warmer/ping" || UNIT_WRITES.iter().any(|(unit, _)| *unit == route)
    {
      continue;
    }

    let write = if path.contains("{roomId}") {
      let path = path
        .replace("{roomId}", &BEAS_ROOM.to_string())
        .replace("{messageId}", &hers.to_string())
        .replace("{account}", &CAL.to_string());

      (method_of(method), path, Some(json!({})))
    } else if let Some(write) = named_elsewhere(&route, BEAS_ROOM) {
      write
    } else {
      panic!("{route} changes something and is neither judged by the write guard nor a unit's own write");
    };

    let (status, refusal) = relayed_any(&stood.port, write, ADMIN).await;

    assert_eq!(
      (status, refusal["code"].clone()),
      (StatusCode::FORBIDDEN, json!("private-room")),
      "{route} for the administrator's account in Bea's private room: {refusal}"
    );
    judged += 1;
  }

  assert!(judged >= 29, "{judged} routes judged");
  assert_eq!(lines(stood, BEAS_ROOM).await, before, "nothing refused was kept");
  assert_eq!(
    asked(&stood.port, "GET", &format!("/rooms/{BEAS_ROOM}"), None).await.0,
    StatusCode::OK,
    "nor was the room removed"
  );
}

// Every row the record keeps of whose private room a room is, as it stands.
async fn kept_private(stood: &Stood) -> Vec<String> {
  stood
    .scratch
    .texts(
      r#"SELECT "chat_id" || ' ' || coalesce("account"::text, 'sealed') || ' ' || "written_at" FROM "private_rooms"
         ORDER BY "chat_id""#,
    )
    .await
}

#[tokio::test]
async fn the_pass_at_start_keeps_every_private_room_once() {
  let stood = stood(false).await;
  let kept = kept_private(&stood).await;

  assert_eq!(kept.len(), 2, "the owner's private room and Bea's: {kept:?}");

  private_rooms::at_start(&stood.standing.receiver).await.unwrap();

  assert_eq!(kept_private(&stood).await, kept, "a second pass changes nothing");
}

#[tokio::test]
async fn a_store_from_before_the_private_rooms_were_kept_has_them_filled() {
  let stood = stood(false).await;
  let db = stood.scratch.db();

  said(&stood.port, BEAS_ROOM, BEA, "Bea", "only for my BMO").await;
  // Down to before the private rooms' table, the one migration after it with it.
  Migrator::down(db, Some(2)).await.unwrap();
  assert_eq!(migrate(db).await.unwrap(), 2, "the private rooms' table is made again, empty");
  assert!(kept_private(&stood).await.is_empty());

  // Kept again by the first start after the upgrade, Bea's room stays hers once her supervisor is gone.
  private_rooms::at_start(&stood.standing.receiver).await.unwrap();
  retire_beas_bmo(&stood);
  private_rooms::at_start(&stood.standing.receiver).await.unwrap();

  let rooms = kept_private(&stood).await;

  assert!(rooms.iter().any(|row| row.starts_with(&format!("{BEAS_ROOM} {BEA} "))), "{rooms:?}");
  assert!(rooms.iter().any(|row| row.starts_with(&format!("{OWNER} {OWNERS} "))), "{rooms:?}");

  let (status, refusal) = says(&stood.port, BEAS_ROOM, CAL, "Cal", "let me in").await;

  assert_eq!((status, refusal["code"].clone()), (StatusCode::FORBIDDEN, json!("private-room")));
}

#[tokio::test]
async fn the_owners_private_room_is_sealed_until_his_account_is_linked() {
  let stood = stood_linked(false, false).await;
  let read = |port: Router| async move {
    let one = asked(&port, "GET", &format!("/rooms/{OWNER}"), None).await.1;
    let listed = asked(&port, "GET", "/rooms", None).await.1["items"]
      .as_array()
      .unwrap()
      .iter()
      .find(|item| item["id"] == json!(OWNER))
      .cloned();

    (one["privateOf"].clone(), one["sealed"].clone(), listed)
  };

  stood.scratch.message(OWNER, 1, OWNER).await;

  let (private_of, sealed, listed) = read(stood.port.clone()).await;

  assert_eq!((private_of, sealed), (Value::Null, json!(true)));
  assert_eq!(
    listed.map(|item| (item["privateOf"].clone(), item["sealed"].clone())),
    Some((Value::Null, json!(true)))
  );

  for account in [OWNERS, BEA] {
    let (status, refusal) = says(&stood.port, OWNER, account, "Someone", "hello").await;

    assert_eq!(
      (status, refusal["code"].clone()),
      (StatusCode::FORBIDDEN, json!("private-room")),
      "no account writes in it yet: {refusal}"
    );
  }

  stood.standing.web.linking(OWNER, OWNERS.to_string());

  let (private_of, sealed, listed) = read(stood.port.clone()).await;

  assert_eq!((private_of, sealed), (json!(OWNERS), json!(false)));
  assert_eq!(
    listed.map(|item| (item["privateOf"].clone(), item["sealed"].clone())),
    Some((json!(OWNERS), json!(false)))
  );
  assert_eq!(says(&stood.port, OWNER, OWNERS, "Owner", "hello").await.0, StatusCode::CREATED);
}

fn method_of(method: &str) -> &'static str {
  match method {
    "POST" => "POST",
    "PUT" => "PUT",
    "PATCH" => "PATCH",
    "DELETE" => "DELETE",
    other => panic!("no write is asked by {other}"),
  }
}

#[tokio::test]
async fn a_forward_out_of_another_accounts_private_room_is_refused() {
  let stood = stood(false).await;
  let hers = said(&stood.port, BEAS_ROOM, BEA, "Bea", "only for my BMO").await;
  let room = group(&stood.port, CAL).await;
  let (status, refusal) = asked(
    &stood.port,
    "POST",
    &format!("/rooms/{BEAS_ROOM}/messages/{hers}/forwards"),
    Some(json!({ "to": room, "account": CAL })),
  )
  .await;

  assert_eq!((status, refusal["code"].clone()), (StatusCode::FORBIDDEN, json!("private-room")));
  assert!(lines(&stood, room).await.is_empty(), "nothing of hers reached his room");

  let (status, answer) = asked(
    &stood.port,
    "POST",
    &format!("/rooms/{BEAS_ROOM}/messages/{hers}/forwards"),
    Some(json!({ "to": room, "account": BEA })),
  )
  .await;

  assert_eq!(status, StatusCode::CREATED, "she forwards her own: {answer}");
}

#[tokio::test]
async fn a_private_room_kept_on_its_record_is_judged_without_the_agency() {
  let stood = stood(false).await;

  said(&stood.port, BEAS_ROOM, BEA, "Bea", "only for my BMO").await;
  stood.standing.agency.going_down();
  stood.standing.receiver.forget_agents();

  let (status, refusal) = says(&stood.port, BEAS_ROOM, CAL, "Cal", "let me in").await;

  assert_eq!((status, refusal["code"].clone()), (StatusCode::FORBIDDEN, json!("private-room")));
  assert_eq!(
    asked(&stood.port, "GET", &format!("/rooms/{BEAS_ROOM}"), None).await.1["privateOf"],
    json!(BEA)
  );
  assert_eq!(
    asked(&stood.port, "GET", "/rooms", None).await.0,
    StatusCode::OK,
    "the list asks no agency"
  );
  assert!(stood.standing.receiver.roads().room_facts(BEAS_ROOM).await.one_to_one());
}

#[tokio::test]
async fn no_read_assigns_a_room_on_what_the_agency_lists() {
  let stood = stood(false).await;

  // Bea's room as if her supervisor had been given its owner after the first start: the record keeps nothing of it.
  stood
    .scratch
    .run(r#"DELETE FROM "private_rooms" WHERE "chat_id" = $1"#, vec![BEAS_ROOM.into()])
    .await;

  assert_eq!(
    asked(&stood.port, "GET", &format!("/rooms/{BEAS_ROOM}"), None).await.1["privateOf"],
    Value::Null
  );
  assert_eq!(says(&stood.port, BEAS_ROOM, CAL, "Cal", "hello").await.0, StatusCode::CREATED);
  private_rooms::at_start(&stood.standing.receiver).await.unwrap();
  assert!(
    !kept_private(&stood).await.iter().any(|row| row.starts_with(&format!("{BEAS_ROOM} "))),
    "a start after the first assigns nothing"
  );
}

// A supervisor's own room as one retired before the record kept private rooms leaves it: standing on the record
// under its name, its number there, what was said in it, and no row of whose it is.
const RETIRED: &str = "u9";
const RETIREDS_ROOM: i64 = -9000000000070;

async fn retired_room(stood: &Stood) {
  for (statement, values) in [
    (
      r#"INSERT INTO "provider_rooms" ("provider", "room_id", "chat_id", "title") VALUES ('record', $1, $2, 'Dee and her BMO')"#,
      vec![RETIRED.into(), RETIREDS_ROOM.into()],
    ),
    (
      r#"INSERT INTO "provider_users" ("provider", "user_id", "sender_id") VALUES ('record', $1, -960)"#,
      vec![RETIRED.into()],
    ),
    (
      r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES ($1, 'Dee and her BMO')"#,
      vec![RETIREDS_ROOM.into()],
    ),
  ] {
    stood.scratch.run(statement, values).await;
  }

  stood.scratch.message(RETIREDS_ROOM, 1, -961).await;
}

// A supervisor added as the console relays it for an account in a role.
async fn added(stood: &Stood, body: Value, account: i64, role: &str) -> (StatusCode, Value) {
  relayed(&stood.port, ("POST", "/supervisors".to_owned(), Some(body)), account, role).await
}

#[tokio::test]
async fn a_member_re_adding_a_retired_bmo_s_name_gets_nothing_of_its_room() {
  let stood = stood(false).await;

  retired_room(&stood).await;

  let body = json!({ "name": RETIRED, "label": "Mine now", "room": "Mine", "ownerAccount": CAL });
  let (status, refusal) = added(&stood, body, CAL, "member").await;

  assert_eq!(
    (status, refusal["code"].clone()),
    (StatusCode::CONFLICT, json!("name-taken")),
    "{refusal}"
  );
  assert!(
    stood.standing.agency.added_rows.lock().unwrap().is_empty(),
    "the agency was asked nothing"
  );

  let room = asked(&stood.port, "GET", &format!("/rooms/{RETIREDS_ROOM}"), None).await.1;

  assert_eq!(
    (room["privateOf"].clone(), room["sealed"].clone()),
    (Value::Null, json!(true)),
    "{room}"
  );

  let (status, refusal) = says(&stood.port, RETIREDS_ROOM, CAL, "Cal", "let me read").await;

  assert_eq!((status, refusal["code"].clone()), (StatusCode::FORBIDDEN, json!("private-room")));

  // Were the agency to list the name for him now, no start assigns the room: the one adopting pass is past.
  let mut his = registration(RETIRED, "Mine now", vec![RETIRED], None);
  let mut own = registration(AGENT, SENDER, vec![ADDRESS], None);

  his.row.insert("ownerAccount".to_owned(), json!(CAL));
  own.row.insert("ownerAccount".to_owned(), json!(OWNERS));
  stood.standing.agency.registering(vec![own, his]);
  stood.standing.receiver.forget_agents();
  private_rooms::at_start(&stood.standing.receiver).await.unwrap();

  let room = asked(&stood.port, "GET", &format!("/rooms/{RETIREDS_ROOM}"), None).await.1;

  assert_eq!(
    (room["privateOf"].clone(), room["sealed"].clone()),
    (Value::Null, json!(true)),
    "{room}"
  );

  let (status, refusal) = asked(
    &stood.port,
    "GET",
    &format!("/supervisors/{RETIRED}/rooms/{RETIREDS_ROOM}/messages"),
    None,
  )
  .await;

  assert_eq!(
    (status, refusal["code"].clone()),
    (StatusCode::FORBIDDEN, json!("not-a-group")),
    "{refusal}"
  );
}

#[tokio::test]
async fn the_start_seals_a_retired_bmo_s_room_no_supervisor_claims() {
  let stood = stood(false).await;

  retired_room(&stood).await;
  private_rooms::at_start(&stood.standing.receiver).await.unwrap();

  assert!(
    kept_private(&stood)
      .await
      .iter()
      .any(|row| row.starts_with(&format!("{RETIREDS_ROOM} sealed "))),
    "{:?}",
    kept_private(&stood).await
  );
}

#[tokio::test]
async fn an_account_adds_a_supervisor_for_itself_and_the_administrator_for_anyone() {
  let stood = stood(false).await;
  let body = json!({ "name": "agent-c", "label": "Agent C", "room": "Room C", "ownerAccount": BEA });
  let (status, refusal) = added(&stood, body.clone(), CAL, "member").await;

  assert_eq!((status, refusal["code"].clone()), (StatusCode::FORBIDDEN, json!("not-owners-account")));
  assert!(stood.standing.agency.added_rows.lock().unwrap().is_empty());

  let (status, answer) = added(&stood, body, ADMIN, "administrator").await;

  assert_eq!(status, StatusCode::CREATED, "{answer}");

  let room = answer["room"]["id"].as_i64().unwrap();

  assert_eq!(
    asked(&stood.port, "GET", &format!("/rooms/{room}"), None).await.1["privateOf"],
    json!(BEA)
  );
}

#[tokio::test]
async fn an_addition_whose_room_half_fails_asks_the_agency_nothing() {
  let stood = stood(false).await;

  // A number of the record already standing under the name, which the room half cannot write again.
  stood
    .scratch
    .run(
      r#"INSERT INTO "provider_users" ("provider", "user_id", "sender_id") VALUES ('record', 'agent-c', -970)"#,
      vec![],
    )
    .await;
  stood.scratch.message(-9000000000080, 1, -970).await;

  let body = json!({ "name": "agent-c", "label": "Agent C", "room": "Room C", "ownerAccount": BEA });
  let (status, answer) = added(&stood, body, BEA, "member").await;

  assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{answer}");
  assert!(
    stood.standing.agency.added_rows.lock().unwrap().is_empty(),
    "the agency holds no row of it"
  );
  assert_eq!(
    stood
      .scratch
      .count(r#"SELECT count(*) FROM "provider_rooms" WHERE "room_id" = 'agent-c'"#, vec![])
      .await,
    0,
    "and the room made for it is taken back"
  );
}

// The owner's own road into a room, as the console's owner login hands it over, naming an account where one is
// given.
async fn owners_words(stood: &Stood, room: i64, account: Option<i64>) -> (StatusCode, Value) {
  let body = json!({ "chatId": room, "text": "the owner speaks" });

  match account {
    Some(account) => {
      relayed(
        &stood.port,
        ("POST", "/owner/messages".to_owned(), Some(body)),
        account,
        "administrator",
      )
      .await
    }
    None => asked(&stood.port, "POST", "/owner/messages", Some(body)).await,
  }
}

#[tokio::test]
async fn while_no_account_is_linked_to_the_owner_he_speaks_on_his_own_road_as_before() {
  let stood = stood_linked(false, false).await;
  let room = group(&stood.port, BEA).await;

  for account in [None, Some(OWNERS)] {
    assert_eq!(
      owners_words(&stood, OWNER, account).await.0,
      StatusCode::OK,
      "his private room, {account:?}"
    );
    assert_eq!(owners_words(&stood, room, account).await.0, StatusCode::OK, "a group room, {account:?}");
  }

  let (status, refusal) = owners_words(&stood, BEAS_ROOM, None).await;

  assert_eq!(
    (status, refusal["code"].clone()),
    (StatusCode::FORBIDDEN, json!("private-room")),
    "{refusal}"
  );
  assert_eq!(
    says(&stood.port, OWNER, OWNERS, "Owner", "as an account").await.0,
    StatusCode::FORBIDDEN,
    "an account's words in his sealed room are refused until the link names it"
  );

  stood.standing.web.linking(OWNER, OWNERS.to_string());

  assert_eq!(owners_words(&stood, OWNER, None).await.0, StatusCode::OK);
  assert_eq!(says(&stood.port, OWNER, OWNERS, "Owner", "as his account").await.0, StatusCode::CREATED);
  assert_eq!(
    asked(&stood.port, "GET", &format!("/rooms/{OWNER}"), None).await.1["privateOf"],
    json!(OWNERS)
  );
}

#[tokio::test]
async fn the_owners_road_into_a_room_of_no_account_asks_no_web_unit() {
  let stood = stood(false).await;
  let room = group(&stood.port, BEA).await;

  stood.standing.web.going_down();

  assert_eq!(owners_words(&stood, room, None).await.0, StatusCode::OK, "a group room is no one's");
  assert_eq!(owners_words(&stood, OWNER, None).await.0, StatusCode::OK, "his own private room is his");
  assert_eq!(asked(&stood.port, "GET", "/rooms", None).await.0, StatusCode::OK);
}

#[tokio::test]
async fn the_owners_private_chat_on_a_chat_service_is_his_private_room() {
  const DM: i64 = 4242;

  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let road = StandInChat::new("road-a").holding(&[DM]).owners(DM).agent_in(DM, 900);

  road.moding(ChatVoice::HumanReader, ChatModeStanding::Active);

  let standing = standing(scratch.db(), DataRoot::new(data.path()), vec![Arc::new(road)]);

  standing.web.linking(OWNER, OWNERS.to_string());
  scratch.message(DM, 1, OWNER).await;

  let port = routes(standing.receiver.clone(), TOKEN);

  assert_ne!(DM, standing.receiver.owner_room(), "the configured owner's room is another");
  assert_eq!(asked(&port, "GET", &format!("/rooms/{DM}"), None).await.1["privateOf"], json!(OWNERS));

  let (status, refusal) = says(&port, DM, BEA, "Bea", "hello").await;

  assert_eq!(
    (status, refusal["code"].clone()),
    (StatusCode::FORBIDDEN, json!("private-room")),
    "{refusal}"
  );
}

#[tokio::test]
async fn the_dry_run_writes_nothing_and_says_how_every_room_would_settle() {
  let stood = stood(false).await;

  retired_room(&stood).await;

  let before = everything(&stood.scratch).await;
  let units = stood.standing.receiver.units();
  let asked = Asked {
    agency: Some(units.agency.as_ref()),
    web: Some(units.web.as_ref()),
  };
  let settled = private_rooms::settle(stood.scratch.db(), (OWNER, OWNER), asked, false).await.unwrap();
  let line = |room: i64| settled.iter().find(|one| one.room == room).map(|one| one.verdict.clone());

  assert_eq!(everything(&stood.scratch).await, before, "nothing written");
  assert_eq!(
    line(BEAS_ROOM),
    Some(private_rooms::Verdict::Kept(bmo_receiver::accounts::Privacy::Of(BEA)))
  );
  assert_eq!(line(RETIREDS_ROOM), Some(private_rooms::Verdict::Seal));
  assert_eq!(line(LINKED), Some(private_rooms::Verdict::Shared));

  let unasked = private_rooms::settle(stood.scratch.db(), (OWNER, OWNER), Asked::default(), false)
    .await
    .unwrap();

  assert_eq!(
    unasked.iter().find(|one| one.room == RETIREDS_ROOM).map(|one| one.verdict.clone()),
    Some(private_rooms::Verdict::Unknown),
    "without the agency a room the record keeps nothing of is left unknown"
  );
  assert_eq!(everything(&stood.scratch).await, before);
}

#[tokio::test]
async fn an_ownerless_persons_bmo_is_adopted_by_the_account_its_name_names() {
  let stood = stood(false).await;
  let db = stood.scratch.db();
  let names = [
    ("u7", -9000000000091),
    ("agent-x", -9000000000092),
    ("u7x", -9000000000093),
    ("u07", -9000000000094),
  ];

  for (place, (name, room)) in names.iter().enumerate() {
    let sender = -980 - place as i64;

    stood
      .scratch
      .run(
        r#"INSERT INTO "provider_rooms" ("provider", "room_id", "chat_id", "title") VALUES ('record', $1, $2, $1)"#,
        vec![(*name).into(), (*room).into()],
      )
      .await;
    stood
      .scratch
      .run(
        r#"INSERT INTO "provider_users" ("provider", "user_id", "sender_id") VALUES ('record', $1, $2)"#,
        vec![(*name).into(), sender.into()],
      )
      .await;
  }

  // Each registered with no owner, as a registration made before the owner account was handed on.
  let mut own = registration(AGENT, SENDER, vec![ADDRESS], None);

  own.row.insert("ownerAccount".to_owned(), json!(OWNERS));

  let listed = std::iter::once(own)
    .chain(names.iter().map(|(name, _)| registration(name, name, vec![name], None)))
    .collect();

  stood.standing.agency.registering(listed);
  stood.standing.receiver.forget_agents();

  // The first start after the record began keeping private rooms.
  Migrator::down(db, Some(2)).await.unwrap();
  migrate(db).await.unwrap();

  let settled = private_rooms::at_start(&stood.standing.receiver).await.unwrap();
  let kept = kept_private(&stood).await;

  assert!(kept.iter().any(|row| row.starts_with(&format!("{} 7 ", names[0].1))), "{kept:?}");

  for (name, room) in &names[1..] {
    assert!(
      !kept.iter().any(|row| row.starts_with(&format!("{room} "))),
      "{name}'s room is no one's: {kept:?}"
    );
  }

  let why = settled.iter().find(|one| one.room == names[0].1).map(|one| one.why.clone()).unwrap();

  assert!(why.contains("owner from name"), "{why}");
  assert_eq!(private_rooms::owner_from_name("u7"), Some(7));
  assert_eq!(private_rooms::owner_from_name("u07"), None, "a leading zero names no account");
  assert_eq!(private_rooms::owner_from_name("u0"), None);
  assert_eq!(private_rooms::owner_from_name("u"), None);
  assert_eq!(private_rooms::owner_from_name("u-7"), None);
}

//! The console's accounts in its rooms (docs/receiver.md, 「Console rooms and accounts」): words another account
//! says carry nothing granted to the owner, the owner's account keeps all of it as his own road does, a provider
//! speaks as the account only where it says it can, a room's members by account come and go with a line saying so,
//! an account's record identity is made once, a BMO's private room wakes on its account's words alone and refuses
//! every other, a read of it leaves nothing behind, and a messenger's member is answered as before.

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
use bmo_receiver::receiver::{Context, How, Inbound, Room};
use bmo_receiver::routes::routes;
use bmo_receiver::verbs::VerbError;
use bmo_receiver::verbs::room_rules::{self, Change, Form, Subject};
use receiver_codes::RoomGrantRefusal;

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
  standing.web.linking(OWNER, OWNERS.to_string());

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

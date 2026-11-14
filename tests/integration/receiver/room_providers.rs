//! A room's chat providers (docs/receiver.md, 「A room's chat providers」; docs/api.md, 「Rooms」): the providers a
//! room may be on, a room's list read off what it holds until it is set whole, every kept message carried to each
//! provider of the room it does not already stand on, and a provider's echo of what was carried to it taken for
//! the message already kept.

use std::sync::Arc;

use axum::{Router, body::Body, extract::Request, http::StatusCode};
use bmo_chat_client::{ChatModeStanding, ChatSend, ChatVoice};
use bmo_receiver::roads::Road;
use bmo_receiver::roads::providers::{self, Carried, Delivery};
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{Front, TOKEN, get, post, put, read, stood_on};
use crate::integration::common::units::{OWNER, StandInChat};

const ROOM: i64 = -100;

// A front reaching one chat service that holds the room, the room titled so the receiver holds it.
async fn front(scratch: &Scratch) -> (Front, Arc<StandInChat>) {
  declaring(scratch, &[]).await
}

// The same front, its chat service declaring the read modes given, each by its voice and where it stands.
async fn declaring(
  scratch: &Scratch,
  modes: &[(ChatVoice, ChatModeStanding)],
) -> (Front, Arc<StandInChat>) {
  let road = Arc::new(StandInChat::new("road-a").holding(&[ROOM]).owners(OWNER));

  for (voice, standing) in modes {
    road.moding(*voice, *standing);
  }

  let front = stood_on(scratch, OWNER, vec![road.clone()]).await;

  scratch
    .run(r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES (-100, 'Alpha Room')"#, vec![])
    .await;

  (front, road)
}

// The chat services' hand-over routes over the same receiver.
fn handovers(front: &Front) -> Router {
  bmo_receiver::receiver::handover::routes(front.receiver(), TOKEN)
}

// A message handed over on those routes; the status answered.
async fn hand(routes: &Router, message: i64, body: Value) -> StatusCode {
  let asked = Request::put(format!("/chat/rooms/{ROOM}/messages/{message}"))
    .header("authorization", format!("Bearer {TOKEN}"))
    .header("content-type", "application/json")
    .body(Body::from(body.to_string()))
    .unwrap();

  read(routes.clone().oneshot(asked).await.unwrap()).await.0
}

// A message the chat service hands over, said live in the room.
fn handed(text: &str, tag: Option<&str>) -> Value {
  json!({
    "origin": "live",
    "senderId": 55,
    "senderName": "tester",
    "sentAt": 1_760_000_000_000_i64,
    "text": text,
    "replyToMessageId": null,
    "via": "road-a",
    "album": null,
    "mode": "bot-account",
    "context": { "tag": tag },
    "room": { "kind": "group" },
  })
}

#[tokio::test]
async fn the_providers_are_the_record_and_each_chat_service_and_a_rooms_list_is_set_whole() {
  let scratch = Scratch::new().await;
  let (front, _road) = front(&scratch).await;

  let (status, every) = get(&front, "/providers").await;
  let keys: Vec<&str> = every["items"]
    .as_array()
    .unwrap()
    .iter()
    .filter_map(|one| one["key"].as_str())
    .collect();

  assert_eq!(status.as_u16(), 200, "{every}");
  assert_eq!(keys, ["record", "road-a"]);
  assert!(every["items"].as_array().unwrap().iter().all(|one| one["label"].is_string()));

  assert_eq!(
    get(&front, "/rooms/-100/providers").await.1,
    json!({ "providers": ["road-a"], "chosen": false }),
    "a room never set is on the chat service that holds it",
  );

  for refused in [
    json!({ "providers": [] }),
    json!({ "providers": ["road-z"] }),
    json!({ "providers": ["record", "record"] }),
  ] {
    let (status, body) = put(&front, "/rooms/-100/providers", refused.clone()).await;

    assert_eq!((status.as_u16(), body["code"].as_str()), (400, Some("bad-request")), "{refused}");
  }

  assert_eq!(get(&front, "/rooms/-7/providers").await.1["code"], "not-found");
  assert_eq!(
    put(&front, "/rooms/-100/providers", json!({ "providers": ["record"] })).await.1,
    json!({ "providers": ["record"], "chosen": true }),
  );
  assert_eq!(
    get(&front, "/rooms/-100/providers").await.1,
    json!({ "providers": ["record"], "chosen": true })
  );
}

#[tokio::test]
async fn a_kept_message_goes_to_every_provider_of_its_room_but_the_ones_it_stands_on() {
  let scratch = Scratch::new().await;
  let (front, road) = front(&scratch).await;
  let receiver = front.receiver();

  // A line the record keeps goes on to the chat service, tagged and in the service's own voice.
  receiver.speech().kept(ROOM, "kept in the record", None, None, None).await.unwrap();

  let sent = road.sent();

  assert_eq!(sent.len(), 1, "{sent:?}");
  assert_eq!(
    sent[0].send,
    ChatSend::Text {
      text: "kept in the record".to_owned(),
      silent: false,
      fold: false
    }
  );
  assert!(sent[0].tag.is_some(), "the send names its delivery");
  assert_eq!(sent[0].mode, None);

  // A message the service handed over already stands there, and the record holds it by being the archive.
  assert_eq!(hand(&handovers(&front), 5, handed("said there", None)).await, StatusCode::CREATED);
  assert_eq!(road.sent().len(), 1, "nothing goes back to the road a message came in on");

  // A copy the road already carried is not carried to it again.
  receiver
    .speech()
    .kept(ROOM, "sent on the road", Some(("road-a", Some(77))), None, None)
    .await
    .unwrap();
  assert_eq!(road.sent().len(), 1, "a line the road already carried goes to it no second time");

  // A room on the record alone reaches no chat service.
  put(&front, "/rooms/-100/providers", json!({ "providers": ["record"] })).await;
  receiver.speech().kept(ROOM, "for the console alone", None, None, None).await.unwrap();
  assert_eq!(road.sent().len(), 1, "a room on the record alone reaches no chat service");
}

#[tokio::test]
async fn a_providers_echo_of_a_carried_message_is_the_message_already_kept() {
  let scratch = Scratch::new().await;
  let (front, road) = front(&scratch).await;
  let (status, said) =
    post(&front, "/owner/messages", json!({ "chatId": ROOM, "text": "From the console." })).await;

  assert_eq!(status.as_u16(), 200, "{said}");

  let sent = road.sent();

  assert_eq!(sent.len(), 1, "{sent:?}");
  assert_eq!(
    sent[0].mode,
    Some(ChatVoice::HumanReader),
    "the owner's words go in the owner's own voice"
  );

  let tag = sent[0].tag.clone().unwrap();
  let routes = handovers(&front);

  // The service's copy under the number it gave the send, and one naming the send's tag.
  let by_number = hand(&routes, 900, handed("From the console.", None)).await;
  let by_tag = hand(&routes, 901, handed("From the console.", Some(&tag))).await;
  let other = hand(&routes, 902, handed("Another line.", None)).await;

  assert_eq!((by_number, by_tag, other), (StatusCode::OK, StatusCode::OK, StatusCode::CREATED));
  assert_eq!(
    scratch
      .texts(r#"SELECT "message_id"::text FROM "messages" WHERE "chat_id" = -100 ORDER BY "rowid""#)
      .await,
    [said["id"].to_string(), "902".to_owned()],
    "the echoes are the message already kept, and are kept no second time",
  );
  assert!(
    front
      .standing
      .waking
      .woken()
      .iter()
      .all(|woken| !format!("{woken:?}").contains("message_id: 900")
        && !format!("{woken:?}").contains("message_id: 901")),
    "an echo wakes nobody",
  );
}

// The agent and voice carried on a message, as the row keeps them: the agent whose words these are and the
// voice the send went out in, or `∅|∅` where the row names neither.
async fn voiced(scratch: &Scratch, message: i64) -> String {
  scratch
    .texts(&format!(
      r#"SELECT COALESCE("agent_name", '∅') || '|' || COALESCE("voice", '∅')
         FROM "messages" WHERE "chat_id" = {ROOM} AND "message_id" = {message}"#
    ))
    .await
    .pop()
    .unwrap_or_default()
}

#[tokio::test]
async fn a_send_the_receiver_archives_itself_in_the_human_avatar_voice_names_its_agent_and_voice() {
  let scratch = Scratch::new().await;
  let (front, _road) = front(&scratch).await;
  let receiver = front.receiver();

  let message = receiver
    .speech()
    .kept(ROOM, "answered as the owner", None, Some("agent-a"), Some(ChatVoice::HumanAvatar))
    .await
    .unwrap();

  assert_eq!(
    voiced(&scratch, message).await,
    "agent-a|human-avatar",
    "a send the receiver archives names the agent whose words these are and the voice it went out in",
  );
}

#[tokio::test]
async fn a_message_a_provider_hands_over_names_no_agent_or_voice() {
  let scratch = Scratch::new().await;
  let (front, _road) = front(&scratch).await;

  assert_eq!(hand(&handovers(&front), 5, handed("said in the room", None)).await, StatusCode::CREATED);
  assert_eq!(
    voiced(&scratch, 5).await,
    "∅|∅",
    "a person's message that came in from a provider names neither agent nor voice",
  );
}

#[tokio::test]
async fn a_providers_echo_of_the_receivers_own_send_keeps_the_sends_agent_and_voice() {
  let scratch = Scratch::new().await;
  let (front, _road) = front(&scratch).await;
  let receiver = front.receiver();

  // The receiver archives its own human-avatar send and stands it on the road under the number it went out as.
  let message = receiver
    .speech()
    .kept(
      ROOM,
      "answered as the owner",
      Some(("road-a", Some(77))),
      Some("agent-a"),
      Some(ChatVoice::HumanAvatar),
    )
    .await
    .unwrap();

  // The road hands that send back: it is the message already kept, de-duplicated and kept no second time, so
  // the fields the send was archived with still stand.
  assert_eq!(
    hand(&handovers(&front), 77, handed("answered as the owner", None)).await,
    StatusCode::OK
  );
  assert_eq!(
    voiced(&scratch, message).await,
    "agent-a|human-avatar",
    "the echo is de-duplicated and the send keeps the agent and voice it was archived with",
  );
  assert_eq!(
    scratch
      .texts(r#"SELECT "message_id"::text FROM "messages" WHERE "chat_id" = -100 ORDER BY "rowid""#)
      .await,
    [message.to_string()],
    "the echo is kept no second time",
  );
}

#[tokio::test]
async fn a_chosen_list_rules_both_ways_and_names_the_rooms_own_road() {
  use bmo_receiver::roads::Road;

  let scratch = Scratch::new().await;
  let (front, road) = front(&scratch).await;
  let receiver = front.receiver();
  let routes = handovers(&front);
  let kept = |id: i64| {
    let scratch = &scratch;
    async move {
      scratch
        .texts(&format!(
          r#"SELECT "text" FROM "messages" WHERE "chat_id" = -100 AND "message_id" = {id}"#
        ))
        .await
    }
  };

  // The road detached: the room stands on the record alone.
  put(&front, "/rooms/-100/providers", json!({ "providers": ["record"] })).await;

  let roads = receiver.roads();

  assert_eq!(
    roads.own(ROOM).await,
    Some(Road::Record),
    "the first provider named is the room's own road"
  );
  assert_eq!(
    roads.answering(ROOM, Some("road-a")).await,
    Some(Road::Record),
    "a road off the list answers nothing"
  );
  assert_eq!(roads.answering(ROOM, Some("record")).await, Some(Road::Record));
  assert_eq!(
    roads.unasked(ROOM).await,
    Some(Road::Record),
    "a line nobody asked for stays on the record"
  );

  // What the detached road hands over is not taken, and nothing of the room goes out on it.
  assert_ne!(hand(&routes, 6, handed("said on the detached road", None)).await, StatusCode::CREATED);
  assert!(kept(6).await.is_empty(), "a message a detached road hands over is not kept");

  receiver.speech().say(ROOM, "a notice for the room").await.unwrap();
  assert!(
    road.sent().is_empty(),
    "nothing of the room goes out on a road its list does not name: {:?}",
    road.sent()
  );

  // Attached again, the road is heard and carried to, and it is the room's own road once it is first.
  put(&front, "/rooms/-100/providers", json!({ "providers": ["road-a", "record"] })).await;

  assert_eq!(hand(&routes, 7, handed("said on the road again", None)).await, StatusCode::CREATED);
  assert_eq!(kept(7).await, vec!["said on the road again".to_owned()]);
  assert_eq!(roads.own(ROOM).await, Some(Road::Service("road-a".to_owned())));
}

#[tokio::test]
async fn a_message_the_owner_forwards_is_kept_as_his_in_the_room_it_goes_into_marked_and_carried_there() {
  let scratch = Scratch::new().await;
  let (front, road) = front(&scratch).await;

  // A message said in another room the receiver holds.
  scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via")
         VALUES (-200, 5, 55, 'someone', 1, 'the words to pass on', 'record')"#,
      vec![],
    )
    .await;

  let (status, answer) = post(&front, "/rooms/-200/messages/5/forwards", json!({ "to": ROOM })).await;

  assert_eq!(status, StatusCode::CREATED, "{answer}");

  let copy = answer["id"].as_i64().unwrap();
  let (status, kept) = get(&front, &format!("/rooms/{ROOM}/messages/{copy}")).await;

  assert_eq!(status, StatusCode::OK);
  assert_eq!(kept["text"], "the words to pass on");
  assert_eq!(kept["senderId"], OWNER, "kept as the owner's");
  assert_eq!(kept["forwardedFrom"], json!({ "room": -200, "messageId": 5, "senderName": "someone" }));

  let carried = road.sent();

  assert_eq!(carried.len(), 1, "carried out on the room's own road: {carried:?}");
  assert_eq!(carried[0].mode, Some(ChatVoice::HumanReader), "in the owner's own voice");

  // Refusals: into its own room, a message not held, a room not held.
  assert_eq!(
    post(&front, "/rooms/-200/messages/5/forwards", json!({ "to": -200 })).await.0,
    StatusCode::BAD_REQUEST
  );
  assert_eq!(
    post(&front, "/rooms/-200/messages/9/forwards", json!({ "to": ROOM })).await.0,
    StatusCode::NOT_FOUND
  );
  assert_eq!(
    post(&front, "/rooms/-200/messages/5/forwards", json!({ "to": -999 })).await.0,
    StatusCode::NOT_FOUND
  );
}

#[tokio::test]
async fn a_forwarded_message_keeps_its_stored_picture_under_the_copy_and_carries_it_as_a_picture() {
  let scratch = Scratch::new().await;
  let (front, road) = front(&scratch).await;
  // A picture message of another room, its picture filed under that room.
  let folder = front.data.path().join("images/-200");
  let bytes = {
    let mut out = std::io::Cursor::new(Vec::new());

    image::RgbImage::from_pixel(8, 8, image::Rgb([10, 200, 30]))
      .write_to(&mut out, image::ImageFormat::Jpeg)
      .unwrap();
    out.into_inner()
  };

  std::fs::create_dir_all(&folder).unwrap();
  std::fs::write(folder.join("5.jpg"), &bytes).unwrap();
  scratch
    .run(
      &format!(
        r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via", "image_path", "image_state")
           VALUES (-200, 5, 55, 'someone', 1, 'a diagram', 'record', '{}', 'stored')"#,
        folder.join("5.jpg").display()
      ),
      vec![],
    )
    .await;

  let (status, answer) = post(&front, "/rooms/-200/messages/5/forwards", json!({ "to": ROOM })).await;

  assert_eq!(status, StatusCode::CREATED, "{answer}");

  let copy = answer["id"].as_i64().unwrap();
  let (status, kept) = get(&front, &format!("/rooms/{ROOM}/messages/{copy}")).await;

  assert_eq!(status, StatusCode::OK);
  assert_eq!(kept["text"], "a diagram");

  let stored = kept["imagePath"].as_str().expect("the copy stores the picture").to_owned();

  assert!(
    stored.starts_with(&front.data.path().join(format!("images/{ROOM}")).to_string_lossy().into_owned()),
    "{stored}"
  );
  assert_eq!(std::fs::read(&stored).unwrap(), bytes, "the same picture, kept again under the copy");

  let carried = road.sent();

  assert_eq!(carried.len(), 1, "{carried:?}");
  assert!(
    matches!(&carried[0].send, ChatSend::Photo { .. } | ChatSend::Album { .. }),
    "carried as a picture: {:?}",
    carried[0].send
  );
}

#[tokio::test]
async fn a_reply_to_the_services_copy_of_a_line_the_record_numbered_answers_that_line() {
  let scratch = Scratch::new().await;
  let (front, _road) = front(&scratch).await;
  let routes = handovers(&front);

  // A line the record numbered, carried to the service, which gave its copy the number 900.
  scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via", "agent_name")
         VALUES (-100, -1, 7, 'Agent A', 1760000000, 'the answer', 'record', 'agent-a')"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "room_deliveries" ("chat_id", "message_id", "provider", "provider_message_id", "delivered_at")
         VALUES (-100, -1, 'road-a', 900, 1760000001)"#,
      vec![],
    )
    .await;

  let mut reply = handed("about that", None);

  reply["replyToMessageId"] = json!(900);

  assert!(hand(&routes, 901, reply).await.is_success());
  assert_eq!(
    scratch
      .texts(r#"SELECT "reply_to_message_id"::text FROM "messages" WHERE "chat_id" = -100 AND "message_id" = 901"#)
      .await,
    ["-1"],
    "the reply names the line under the number the room keeps it under"
  );
}

#[tokio::test]
async fn an_answer_folds_only_where_its_own_mark_asks_it() {
  use bmo_receiver::chat::reply::{OutgoingReply, send_reply};

  let scratch = Scratch::new().await;
  let (front, road) = front(&scratch).await;
  let receiver = front.receiver();
  let answer = |text: &str| OutgoingReply {
    chat_id: ROOM,
    text: text.to_owned(),
    via: Some("road-a".to_owned()),
    ..OutgoingReply::default()
  };
  let long =
    "A short lead.\n\n- the first long point of the body\n- the second long point of the body\n- the third";

  assert!(send_reply(&receiver, &answer(long)).await.is_some());
  assert!(send_reply(&receiver, &answer(&format!("{long}\n\n<<FOLD>>"))).await.is_some());
  assert!(send_reply(&receiver, &answer("Quoted: `<<FOLD>>`")).await.is_some());

  let sent: Vec<ChatSend> = road.sent().iter().map(|held| held.send.clone()).collect();

  assert_eq!(
    sent,
    [
      ChatSend::Text {
        text: long.to_owned(),
        silent: false,
        fold: false,
      },
      ChatSend::Text {
        text: long.to_owned(),
        silent: false,
        fold: true,
      },
      ChatSend::Text {
        text: "Quoted: `<<FOLD>>`".to_owned(),
        silent: false,
        fold: false,
      },
    ],
    "an unmarked answer goes out unfolded however long it runs, and the mark never reaches the room"
  );
  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "messages" WHERE "chat_id" = $1 AND "text" LIKE '%<<FOLD>>%'"#,
        vec![ROOM.into()]
      )
      .await,
    1,
    "the room's record keeps the words without the mark, a quoted one aside"
  );
}

// The voice of each send the room's chat service was handed, in order.
fn voices(road: &StandInChat) -> Vec<Option<ChatVoice>> {
  road.sent().iter().map(|sent| sent.mode).collect()
}

#[tokio::test]
async fn a_room_never_added_is_answered_in_the_one_voice_its_service_stands_on_and_the_owner_in_his_own() {
  let scratch = Scratch::new().await;
  let (front, road) = declaring(
    &scratch,
    &[
      (ChatVoice::OwnAccount, ChatModeStanding::Inactive),
      (ChatVoice::HumanAvatar, ChatModeStanding::Active),
    ],
  )
  .await;
  let receiver = front.receiver();

  // A line that names no voice goes in human-avatar, the one mode the service has active.
  let line = receiver.speech().kept(ROOM, "answered as the person", None, None, None).await.unwrap();

  assert_eq!(voices(&road), vec![Some(ChatVoice::HumanAvatar)]);

  // The owner's own words still go in his own voice, above the one the service stands on.
  let delivery = Delivery {
    chat_id: ROOM,
    message_id: line,
    reply_to: None,
    owner: true,
    standing: vec![Road::Record.key().to_owned()],
    carried: Carried::words("said by the owner"),
  };

  providers::deliver(receiver.db(), receiver.roads(), &delivery).await;
  assert_eq!(voices(&road), vec![Some(ChatVoice::HumanAvatar), Some(ChatVoice::HumanReader)]);
}

#[tokio::test]
async fn a_rooms_mode_stands_above_the_one_voice_its_service_stands_on() {
  let scratch = Scratch::new().await;
  let (front, road) = declaring(&scratch, &[(ChatVoice::HumanAvatar, ChatModeStanding::Active)]).await;

  scratch
    .run(
      r#"INSERT INTO "room_modes" ("chat_id", "mode", "added_at") VALUES (-100, 'record-only', 0)"#,
      vec![],
    )
    .await;
  front
    .receiver()
    .speech()
    .kept(ROOM, "kept for the user alone", None, None, None)
    .await
    .unwrap();

  assert_eq!(voices(&road), vec![None], "a record-only room names no voice, as it always has");
}

#[tokio::test]
async fn a_room_on_a_service_with_several_modes_active_names_no_voice() {
  let scratch = Scratch::new().await;
  let (front, road) = declaring(
    &scratch,
    &[
      (ChatVoice::HumanAvatar, ChatModeStanding::Active),
      (ChatVoice::OwnAccount, ChatModeStanding::Active),
    ],
  )
  .await;

  front
    .receiver()
    .speech()
    .kept(ROOM, "the service picks", None, None, None)
    .await
    .unwrap();

  assert_eq!(voices(&road), vec![None], "the service answers in its own voice");
}

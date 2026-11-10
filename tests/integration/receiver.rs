//! The receiver's judgement of a message (docs/receiver.md, 「Where it stands」): a message kept with its room's
//! title and handed to the wake, the agent's own words and a message already read stopped there, the owner's
//! setting commands acknowledged in the room's own voice, a file kept beside its message, an edit written in
//! place, and the chat services' hand-overs taken behind the receiver's token.

mod avatars;
mod commands;
mod console_rooms;
mod files;
mod job_queue;
mod message_files;
mod message_metadata;
mod notices;
mod owner_road;
mod owner_sends;
mod owner_words;
mod read_only_rooms;
mod read_requests;
mod record_alone_rooms;
mod room_addition;
mod room_archive;
mod room_figures;
mod room_mode;
mod room_pictures;
mod room_providers;
mod secrets;
mod send_kinds;
mod voice;
mod worker_settings;

use std::sync::Arc;

use bmo_chat_client::{GatheredPatch, MessageFile};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::{Context, How, Inbound, Room};
use receiver_policy::commands::COMMANDS;

use crate::integration::common::Scratch;
use crate::integration::common::units::{ADDRESS, OWNER, StandInChat, Standing, Woken, standing};

/// The agent's number in the rooms of these tests.
const AGENT_NUMBER: i64 = 7;

fn room() -> Room {
  Room {
    owner_room: false,
    identity: Some(AGENT_NUMBER),
    private_to: None,
  }
}

fn inbound(chat_id: i64, message_id: i64, sender_id: i64, text: &str) -> Inbound {
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
    mode: Some("bot-account".into()),
    via_reader: None,
    album: None,
  }
}

// A receiver over a scratch database and data root of its own, reaching one chat service that holds the rooms.
async fn stood(rooms: &[i64]) -> (Scratch, tempfile::TempDir, Standing) {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let chat = Arc::new(StandInChat::new("road-a").holding(rooms));
  let standing = standing(scratch.db(), DataRoot::new(data.path()), vec![chat]);

  (scratch, data, standing)
}

async fn text(scratch: &Scratch, sql: &str) -> Option<String> {
  scratch.texts(sql).await.into_iter().next()
}

// The owner's words: a phrase of the policy's, the agent's name in front and a polite ending after it.
fn said(phrase: &[String], tail: usize) -> String {
  format!("{ADDRESS}, {}{}", phrase.last().unwrap(), COMMANDS.polite_tails[tail])
}

#[tokio::test]
async fn a_message_is_kept_with_its_rooms_title_and_handed_to_the_wake() {
  let (scratch, _data, standing) = stood(&[-101]).await;
  let context = Context {
    room_title: Some("a room".into()),
    ..Context::default()
  };

  standing
    .receiver
    .message(&inbound(-101, 1, OWNER, "hello"), &context, room())
    .await
    .unwrap();

  assert_eq!(standing.heard.told(), [(-101, 1)]);
  assert_eq!(
    standing.waking.woken(),
    [Woken::Words {
      chat_id: -101,
      message_id: 1,
      owner: true,
      how: How::Words,
    }]
  );
  assert_eq!(
    text(&scratch, r#"SELECT "title" FROM "chat_titles" WHERE "chat_id" = -101"#)
      .await
      .as_deref(),
    Some("a room")
  );
  assert_eq!(
    text(&scratch, r#"SELECT "via" FROM "messages" WHERE "chat_id" = -101 AND "message_id" = 1"#)
      .await
      .as_deref(),
    Some("road-a")
  );
}

#[tokio::test]
async fn the_agents_own_words_and_a_message_already_read_are_kept_and_go_no_further() {
  let (scratch, _data, standing) = stood(&[-102]).await;
  let receiver = &standing.receiver;

  receiver
    .message(&inbound(-102, 1, AGENT_NUMBER, "my words"), &Context::default(), room())
    .await
    .unwrap();

  // The room was read up to a message sent after the one that arrives now.
  scratch
    .run(
      r#"INSERT INTO "chat_reads" ("chat_id", "last_read_message_id") VALUES (-102, 5)"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text") VALUES (-102, 5, 55, 'tester', 1760000100, 'later')"#,
      vec![],
    )
    .await;
  receiver
    .message(&inbound(-102, 4, 55, "read already"), &Context::default(), room())
    .await
    .unwrap();

  assert_eq!(standing.heard.told(), [(-102, 1), (-102, 4)]);
  assert!(standing.waking.woken().is_empty());
}

#[tokio::test]
async fn a_room_the_database_cannot_number_is_not_kept() {
  let beyond = 1_i64 << 60;
  let (scratch, _data, standing) = stood(&[beyond]).await;

  standing
    .receiver
    .message(&inbound(beyond, 1, 55, "hello"), &Context::default(), room())
    .await
    .unwrap();

  assert_eq!(scratch.count(r#"SELECT COUNT(*) FROM "messages""#, vec![]).await, 0);
  assert!(standing.heard.told().is_empty() && standing.waking.woken().is_empty());
}

#[tokio::test]
async fn the_owners_setting_command_changes_the_room_and_is_acknowledged_in_its_own_voice() {
  let (scratch, _data, standing) = stood(&[-104]).await;
  let receiver = &standing.receiver;
  let phrases = &COMMANDS.phrases;

  receiver
    .message(&inbound(-104, 1, OWNER, &said(&phrases.photos_on, 4)), &Context::default(), room())
    .await
    .unwrap();
  receiver
    .message(&inbound(-104, 2, OWNER, &said(&phrases.photos_off, 5)), &Context::default(), room())
    .await
    .unwrap();

  // Each acknowledgement is a row the instruction pass delivers, for the default answerer's session; nothing is woken.
  assert!(standing.waking.woken().is_empty());
  assert_eq!(
    scratch
      .count(r#"SELECT "look" FROM "photo_chats" WHERE "chat_id" = -104"#, vec![])
      .await,
    0
  );
  // One acknowledgement per setting: the second took the first one's place.
  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "instructions" WHERE "chat_id" = -104 AND "kind" = 'setting:photos'"#,
        vec![]
      )
      .await,
    1
  );
  assert!(
    text(&scratch, r#"SELECT "text" FROM "instructions" WHERE "chat_id" = -104"#)
      .await
      .unwrap()
      .contains("The owner switched photo viewing off for this chat alone")
  );
  assert_eq!(
    text(&scratch, r#"SELECT "agent" FROM "instructions" WHERE "chat_id" = -104"#)
      .await
      .as_deref(),
    Some("agent-a")
  );

  // A caption under a file the owner sent is not a command.
  let captioned = Context {
    carries_file: true,
    spoken_text: Some(said(&phrases.photos_on, 4)),
    ..Context::default()
  };

  receiver.message(&inbound(-104, 3, OWNER, "a file"), &captioned, room()).await.unwrap();
  assert_eq!(
    standing.waking.woken(),
    [Woken::Words {
      chat_id: -104,
      message_id: 3,
      owner: true,
      how: How::Words,
    }]
  );
}

#[tokio::test]
async fn a_mute_names_the_person_answered_and_never_the_owner_or_the_agent() {
  let (scratch, _data, standing) = stood(&[-105]).await;
  let receiver = &standing.receiver;
  let answering = |sender| Context {
    reply_to_sender_id: Some(sender),
    ..Context::default()
  };
  let mute = COMMANDS.phrases.mute.last().unwrap();
  let unmute = format!("{} 66", COMMANDS.phrases.unmute.last().unwrap());

  receiver.message(&inbound(-105, 1, OWNER, mute), &answering(55), room()).await.unwrap();
  receiver
    .message(&inbound(-105, 2, OWNER, mute), &answering(AGENT_NUMBER), room())
    .await
    .unwrap();
  receiver
    .message(&inbound(-105, 3, OWNER, &unmute), &Context::default(), room())
    .await
    .unwrap();

  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "muted_members" WHERE "chat_id" = -105 AND "user_id" = 55"#,
        vec![]
      )
      .await,
    1
  );
  assert_eq!(
    scratch
      .count(r#"SELECT COUNT(*) FROM "muted_members" WHERE "chat_id" = -105"#, vec![])
      .await,
    1
  );
  assert!(
    text(
      &scratch,
      r#"SELECT "text" FROM "instructions" WHERE "chat_id" = -105 AND "kind" = 'setting:mute'"#
    )
    .await
    .unwrap()
    .contains("The owner unmuted a member of this chat")
  );
}

fn file(id: &str, name: &str, mime: &str, bytes: &[u8]) -> MessageFile {
  MessageFile {
    id: id.into(),
    name: name.into(),
    mime: mime.into(),
    bytes: bytes.to_vec(),
    kind: None,
  }
}

#[tokio::test]
async fn an_attachment_keeps_its_files_beside_its_message_and_wakes_by_whose_file_it_is() {
  let (scratch, data, standing) = stood(&[-106]).await;
  let receiver = &standing.receiver;
  let context = Context {
    spoken_text: Some("this one".into()),
    carries_file: true,
    ..Context::default()
  };
  let handed = [
    file("1", "notes.txt", "text/plain", b"notes"),
    file("2", "screen.jpg", "image/jpeg", &[0xff, 0xd8]),
  ];

  receiver
    .attachment(&inbound(-106, 1, OWNER, "[file notes.txt] this one"), &context, room(), &handed)
    .await
    .unwrap();
  // The agent's own file is kept and goes no further.
  receiver
    .attachment(&inbound(-106, 2, AGENT_NUMBER, "[file b.txt]"), &context, room(), &[])
    .await
    .unwrap();

  let stored = text(
    &scratch,
    r#"SELECT "stored_path" FROM "collected_items" WHERE "chat_id" = -106 AND "message_id" = 1 AND "kind" = 'document'"#,
  )
  .await
  .unwrap();
  let picture = text(
    &scratch,
    r#"SELECT "image_path" FROM "messages" WHERE "chat_id" = -106 AND "message_id" = 1"#,
  )
  .await
  .unwrap();

  assert_eq!(std::fs::read(&stored).unwrap(), b"notes");
  assert!(stored.starts_with(&data.path().join("collected/-106").to_string_lossy().into_owned()));
  assert_eq!(std::fs::read(&picture).unwrap(), [0xff, 0xd8]);
  assert_eq!(standing.heard.told(), [(-106, 1), (-106, 2)]);
  assert_eq!(
    standing.waking.woken(),
    [Woken::File {
      chat_id: -106,
      message_id: 1,
      owner: true,
      caption: "this one".into(),
    }]
  );
}

#[tokio::test]
async fn a_file_still_being_gathered_wakes_once_it_is_handed_over() {
  let (scratch, _data, standing) = stood(&[-109]).await;
  let receiver = &standing.receiver;
  let gathering = Context {
    carries_file: true,
    gathering: true,
    ..Context::default()
  };

  receiver
    .attachment(&inbound(-109, 1, 55, "[voice]"), &gathering, room(), &[])
    .await
    .unwrap();
  assert!(standing.waking.woken().is_empty(), "nothing is woken ahead of the bytes");

  let kept = GatheredPatch::kept(vec![file("1", "voice.ogg", "audio/ogg", b"ogg")]);

  receiver.gathered(-109, 1, "road-a", &kept).await.unwrap();

  assert_eq!(
    scratch
      .count(
        r#"SELECT COUNT(*) FROM "collected_items" WHERE "chat_id" = -109 AND "kind" = 'audio'"#,
        vec![]
      )
      .await,
    1
  );
  assert_eq!(
    standing.waking.woken(),
    [Woken::File {
      chat_id: -109,
      message_id: 1,
      owner: false,
      caption: "[voice]".into(),
    }]
  );
}

#[tokio::test]
async fn an_edit_writes_the_words_over_a_kept_message_and_nothing_else() {
  let (scratch, _data, standing) = stood(&[-107]).await;
  let receiver = &standing.receiver;

  scratch.message(-107, 1, 55).await;
  receiver.edited(&inbound(-107, 1, 55, "fixed")).await.unwrap();
  receiver.edited(&inbound(-107, 9, 55, "never said")).await.unwrap();

  assert_eq!(standing.heard.told(), [(-107, 1)]);
  assert!(standing.waking.woken().is_empty());
  assert_eq!(
    text(&scratch, r#"SELECT "text" FROM "messages" WHERE "chat_id" = -107 AND "message_id" = 1"#)
      .await
      .as_deref(),
    Some("fixed")
  );
  assert_eq!(
    scratch.count(r#"SELECT COUNT(*) FROM "messages" WHERE "chat_id" = -107"#, vec![]).await,
    1
  );
}

#[tokio::test]
async fn the_chat_services_hand_overs_are_taken_behind_the_token_and_read_as_the_protocol_states_them() {
  use axum::body::Body;
  use axum::http::{Request, StatusCode};
  use tower::ServiceExt;

  let (scratch, _data, standing) = stood(&[-110]).await;
  let routes = bmo_receiver::receiver::handover::routes(standing.receiver.clone(), "the-token");
  let handed = serde_json::json!({
    "origin": "live",
    "senderId": 55,
    "senderName": "tester",
    "sentAt": 1_760_000_000_000_i64,
    "text": "hello",
    "replyToMessageId": null,
    "via": "road-a",
    "album": null,
    "mode": "bot-account",
    "context": { "roomTitle": "a room" },
    "room": { "kind": "group", "identity": AGENT_NUMBER },
  });
  let taken_path = "/chat/rooms/-110/messages/1";
  let put = |path: &str, token: &str, body: String| {
    Request::put(path)
      .header("authorization", format!("Bearer {token}"))
      .header("content-type", "application/json")
      .body(Body::from(body))
      .unwrap()
  };

  let refused = routes.clone().oneshot(put(taken_path, "wrong", handed.to_string())).await.unwrap();
  let taken = routes.clone().oneshot(put(taken_path, "the-token", handed.to_string())).await.unwrap();
  let unread = routes.oneshot(put(taken_path, "the-token", "{}".into())).await.unwrap();

  assert_eq!(refused.status(), StatusCode::UNAUTHORIZED);
  assert_eq!(taken.status(), StatusCode::CREATED);
  assert_eq!(unread.status(), StatusCode::BAD_REQUEST);
  assert_eq!(standing.heard.told(), [(-110, 1)]);
  assert_eq!(
    text(&scratch, r#"SELECT "title" FROM "chat_titles" WHERE "chat_id" = -110"#)
      .await
      .as_deref(),
    Some("a room")
  );
  assert_eq!(
    standing.waking.woken(),
    [Woken::Words {
      chat_id: -110,
      message_id: 1,
      owner: false,
      how: How::Words,
    }]
  );
}

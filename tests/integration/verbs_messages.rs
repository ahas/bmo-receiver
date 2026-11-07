//! The sends of the receiver's verbs over a scratch database (docs/messages.md): a line asked to be said in a
//! run of its own, the acts on a message, a room's file kept to its room, the owner's line that stands without
//! the bot, and the news told back to the chat that set a line off. The launcher of the test's own stands in for
//! the binary's, since the run's rows and its log are the agency's.

use std::io::Cursor;
use std::sync::Arc;
use std::sync::Mutex;

use bmo_chat_client::{
  AlbumItem, ChatCapability, ChatSend, ChatSent, FileItem, MissedPicture, MissedPictureReason,
};
use futures_util::future::BoxFuture;
use tempfile::tempdir;

use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::Receiver;
use bmo_receiver::verbs::messages::{
  Picture, Relay, SayAsked, failed_instruction, hand_back_to, opening, photo, pin, react, say, say_refusal,
  sent_instruction, sticker, tell_owner,
};
use bmo_receiver::verbs::room_files::{room_file_refusal, room_stores};
use bmo_receiver::verbs::sends::{Sends, verb_refusal};
use bmo_receiver::verbs::{Launch, LaunchAsked, RunAsked, VerbError};

use crate::integration::common::Scratch;
use crate::integration::common::units::{
  ADDRESS, AGENT, OWNER, SENDER, StandInChat, registration, standing,
};

const ROOM: i64 = -300;
const MESSAGE: i64 = 7;
const TURN_AGENT: &str = "BMO_TURN_AGENT";

/// A launcher of the test's own: it opens a row, starts nothing of its own and keeps what it was asked.
#[derive(Default)]
struct Launcher {
  started: Mutex<Vec<RunAsked>>,
  launched: Mutex<Vec<LaunchAsked>>,
  closed: Mutex<Vec<(i64, String)>>,
  logged: Mutex<Vec<(String, String, String)>>,
}

impl Launch for Launcher {
  fn start(&self, asked: &RunAsked) -> BoxFuture<'_, Option<i64>> {
    self.started.lock().unwrap().push(asked.clone());

    Box::pin(async { Some(11) })
  }

  fn launch(&self, asked: &LaunchAsked) -> BoxFuture<'_, bool> {
    self.launched.lock().unwrap().push(asked.clone());

    Box::pin(async { true })
  }

  fn close(&self, run_id: i64, outcome: &str) -> BoxFuture<'_, ()> {
    self.closed.lock().unwrap().push((run_id, outcome.to_owned()));

    Box::pin(async {})
  }

  fn log(&self, unit: &str, kind: &str, text: &str) -> BoxFuture<'_, ()> {
    self.logged.lock().unwrap().push((unit.to_owned(), kind.to_owned(), text.to_owned()));

    Box::pin(async {})
  }

  fn asked_by(&self, _run_id: i64) -> BoxFuture<'_, Option<String>> {
    Box::pin(async { Some(AGENT.to_owned()) })
  }
}

// A receiver over a scratch database, reaching one road that holds the room and the owner's chat, and a launcher
// of the test's own.
async fn stood() -> (Scratch, Arc<StandInChat>, Arc<Launcher>, Receiver) {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let road = Arc::new(StandInChat::new("road-a").holding(&[ROOM]).owners(OWNER));
  let standing = standing(scratch.db(), DataRoot::new(data.path()), [road.clone()].to_vec());

  let mut agent = registration(AGENT, SENDER, [ADDRESS].to_vec(), None);

  agent.row.insert("role".to_owned(), serde_json::json!("chat"));
  standing.agency.registering([agent].to_vec());

  (scratch, road, Arc::new(Launcher::default()), standing.receiver.clone())
}

#[tokio::test]
async fn a_line_asked_to_be_said_is_launched_and_nobody_waits_on_the_gates() {
  let (_scratch, road, launcher, receiver) = stood().await;
  let sends = Sends::new(receiver.clone(), Some(AGENT.to_owned()), None).with_launch(launcher.clone());
  let asked = SayAsked {
    chat_id: ROOM,
    text: "  The job is done.  ".to_owned(),
    reply: false,
    reply_to: None,
    started_by: Some(format!("chat:{OWNER}")),
  };
  let lines = say(&sends, &asked, None).await.unwrap();

  assert_eq!(lines.len(), 1);
  assert!(lines[0].starts_with("say launched as bmo-say-"), "{lines:?}");
  assert_eq!(launcher.started.lock().unwrap().len(), 1);
  assert_eq!(launcher.launched.lock().unwrap().len(), 1);
  assert_eq!(launcher.launched.lock().unwrap()[0].command.name, "say-runner", "{lines:?}");
  assert_eq!(
    launcher.launched.lock().unwrap()[0].command.env,
    [(TURN_AGENT.to_owned(), AGENT.to_owned())].to_vec()
  );
  assert!(road.sent().is_empty(), "the words are not put in the room by this call");
}

#[tokio::test]
async fn a_line_with_a_reply_target_is_sent_as_a_reply_to_it() {
  let (_scratch, road, launcher, receiver) = stood().await;
  let sends = Sends::new(receiver.clone(), Some(AGENT.to_owned()), None).with_launch(launcher);

  say(
    &sends,
    &SayAsked {
      chat_id: ROOM,
      text: "here it is".to_owned(),
      reply: false,
      reply_to: Some(20620),
      started_by: None,
    },
    None,
  )
  .await
  .unwrap();

  let sent = road.sent();

  assert_eq!(sent.len(), 1, "{sent:?}");
  assert_eq!(sent[0].reply_to_message_id, Some(20620));
}

#[tokio::test]
async fn a_line_asked_without_a_launcher_standing_is_said_by_the_receiver_itself() {
  let (_scratch, road, _launcher, receiver) = stood().await;
  let sends = Sends::new(receiver.clone(), None, None);

  assert!(
    say_refusal(&SayAsked {
      chat_id: ROOM,
      text: "  ".to_owned(),
      reply: false,
      reply_to: None,
      started_by: None
    })
    .is_some()
  );
  assert_eq!(
    say_refusal(&SayAsked {
      chat_id: ROOM,
      text: "words".to_owned(),
      reply: true,
      reply_to: None,
      started_by: None
    })
    .unwrap(),
    "an answer comes back to the chat that asked for it, so --reply needs --by chat:<your own chatId>, and that chat cannot be the room being written to.",
  );

  let lines = say(
    &sends,
    &SayAsked {
      chat_id: ROOM,
      text: "words".to_owned(),
      reply: false,
      reply_to: None,
      started_by: None,
    },
    None,
  )
  .await
  .unwrap();

  assert!(lines[0].starts_with("say launched as bmo-say-"), "the call answers at once: {lines:?}");

  // The receiver puts the words in the room apart from the call.
  for _ in 0..200 {
    if !road.sent().is_empty() {
      break;
    }

    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
  }

  let sent: Vec<String> = road.sent().iter().map(|send| serde_json::to_string(send).unwrap()).collect();

  assert_eq!(sent.len(), 1, "{sent:?}");
  assert!(sent[0].contains("words"), "{sent:?}");
}

#[test]
fn the_news_told_back_names_the_room_the_opening_and_whether_an_answer_comes() {
  let owner_chat = format!("chat:{OWNER}");
  let room_chat = format!("chat:{ROOM}");

  assert_eq!(hand_back_to(Some(&owner_chat), ROOM), Some(OWNER));
  assert_eq!(hand_back_to(Some(&room_chat), ROOM), None);
  assert_eq!(opening("one\n\ntwo"), "one");
  assert_eq!(opening(&"a".repeat(70)), format!("{}…", "a".repeat(60)));
  assert!(
    sent_instruction(ROOM, "words", Some(Relay::Waiting))
      .contains("its answer will reach you here on its own")
  );
  assert!(sent_instruction(ROOM, "words", Some(Relay::Dropped)).contains("will not be brought back"));
  assert!(failed_instruction(ROOM, "words", "the road refused it").contains("never say it did"));
}

#[tokio::test]
async fn a_pin_goes_quietly_and_the_acts_the_road_does_not_perform_are_refused_in_one_sentence() {
  let (_scratch, road, _launcher, receiver) = stood().await;
  let sends = Sends::new(receiver.clone(), Some(AGENT.to_owned()), None);

  let lines = pin(&sends, ROOM, MESSAGE, false).await.unwrap();

  assert_eq!(lines, ["pinned message 7"].to_vec());
  assert_eq!(
    road.sent().iter().map(|held| held.send.clone()).collect::<Vec<ChatSend>>(),
    [ChatSend::Pin {
      message_id: MESSAGE,
      on: true,
      silent: true
    }]
    .to_vec(),
  );

  let Err(VerbError::Refused(why)) = react(&sends, ROOM, MESSAGE, "thumbs up").await else {
    panic!("the road declares no reaction");
  };
  assert_eq!(why, verb_refusal("the road-a road", "reaction"), "{why}");

  let Err(VerbError::Refused(why)) = sticker(&sends, ROOM, "balloon", None).await else {
    panic!("the road declares no sticker");
  };
  assert!(why.contains("sticker"), "{why}");
}

#[tokio::test]
async fn a_rooms_file_never_goes_into_another_room_and_a_job_sends_none_of_it() {
  let (_scratch, _road, _launcher, receiver) = stood().await;
  let sends = Sends::new(receiver.clone(), None, None);
  let data = tempdir().unwrap();
  let root = DataRoot::new(data.path());
  let stores = room_stores(&root);
  let kept = stores[0].join(ROOM.to_string()).join("7-report.pdf");

  std::fs::create_dir_all(kept.parent().unwrap()).unwrap();
  std::fs::write(&kept, b"pdf").unwrap();

  let path = kept.to_string_lossy().into_owned();

  assert_eq!(room_file_refusal(&path, ROOM, &stores, false), None);
  assert!(room_file_refusal(&path, -400, &stores, false).unwrap().contains("is kept for room"));
  assert!(room_file_refusal(&path, ROOM, &stores, true).unwrap().contains("a job sends none"));

  let pictures = [Picture {
    url: path,
    label: None,
  }]
  .to_vec();
  let Err(VerbError::Refused(why)) = photo(&sends, &root, -400, &pictures, None, None, false).await else {
    panic!("the file is kept for another room");
  };
  assert!(why.contains("is kept for room"), "{why}");
}

#[tokio::test]
async fn a_picture_the_road_does_not_perform_is_refused_in_one_sentence() {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let root = DataRoot::new(data.path());
  let road = Arc::new(StandInChat::new("road-b").holding(&[ROOM]));
  let standing = standing(scratch.db(), root.clone(), [road.clone()].to_vec());
  let sends = Sends::new(standing.receiver.clone(), None, None);

  let pictures = [Picture {
    url: "https://example.org/a.png".to_owned(),
    label: None,
  }]
  .to_vec();
  let Err(VerbError::Refused(why)) = photo(&sends, &root, ROOM, &pictures, None, None, false).await else {
    panic!("the road performs no album send");
  };

  assert_eq!(why, verb_refusal("the road-b road", "album"), "{why}");
}

// A scratch receiver over a data root of the test's own, reaching one road that holds the room and declares the
// album send, with the sends those two stand on.
async fn picturing_road() -> (Scratch, Arc<StandInChat>, DataRoot, tempfile::TempDir, Sends) {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let root = DataRoot::new(data.path());
  let road = Arc::new(StandInChat::new("road-a").holding(&[ROOM]).canning(&[ChatCapability::Album]));
  let standing = standing(scratch.db(), root.clone(), [road.clone()].to_vec());
  let sends = Sends::new(standing.receiver.clone(), None, None);

  (scratch, road, root, data, sends)
}

fn png() -> Vec<u8> {
  let mut out = Cursor::new(Vec::new());

  image::RgbImage::from_pixel(8, 8, image::Rgb([10, 200, 30]))
    .write_to(&mut out, image::ImageFormat::Png)
    .unwrap();
  out.into_inner()
}

#[tokio::test]
async fn a_picture_under_the_data_root_goes_as_its_bytes_and_is_kept_under_its_message() {
  let (scratch, road, root, _data, sends) = picturing_road().await;
  let picture = png();
  let folder = root.collected().join(ROOM.to_string());

  std::fs::create_dir_all(&folder).unwrap();
  let path = folder.join("drawing.png");
  std::fs::write(&path, &picture).unwrap();

  let pictures = [Picture {
    url: path.to_string_lossy().into_owned(),
    label: None,
  }]
  .to_vec();
  let lines = photo(&sends, &root, ROOM, &pictures, Some("the drawing"), None, false).await.unwrap();

  assert_eq!(lines, ["sent".to_owned()].to_vec());
  assert_eq!(
    road.sent().iter().map(|held| held.send.clone()).collect::<Vec<ChatSend>>(),
    [ChatSend::Album {
      items: [AlbumItem::File(FileItem {
        name: "drawing.png".to_owned(),
        mime: "image/png".to_owned(),
        bytes: picture.clone(),
      })]
      .to_vec(),
      caption: Some("the drawing".to_owned()),
    }]
    .to_vec(),
    "a file under the data root goes as its bytes, not as a path the road cannot read"
  );

  let stored = scratch
    .texts(&format!(
      r#"SELECT COALESCE("image_path", '') FROM "messages" WHERE "chat_id" = {ROOM} AND "message_id" = 900"#
    ))
    .await
    .concat();

  assert!(stored.ends_with(&format!("images/{ROOM}/900.jpg")), "{stored}");
  assert_eq!(std::fs::read(&stored).unwrap(), picture, "the picture sent is the picture kept");
}

#[tokio::test]
async fn a_picture_outside_the_data_root_or_missing_there_is_refused_before_anything_goes() {
  let (_scratch, road, root, _data, sends) = picturing_road().await;
  let outside = tempdir().unwrap();
  let outside_path = outside.path().join("drawing.png");

  std::fs::write(&outside_path, png()).unwrap();

  let pictures = [Picture {
    url: outside_path.to_string_lossy().into_owned(),
    label: None,
  }]
  .to_vec();
  let Err(VerbError::Refused(why)) = photo(&sends, &root, ROOM, &pictures, None, None, false).await else {
    panic!("a file outside the data root may not be sent");
  };
  assert!(why.contains("outside the data folder"), "{why}");

  let pictures = [Picture {
    url: root.collected().join(ROOM.to_string()).join("gone.png").to_string_lossy().into_owned(),
    label: None,
  }]
  .to_vec();
  let Err(VerbError::Refused(why)) = photo(&sends, &root, ROOM, &pictures, None, None, false).await else {
    panic!("a file that is not there may not be sent");
  };
  assert!(why.contains("no such file"), "{why}");
  assert!(road.sent().is_empty(), "nothing went out");
}

#[tokio::test]
async fn a_picture_the_road_did_not_take_is_said_with_why_and_the_send_fails() {
  let (_scratch, road, root, _data, sends) = picturing_road().await;
  let picture = png();
  let folder = root.collected().join(ROOM.to_string());

  std::fs::create_dir_all(&folder).unwrap();
  std::fs::write(folder.join("drawing.png"), &picture).unwrap();
  *road.answer.lock().unwrap() = Some(ChatSent {
    message_id: None,
    missed: [MissedPicture {
      address: "drawing.png".to_owned(),
      reason: MissedPictureReason::Platform {
        detail: "the platform refused it".to_owned(),
      },
    }]
    .to_vec(),
  });

  let pictures = [Picture {
    url: folder.join("drawing.png").to_string_lossy().into_owned(),
    label: None,
  }]
  .to_vec();
  let Err(VerbError::Partly { lines, refusal }) =
    photo(&sends, &root, ROOM, &pictures, None, None, false).await
  else {
    panic!("no picture went, so the send says so");
  };
  let said = lines.join("\n");

  assert_eq!(refusal, "no picture was sent");
  assert!(said.contains("not sent as pictures (1)"), "{said}");
  assert!(said.contains("the platform refused it"), "{said}");
}

#[tokio::test]
async fn the_owners_line_goes_through_the_receivers_own_lines_with_no_gate_and_no_session() {
  let (_scratch, road, _launcher, receiver) = stood().await;
  let sends = Sends::new(receiver.clone(), None, None);

  let lines = tell_owner(&sends, "  The job is done.  ").await.unwrap();

  assert_eq!(lines, ["tell-owner: sent".to_owned()].to_vec());
  assert_eq!(
    road.sent().iter().map(|held| held.send.clone()).collect::<Vec<ChatSend>>(),
    [ChatSend::Text {
      text: "The job is done.".to_owned(),
      silent: false,
      fold: false
    }]
    .to_vec(),
  );
}

#[tokio::test]
async fn on_a_record_room_the_record_keeps_a_reaction_a_pin_an_edit_a_delete_and_a_sticker_and_carries_them()
 {
  use bmo_receiver::roads::providers;

  let (scratch, road, _launcher, receiver) = stood().await;
  let sends = Sends::new(receiver.clone(), Some(AGENT.to_owned()), None);
  let column = |id: i64, name: &str| {
    let scratch = &scratch;
    let sql = format!(
      r#"SELECT COALESCE("{name}"::text, '') FROM "messages" WHERE "chat_id" = {ROOM} AND "message_id" = {id}"#
    );
    async move { scratch.texts(&sql).await.concat() }
  };

  // The room on the record first and the road beside it: a line kept goes to the road under a number of its own.
  providers::choose(receiver.db(), ROOM, &["record".to_owned(), "road-a".to_owned()])
    .await
    .unwrap();

  let kept = receiver.speech().kept(ROOM, "a line on the record", None, None, None).await.unwrap();
  let carried = road.sent().len();

  assert_eq!(
    react(&sends, ROOM, kept, "👍").await.unwrap(),
    [format!("reacted 👍 to message {kept}")]
  );
  assert_eq!(
    scratch
      .texts(&format!(r#"SELECT "emoji" || '|' || "by" FROM "message_reactions" WHERE "chat_id" = {ROOM} AND "message_id" = {kept}"#))
      .await,
    [format!("👍|{AGENT}")],
    "the record keeps the reaction under who put it there"
  );
  assert!(
    road.sent()[carried..].iter().any(|held| matches!(held.send, ChatSend::Reaction { .. })),
    "the act goes on to the road where the message stands under its number: {:?}",
    road.sent()
  );

  react(&sends, ROOM, kept, "none").await.unwrap();
  assert!(
    scratch
      .texts(&format!(r#"SELECT "emoji" FROM "message_reactions" WHERE "message_id" = {kept}"#))
      .await
      .is_empty(),
    "an empty emoji takes the reactor's off"
  );

  pin(&sends, ROOM, kept, false).await.unwrap();
  assert!(!column(kept, "pinned_at").await.is_empty(), "the pin stands as when it was pinned");
  pin(&sends, ROOM, kept, true).await.unwrap();
  assert!(column(kept, "pinned_at").await.is_empty(), "an unpin takes it off");

  let going = sends.road(ROOM, "edit").await.unwrap();

  sends
    .send(
      &going,
      &bmo_chat_client::ChatSendRequest::new(
        ROOM,
        ChatSend::Edit {
          message_id: kept,
          text: "edited".to_owned(),
        },
      ),
    )
    .await
    .unwrap();
  assert_eq!(column(kept, "text").await, "edited");
  assert!(!column(kept, "edited_at").await.is_empty());

  sends
    .send(
      &going,
      &bmo_chat_client::ChatSendRequest::new(ROOM, ChatSend::Delete { message_id: kept }),
    )
    .await
    .unwrap();
  assert_eq!(column(kept, "text").await, "", "a delete clears the words and keeps the row");
  assert!(!column(kept, "deleted_at").await.is_empty());

  sticker(&sends, ROOM, "🎈", None).await.unwrap();
  assert_eq!(
    scratch
      .texts(&format!(
        r#"SELECT "text" || '|' || "kind" FROM "messages" WHERE "chat_id" = {ROOM} AND "kind" IS NOT NULL"#
      ))
      .await,
    ["🎈|sticker"],
    "a sticker is a message of its own"
  );

  // An act on a message the record does not keep is refused.
  assert!(react(&sends, ROOM, 999_999, "👍").await.is_err());
}

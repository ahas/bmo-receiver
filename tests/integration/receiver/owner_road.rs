//! Which road the owner's private chat is answered on (docs/unified-bmo.md, 「The cases」): each road carries the
//! answer to words that came in on it, and whatever nobody asked for there — a turn opened by the bot's lines alone,
//! a line the service says itself, a send that names no road — is kept in the chat's own record, which a road whose
//! screen shows it reaches, and goes on from there to the chat's other providers. Every other room keeps its own
//! road for all of it.

use std::sync::Arc;

use bmo_chat_client::ChatSend;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::roads::Road;

use crate::common::Scratch;
use crate::common::units::{OWNER, SENDER, StandInChat, standing};

/// A room of others, held by the platform road.
const ROOM: i64 = -300;

// The owner's chat on a platform road, and a road whose screen shows the owner's chat beside its own room.
fn roads() -> (Arc<StandInChat>, Arc<StandInChat>) {
  (
    Arc::new(StandInChat::new("road-a").owners(OWNER).holding(&[ROOM]).agent_in(OWNER, 7)),
    Arc::new(StandInChat::new("road-b").showing(OWNER).holding(&[-400])),
  )
}

#[tokio::test]
async fn a_turn_of_the_owners_chat_answers_on_a_road_only_for_words_that_came_in_on_it() {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let (platform, screen) = roads();
  let standing = standing(scratch.db(), DataRoot::new(data.path()), vec![platform, screen]);
  let roads = standing.receiver.roads();
  let road = async |chat_id: i64, came: &[&str], asked: &[&str]| {
    roads.turn_road(chat_id, came, asked).await.map(|road| road.key().to_owned())
  };
  let named = |key: &str| Some(key.to_owned());

  assert_eq!(road(OWNER, &["road-a"], &[]).await, named("road-a"));
  assert_eq!(road(OWNER, &["road-a", "road-a"], &["road-b"]).await, named("road-a"));
  assert_eq!(road(OWNER, &["road-a", "road-b"], &[]).await, named("road-a"));
  assert_eq!(road(OWNER, &["road-b"], &[]).await, named("road-b"));
  assert_eq!(road(OWNER, &[], &["road-a"]).await, named("road-a"));
  assert_eq!(road(OWNER, &[], &[]).await, named("record"));
  assert_eq!(road(OWNER, &[], &["road-a", "road-b"]).await, named("record"));
  assert_eq!(road(OWNER, &[], &["record"]).await, named("record"));

  // A room of others is shown by no screen, and keeps its own road whatever it is handed.
  assert_eq!(road(ROOM, &[], &[]).await, named("road-a"));
  assert_eq!(road(ROOM, &["road-b"], &[]).await, named("road-a"));
  assert_eq!(road(ROOM, &[], &["record"]).await, named("road-a"));

  // A room no road holds has no road at all.
  assert_eq!(roads.unasked(-999).await, None);
  assert_eq!(roads.unasked(ROOM).await, Some(Road::Service("road-a".to_owned())));
}

#[tokio::test]
async fn a_line_nobody_asked_for_is_kept_in_the_owners_record_and_goes_on_the_rooms_road_elsewhere() {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let (platform, screen) = roads();
  let standing = standing(scratch.db(), DataRoot::new(data.path()), vec![platform.clone(), screen.clone()]);
  let speech = standing.receiver.speech();

  let kept = speech.say_to_owner("the job is done").await.unwrap().unwrap();
  let sent = speech.say(ROOM, "the game begins", false).await.unwrap();
  speech.denied(ROOM).await.unwrap();

  // The owner's line stands in the record, under the agent's number in the owner's chat and its name, below every
  // number a platform gave, and the screens are told; the service holding the chat is handed it from the record
  // as one of the chat's providers, and a screen that only shows the chat is not.
  assert!(kept < 0);
  assert_eq!(standing.heard.told(), [(OWNER, kept)]);
  assert_eq!(
    scratch
      .texts(&format!(
        r#"SELECT "via" || ' ' || "sender_id" || ' ' || "sender_name" || ' ' || "text" FROM "messages" WHERE "chat_id" = {OWNER} AND "message_id" = {kept}"#
      ))
      .await,
    [format!("record 7 {SENDER} the job is done (read)")]
  );
  assert!(screen.sent().is_empty());

  // The room's lines went on its own road: the sanitiser read the first, and the fixed line went as rendered,
  // after the owner's line carried from his chat's record.
  let texts: Vec<String> = platform
    .sent()
    .into_iter()
    .map(|request| match request.send {
      ChatSend::Text { text, .. } => text,
      other => panic!("only text was said, not {other:?}"),
    })
    .collect();

  assert_eq!(sent, Some(900));
  assert_eq!(texts, ["the job is done (read)", "the game begins (read)", "chat-denied in en"]);
  assert_eq!(platform.sent()[0].room_id, OWNER);
  assert_eq!(*standing.gate.read.lock().unwrap(), ["the job is done", "the game begins"]);
}

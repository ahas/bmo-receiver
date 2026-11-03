//! A line an agent sent into a room through its sends (docs/messages.md, 「What an agent can send back」;
//! docs/unified-bmo.md, 「The cases」): kept as that agent's words under the number the road gave it, so a reply on
//! the road names it and wakes that agent alone (docs/wake.md, 「Who reaches the session」); and a line on a road
//! that gave no number, kept under the record's own.

use std::sync::Arc;

use bmo_chat_client::{ChatCapability, ChatSend, ChatSendRequest, ChatSent};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::Context;
use bmo_receiver::verbs::sends::Sends;

use super::{everyone_heard, inbound, room};
use crate::integration::common::Scratch;
use crate::integration::common::units::{
  ADDRESS, AGENT, Handed, StandInChat, Standing, registration, standing, woken,
};

const ROOM: i64 = -141;

// The head of an OGG container, which is what a voice note is read as.
const OGG: &[u8] = &[0x4f, 0x67, 0x67, 0x53, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4];

// A room two agents answer on one road that carries voice notes: the default answerer, and another the road numbered 77.
async fn two_answerers() -> (Scratch, tempfile::TempDir, Arc<StandInChat>, Standing) {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let road = Arc::new(StandInChat::new("road-a").holding(&[ROOM]).canning(&[ChatCapability::Voice]));
  let standing = woken(standing(scratch.db(), DataRoot::new(data.path()), vec![road.clone()]));
  let mut other = registration("agent-c", "Agent C", vec!["see"], None);

  other.row.insert("role".to_owned(), serde_json::json!("chat"));
  standing
    .agency
    .registering([registration(AGENT, "Agent A", vec![ADDRESS], None), other].to_vec());
  everyone_heard(&scratch).await;
  scratch
    .run(
      r#"INSERT INTO "room_supervisors" ("chat_id", "supervisor") VALUES ($1, 'agent-a'), ($1, 'agent-c')"#,
      vec![ROOM.into()],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "provider_users" ("provider", "user_id", "sender_id") VALUES ('road-a', 'agent-c', 77) ON CONFLICT DO NOTHING"#,
      vec![],
    )
    .await;

  (scratch, data, road, standing)
}

// A voice note sent into the room as the agent's send, as `bmo talk --voice` asks it.
async fn voiced(standing: &Standing, agent: &str) {
  let sends = Sends::new(standing.receiver.clone(), Some(agent.to_owned()), None);
  let going = sends.road(ROOM, "voice").await.unwrap();
  let note = ChatSend::Voice {
    audio: OGG.to_vec(),
    said: "the campsite at dusk".to_owned(),
    caption: None,
    transcript: None,
  };

  sends.send(&going, &ChatSendRequest::new(ROOM, note)).await.unwrap();
}

#[tokio::test]
async fn a_voice_note_an_agent_sent_is_kept_under_the_roads_number_and_a_reply_to_it_wakes_that_agent_alone()
 {
  let (scratch, _data, road, standing) = two_answerers().await;

  voiced(&standing, "agent-c").await;

  assert_eq!(road.sent().len(), 1, "the note went out once: {:?}", road.sent());
  assert_eq!(
    scratch
      .texts(
        r#"SELECT concat_ws(' ', "message_id", "sender_id", "agent_name", "via") FROM "messages" WHERE "chat_id" = -141"#
      )
      .await,
    ["900 77 agent-c road-a"],
    "the note stands under the number the road gave it, as the agent's own words"
  );

  // A person's reply on the road names the number the road gave the note: it wakes the agent that sent it, and
  // not the default answerer, whose window is closed and whom it does not call.
  let mut reply = inbound(ROOM, 901, 55, "where is that");

  reply.reply_to_message_id = Some(900);
  standing.receiver.message(&reply, &Context::default(), room()).await.unwrap();

  assert_eq!(
    standing.taken.handed(),
    [Handed {
      agent: "agent-c".into(),
      chat_id: ROOM,
      message_id: 901,
      files: vec![],
      opens_window: true,
      read_through: Some(false),
    }]
  );
}

#[tokio::test]
async fn a_line_on_a_road_that_gives_no_number_is_kept_under_the_records_own() {
  let (scratch, _data, road, standing) = two_answerers().await;

  *road.answer.lock().unwrap() = Some(ChatSent {
    message_id: None,
    missed: vec![],
  });
  voiced(&standing, "agent-c").await;

  assert_eq!(
    scratch
      .texts(
        r#"SELECT concat_ws(' ', "message_id", "agent_name", "via") FROM "messages" WHERE "chat_id" = -141"#
      )
      .await,
    ["-1 agent-c record"],
    "a line no road numbered takes the record's own number"
  );
}

#[tokio::test]
async fn a_line_in_a_room_on_the_record_alone_is_kept_under_the_records_own_number() {
  let (scratch, _data, road, standing) = two_answerers().await;

  bmo_receiver::roads::providers::choose(scratch.db(), ROOM, &["record".to_owned()])
    .await
    .unwrap();

  let sends = Sends::new(standing.receiver.clone(), Some("agent-c".to_owned()), None);
  let going = sends.road(ROOM, "text").await.unwrap();
  let line = ChatSend::Text {
    text: "kept here alone".to_owned(),
    silent: false,
    fold: false,
  };

  sends.send(&going, &ChatSendRequest::new(ROOM, line)).await.unwrap();

  assert!(
    road.sent().is_empty(),
    "no messenger hears a room on the record alone: {:?}",
    road.sent()
  );
  assert_eq!(
    scratch
      .texts(
        r#"SELECT concat_ws(' ', "message_id", "agent_name", "via") FROM "messages" WHERE "chat_id" = -141"#
      )
      .await,
    ["-1 agent-c record"]
  );
}

//! Who said a kept message, corrected on the record (docs/api.md, 「Messages」; docs/messages.md, 「The archive」): a
//! line under the agent's own number in its room is filed as the agent's, under its name and the voice its road
//! answers the room by, with nothing sent to a chat service; a line under another sender, an agent the agency does
//! not register and a message the room does not hold are refused.

use axum::http::StatusCode;
use serde_json::json;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{put, stood_on};
use crate::integration::common::units::{AGENT, OWNER, SENDER, StandInChat, registration};

const ROOM: i64 = -100;
const AGENT_NUMBER: i64 = 7;
const MEMBER: i64 = 77;

#[tokio::test]
async fn a_line_under_the_agents_own_number_is_filed_as_its_own_and_no_other_is() {
  let scratch = Scratch::new().await;
  let road = std::sync::Arc::new(StandInChat::new("road-a").holding(&[ROOM]).agent_in(ROOM, AGENT_NUMBER));
  let front = stood_on(&scratch, OWNER, vec![road.clone()]).await;

  front.standing.agency.registering(vec![registration(AGENT, SENDER, vec![], None)]);
  // The room is read through a person's account, so its road answers it in the human-avatar voice.
  scratch
    .run(
      r#"INSERT INTO "room_modes" ("chat_id", "mode", "added_at") VALUES (-100, 'avatar', 1)"#,
      vec![],
    )
    .await;

  // The agent's line read back under the name the platform shows its number by, and a member's line.
  for (message, sender) in [(1, AGENT_NUMBER), (2, MEMBER)] {
    scratch
      .run(
        r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text", "via")
           VALUES (-100, $1, $2, 'someone', $1, 'words', 'road-a')"#,
        vec![message.into(), sender.into()],
      )
      .await;
  }

  let (status, filed) = put(&front, "/rooms/-100/messages/1/speaker", json!({ "agentName": AGENT })).await;

  assert_eq!(status, StatusCode::OK, "{filed}");
  assert_eq!(
    (
      &filed["senderId"],
      &filed["senderName"],
      &filed["agentName"],
      &filed["voice"],
      &filed["text"]
    ),
    (
      &json!(AGENT_NUMBER),
      &json!(SENDER),
      &json!(AGENT),
      &json!("human-avatar"),
      &json!("words")
    )
  );
  assert!(road.sent().is_empty(), "{:?}", road.sent());

  let refused = [
    ("/rooms/-100/messages/2/speaker", AGENT, StatusCode::CONFLICT, "message-not-agents"),
    ("/rooms/-100/messages/1/speaker", "agent-z", StatusCode::CONFLICT, "message-not-agents"),
    ("/rooms/-100/messages/9/speaker", AGENT, StatusCode::NOT_FOUND, "message-unknown"),
  ];

  for (path, agent, status, code) in refused {
    let (answered, said) = put(&front, path, json!({ "agentName": agent })).await;

    assert_eq!((answered, &said["code"]), (status, &json!(code)), "{path} as {agent}: {said}");
  }

  assert_eq!(
    scratch
      .texts(r#"SELECT "sender_name" || '|' || coalesce("agent_name", '') FROM "messages" WHERE "message_id" = 2"#)
      .await,
    ["someone|"]
  );
}

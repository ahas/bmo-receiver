//! A room as the deployment reads it, where `bmo-receiver` serves the routes bmo-job and the console's owner
//! reach (`docs/boundary-api.md`, 「For bmo-job」 and 「For the web unit's console」): the owner, a room, its
//! answerers, its messages and its line walked by a cursor the reader holds. Each test stands on a scratch
//! archive of its own.

use axum::http::StatusCode;
use serde_json::json;

use crate::common::Scratch;
use crate::common::queue::{get, stood};
use crate::common::units::{AGENT, OWNER, registration};

#[tokio::test]
async fn the_owner_is_read_once_and_a_room_reads_as_the_deployment_sees_it() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  assert_eq!(get(&front, "/owner").await, (StatusCode::OK, json!({ "sender": OWNER, "chat": OWNER })),);

  scratch.message(7, 1, OWNER).await;

  assert_eq!(
    get(&front, "/rooms/7").await,
    (
      StatusCode::OK,
      json!({ "chat": 7, "owner_room": false, "registered": true, "work_chat": 7 })
    ),
  );
  assert_eq!(
    get(&front, "/rooms/987654").await,
    (
      StatusCode::OK,
      json!({ "chat": 987654, "owner_room": false, "registered": false, "work_chat": OWNER })
    ),
  );
}

#[tokio::test]
async fn a_rooms_answerers_are_read_as_the_wake_judges_them() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  front
    .standing
    .agency
    .registering([registration(AGENT, "Agent A", ["a"].to_vec(), None, true)].to_vec());
  scratch
    .run(r#"INSERT INTO "room_answerers" ("chat_id", "agent") VALUES (7, 'agent-a')"#, vec![])
    .await;

  assert_eq!(
    get(&front, "/rooms/7/answerers").await,
    (
      StatusCode::OK,
      json!({ "default": AGENT, "voice": AGENT, "answerers": [{ "agent": AGENT, "companion": false }] }),
    ),
  );
}

#[tokio::test]
async fn a_message_said_in_a_room_is_read_back_whole() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  scratch.message(7, 11, OWNER).await;

  assert_eq!(
    get(&front, "/rooms/7/messages/11").await,
    (
      StatusCode::OK,
      json!({ "sender": OWNER, "sender_name": "someone", "sent_at": 1, "text": "words" })
    ),
  );
  assert_eq!(get(&front, "/rooms/7/messages/12").await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_console_reads_its_rooms_each_with_its_name_and_counts() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  scratch.message(7, 1, OWNER).await;
  scratch.message(7, 2, OWNER).await;
  scratch.message(8, 3, OWNER).await;
  scratch
    .run(r#"INSERT INTO "chat_titles" ("chat_id", "title") VALUES (7, 'The room')"#, vec![])
    .await;
  scratch
    .run(
      r#"INSERT INTO "settings" ("key", "value") VALUES ('console_pinned_rooms', '[8]')"#,
      vec![],
    )
    .await;

  let (_, listed) = get(&front, "/console/rooms").await;

  assert_eq!(
    listed["rooms"]
      .as_array()
      .unwrap()
      .iter()
      .map(|room| room["chatId"].as_i64().unwrap())
      .collect::<Vec<i64>>(),
    [8, 7],
  );
  assert_eq!(
    listed["rooms"]
      .as_array()
      .unwrap()
      .iter()
      .map(|room| room["pinned"].as_bool().unwrap())
      .collect::<Vec<bool>>(),
    [true, false],
  );
  assert_eq!(
    listed["rooms"]
      .as_array()
      .unwrap()
      .iter()
      .map(|room| room["messageCount"].as_i64().unwrap())
      .collect::<Vec<i64>>(),
    [1, 2],
  );
  assert_eq!(
    get(&front, "/console/rooms/7/title").await,
    (StatusCode::OK, json!({ "title": "The room" })),
  );
}

#[tokio::test]
async fn a_page_of_a_room_walks_the_line_by_the_cursor_the_reader_holds() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  for (message, sent) in [(1, 1), (2, 2), (3, 3)].to_vec() {
    scratch
      .run(
        r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text")
           VALUES (7, $1, $3, 'someone', $2, 'words')"#,
        vec![message.into(), sent.into(), OWNER.into()],
      )
      .await;
  }

  let (_, whole) = get(&front, "/console/rooms/7/messages?limit=2").await;

  assert_eq!(
    whole["messages"]
      .as_array()
      .unwrap()
      .iter()
      .map(|page| page["messageId"].as_i64().unwrap())
      .collect::<Vec<i64>>(),
    [2, 3],
  );
  assert!(whole["hasMore"].as_bool().unwrap());

  let (_, older) = get(&front, "/console/rooms/7/messages?before=3&limit=2").await;

  assert_eq!(
    older["messages"]
      .as_array()
      .unwrap()
      .iter()
      .map(|page| page["messageId"].as_i64().unwrap())
      .collect::<Vec<i64>>(),
    [1, 2],
  );
  assert!(!older["hasMore"].as_bool().unwrap());

  let (_, past) = get(&front, "/console/rooms/7/messages/after?after=1&limit=2").await;

  assert_eq!(
    past["messages"]
      .as_array()
      .unwrap()
      .iter()
      .map(|page| page["messageId"].as_i64().unwrap())
      .collect::<Vec<i64>>(),
    [2, 3],
  );
  assert!(!past["hasMore"].as_bool().unwrap());

  assert_eq!(
    get(&front, "/console/rooms/7/day-start?from=2").await,
    (StatusCode::OK, json!({ "messageId": 2 })),
  );
  assert_eq!(
    get(&front, "/console/rooms/7/messages/3").await,
    (
      StatusCode::OK,
      json!({ "messages": [json!({ "chatId": 7, "messageId": 3, "senderId": OWNER, "senderName": "someone", "sentAt": 3, "text": "words", "imagePath": null, "replyToMessageId": null, "answerSeconds": null, "round": null, "album": null, "via": null, "files": [] })], "hasMore": false })
    ),
  );
}

#[tokio::test]
async fn the_console_reads_an_agents_own_message_and_nothing_for_a_persons() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  front
    .standing
    .agency
    .registering([registration(AGENT, "Agent A", ["a"].to_vec(), None, true)].to_vec());
  scratch
    .run(
      r#"INSERT INTO "provider_users" ("provider", "user_id", "sender_id") VALUES ('road', 'agent-a', 99)"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text")
         VALUES (7, 4, 99, 'Agent A', 4, 'said as an agent')"#,
      vec![],
    )
    .await;
  scratch.message(7, 5, OWNER).await;

  let (_, agent_said) = get(&front, "/console/rooms/7/agent-message/4").await;

  assert_eq!(agent_said["messageId"].as_i64().unwrap(), 4);
  assert_eq!(get(&front, "/console/rooms/7/agent-message/5").await.0, StatusCode::NOT_FOUND);
}

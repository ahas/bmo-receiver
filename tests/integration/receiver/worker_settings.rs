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

  assert_eq!(
    get(&front, "/owner").await,
    (StatusCode::OK, json!({ "senderId": OWNER, "room": OWNER })),
  );

  scratch.message(7, 1, OWNER).await;

  assert_eq!(
    get(&front, "/rooms/7").await,
    (
      StatusCode::OK,
      json!({
        "id": 7,
        "title": null,
        "messageCount": 1,
        "lastMessageAt": 1_000,
        "pinned": false,
        "senders": [OWNER],
        "companions": [],
        "ownerRoom": false,
        "registered": true,
        "workRoom": 7,
        "mode": null,
      })
    ),
  );
  assert_eq!(
    get(&front, "/rooms/987654").await,
    (
      StatusCode::OK,
      json!({
        "id": 987654,
        "title": null,
        "messageCount": 0,
        "lastMessageAt": null,
        "pinned": false,
        "senders": [],
        "companions": [],
        "ownerRoom": false,
        "registered": false,
        "workRoom": OWNER,
        "mode": null,
      })
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
      json!({
        "default": AGENT,
        "voice": AGENT,
        "items": [{ "agentName": AGENT, "companion": false }],
        "more": false,
      }),
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
      json!({
        "id": 11,
        "room": 7,
        "senderId": OWNER,
        "senderName": "someone",
        "sentAt": 1_000,
        "text": "words",
        "imagePath": null,
        "replyToMessageId": null,
        "answerSeconds": null,
        "round": null,
        "album": null,
        "via": null,
        "agentName": null,
        "voice": null,
        "kind": null,
        "editedAt": null,
        "deletedAt": null,
        "pinnedAt": null,
        "reactions": [],
        "forwardedFrom": null,
        "situation": null,
        "files": [],
      })
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

  let (_, listed) = get(&front, "/rooms").await;

  assert_eq!(
    listed["items"]
      .as_array()
      .unwrap()
      .iter()
      .map(|room| room["id"].as_i64().unwrap())
      .collect::<Vec<i64>>(),
    [8, 7],
  );
  assert_eq!(
    listed["items"]
      .as_array()
      .unwrap()
      .iter()
      .map(|room| room["pinned"].as_bool().unwrap())
      .collect::<Vec<bool>>(),
    [true, false],
  );
  assert_eq!(
    listed["items"]
      .as_array()
      .unwrap()
      .iter()
      .map(|room| room["messageCount"].as_i64().unwrap())
      .collect::<Vec<i64>>(),
    [1, 2],
  );
  assert_eq!(get(&front, "/rooms/7/title").await, (StatusCode::OK, json!({ "title": "The room" })),);
}

#[tokio::test]
async fn two_messages_sent_in_one_second_keep_the_order_they_were_sent_in() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  // The later message is written first, so the row order alone would put it before the earlier one.
  for (message, sent) in [(1, 1_760_000_000_500_i64), (2, 1_760_000_000_100_i64)] {
    scratch
      .run(
        r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text")
           VALUES (7, $1, $3, 'someone', $2, 'words')"#,
        vec![message.into(), sent.into(), OWNER.into()],
      )
      .await;
  }

  let (_, page) = get(&front, "/rooms/7/messages?limit=10").await;

  assert_eq!(
    page["items"]
      .as_array()
      .unwrap()
      .iter()
      .map(|page| page["id"].as_i64().unwrap())
      .collect::<Vec<i64>>(),
    [2, 1],
    "the room's line follows the moment each was sent, to the millisecond",
  );
}

#[tokio::test]
async fn a_page_of_a_room_walks_the_line_by_the_cursor_the_reader_holds() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  for (message, sent) in [(1, 1_000), (2, 2_000), (3, 3_000)].to_vec() {
    scratch
      .run(
        r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text")
           VALUES (7, $1, $3, 'someone', $2, 'words')"#,
        vec![message.into(), sent.into(), OWNER.into()],
      )
      .await;
  }

  let (_, whole) = get(&front, "/rooms/7/messages?limit=2").await;

  assert_eq!(
    whole["items"]
      .as_array()
      .unwrap()
      .iter()
      .map(|page| page["id"].as_i64().unwrap())
      .collect::<Vec<i64>>(),
    [2, 3],
  );
  assert!(whole["more"].as_bool().unwrap());

  let (_, older) = get(&front, "/rooms/7/messages?before=3&limit=2").await;

  assert_eq!(
    older["items"]
      .as_array()
      .unwrap()
      .iter()
      .map(|page| page["id"].as_i64().unwrap())
      .collect::<Vec<i64>>(),
    [1, 2],
  );
  assert!(!older["more"].as_bool().unwrap());

  let (_, past) = get(&front, "/rooms/7/messages?after=1&limit=2").await;

  assert_eq!(
    past["items"]
      .as_array()
      .unwrap()
      .iter()
      .map(|page| page["id"].as_i64().unwrap())
      .collect::<Vec<i64>>(),
    [2, 3],
  );
  assert!(!past["more"].as_bool().unwrap());

  assert_eq!(
    get(&front, "/rooms/7/day-start?from=2000").await,
    (StatusCode::OK, json!({ "messageId": 2 })),
  );
  let (status, one) = get(&front, "/rooms/7/messages/3").await;

  assert_eq!(status, StatusCode::OK);
  assert_eq!(one["id"], 3);
  assert_eq!(one["sentAt"], 3_000);
  assert_eq!(one["text"], "words");
}

#[tokio::test]
async fn a_text_query_finds_the_rooms_messages_that_carry_it_newest_page_first() {
  let scratch = Scratch::new().await;
  let front = stood(&scratch, OWNER).await;

  // The Korean words stand in a fixture: the first message's, the fourth's and the text searched for.
  let korean =
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/search-korean.txt"))
      .unwrap();
  let korean: Vec<&str> = korean.lines().collect();
  let (first_words, fourth_words, searched) = (korean[0], korean[1], korean[2]);

  for (message, text) in [
    (1, first_words),
    (2, "something else"),
    (3, "The Meeting moved"),
    (4, fourth_words),
    (5, "50% off"),
  ] {
    scratch
      .run(
        r#"INSERT INTO "messages" ("chat_id", "message_id", "sender_id", "sender_name", "sent_at", "text")
           VALUES (7, $1, $2, 'someone', $3, $4)"#,
        vec![message.into(), OWNER.into(), (1_000 + message).into(), text.into()],
      )
      .await;
  }

  let ids = |page: &serde_json::Value| -> Vec<i64> {
    page["items"]
      .as_array()
      .unwrap()
      .iter()
      .filter_map(|item| item["id"].as_i64())
      .collect()
  };
  let (_, korean) = get(&front, &format!("/rooms/7/messages?q={}&limit=10", urlencode(searched))).await;
  let (_, english) = get(&front, "/rooms/7/messages?q=meeting&limit=10").await;
  let (_, literal) = get(&front, &format!("/rooms/7/messages?q={}&limit=10", urlencode("%"))).await;
  let (_, first) = get(&front, &format!("/rooms/7/messages?q={}&limit=1", urlencode(searched))).await;
  let (_, older) =
    get(&front, &format!("/rooms/7/messages?q={}&limit=1&before=4", urlencode(searched))).await;

  assert_eq!(ids(&korean), [1, 4], "the words that carry the text, oldest first in the page");
  assert_eq!(ids(&english), [3], "case set aside");
  assert_eq!(ids(&literal), [5], "the text read as given, no pattern");
  assert_eq!((ids(&first), first["more"].as_bool()), (vec![4], Some(true)), "the newest page first");
  assert_eq!(ids(&older), [1], "walked by before");
}

fn urlencode(text: &str) -> String {
  text.bytes().map(|byte| format!("%{byte:02X}")).collect()
}

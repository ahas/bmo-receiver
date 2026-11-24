//! A line another unit asks to be kept (docs/api.md, 「Kept lines」): an agent's night of sleep is kept in the
//! owner's private room, in its record alone, as the fixed line it is, and nothing goes to the chat service; a
//! line that is not one a unit asks to be kept is refused, and so is one that does not read.

use std::sync::Arc;

use axum::http::StatusCode;
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::routes::routes;
use serde_json::json;

use crate::integration::common::Scratch;
use crate::integration::common::queue::{TOKEN, posted_to};
use crate::integration::common::units::{OWNER, StandInChat, deployment, unrun, woken};

// The fixed lines a room's record keeps, each as the line it was said from.
async fn lines_in(scratch: &Scratch, chat_id: i64) -> Vec<serde_json::Value> {
  scratch
    .texts(&format!(
      r#"SELECT "line" FROM "messages" WHERE "chat_id" = {chat_id} AND "kind" = 'fixed-line' ORDER BY "rowid""#
    ))
    .await
    .into_iter()
    .map(|line| serde_json::from_str::<serde_json::Value>(&line).unwrap())
    .collect()
}

#[tokio::test]
async fn a_night_of_sleep_is_kept_in_the_owners_private_room_alone_and_sent_nowhere() {
  let scratch = Scratch::new().await;
  let data = tempfile::tempdir().unwrap();
  let chat = Arc::new(StandInChat::new("road-a").owners(OWNER));
  let standing = woken(unrun(scratch.db(), DataRoot::new(data.path()), vec![chat.clone()], deployment()));
  let receiver = standing.receiver.clone();
  let line = json!({ "code": "slept", "agent": "agent-a", "date": "2026-11-24", "learned": 3, "merged": 1, "closed": 2 });

  let (status, body) =
    posted_to(routes(receiver.clone(), TOKEN), "/lines/agent-a", json!({ "line": line })).await;

  assert_eq!(status, StatusCode::CREATED, "{body}");
  assert_eq!(body["room"], json!(OWNER), "{body}");
  assert_eq!(
    lines_in(&scratch, OWNER).await,
    [json!({
      "code": "slept",
      "fields": { "agent": "agent-a", "date": "2026-11-24", "learned": 3, "merged": 1, "closed": 2 }
    })],
    "one line, kept as the fixed line it is"
  );
  assert!(chat.sent().is_empty(), "nothing went to the chat service");

  // A line a unit asks the receiver to say, or the receiver's own, is not one kept on a unit's asking.
  for line in [
    json!({ "code": "supply-closed", "why": "no-order", "until": null }),
    json!({ "code": "chat-denied" }),
    json!({ "code": "no-such-line" }),
  ] {
    let (status, body) =
      posted_to(routes(receiver.clone(), TOKEN), "/lines/agent-a", json!({ "line": line })).await;

    assert_eq!(
      (status, body["code"].as_str()),
      (StatusCode::BAD_REQUEST, Some("bad-request")),
      "{body}"
    );
  }
  assert_eq!(lines_in(&scratch, OWNER).await.len(), 1, "a refusal keeps nothing");
}

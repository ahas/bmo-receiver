//! A window's close told on the stream (docs/wake.md, 「The names and the window」): once the end last written for
//! it passes, the room's screens hear it closed then, naming the room and the agent; an end moved later is the one
//! that closes it, once; and the ends the database holds still to come are watched as the receiver starts.

use std::time::Duration;

use serde_json::json;
use tempfile::tempdir;

use bmo_receiver::data_root::DataRoot;
use bmo_receiver::now_ms;
use bmo_receiver::receiver::wake::agents::{set_window, watch_window, watch_windows};

use crate::common::Scratch;
use crate::common::units::{AGENT, standing};

const ROOM: i64 = -100;

#[tokio::test]
async fn a_window_whose_end_passes_is_told_closed_once_at_the_end_that_stood() {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let standing = standing(scratch.db(), DataRoot::new(data.path()), vec![]);
  let receiver = standing.receiver.clone();
  let first = now_ms() + 60;
  let moved = now_ms() + 300;

  set_window(scratch.db(), ROOM, AGENT, first).await.unwrap();
  watch_window(&receiver, ROOM, AGENT, first);
  set_window(scratch.db(), ROOM, AGENT, moved).await.unwrap();
  watch_window(&receiver, ROOM, AGENT, moved);

  tokio::time::sleep(Duration::from_millis(150)).await;
  assert!(
    standing.heard.events("window-closed").is_empty(),
    "a window whose end moved has not closed"
  );

  tokio::time::sleep(Duration::from_millis(400)).await;
  assert_eq!(
    standing.heard.events("window-closed"),
    [
      json!({ "type": "window-closed", "chatId": ROOM, "messageId": null, "agent": AGENT, "closedAt": moved })
    ],
  );
}

#[tokio::test]
async fn the_ends_still_to_come_are_watched_as_the_receiver_starts() {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let standing = standing(scratch.db(), DataRoot::new(data.path()), vec![]);
  let coming = now_ms() + 100;

  set_window(scratch.db(), ROOM, AGENT, coming).await.unwrap();
  set_window(scratch.db(), -200, AGENT, now_ms() - 1_000).await.unwrap();
  watch_windows(&standing.receiver).await.unwrap();

  tokio::time::sleep(Duration::from_millis(400)).await;

  let closed = standing.heard.events("window-closed");

  assert_eq!(closed.len(), 1, "an end already past is not told again: {closed:?}");
  assert_eq!(
    (closed[0]["chatId"].as_i64(), closed[0]["closedAt"].as_i64()),
    (Some(ROOM), Some(coming))
  );
}

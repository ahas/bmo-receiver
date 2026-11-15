//! A window's close told on the stream (docs/wake.md, 「The names and the window」): once the end last written for
//! it passes, the room's screens hear it closed then, naming the room and the agent; an end moved later is the one
//! that closes it, once; and the ends the database holds still to come are watched as the receiver starts. Every
//! watch is awaited through its handle, with ends already reached, so no test waits on the clock.

use serde_json::json;
use tempfile::tempdir;

use bmo_receiver::data_root::DataRoot;
use bmo_receiver::now_ms;
use bmo_receiver::receiver::wake::agents::{set_window, watch_window, watch_windows_after};

use crate::integration::common::Scratch;
use crate::integration::common::units::{AGENT, standing};

const ROOM: i64 = -100;
// An hour, an end no test reaches.
const LATER_MS: i64 = 3_600_000;

#[tokio::test]
async fn a_window_whose_end_passes_is_told_closed_once_at_the_end_that_stood() {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let standing = standing(scratch.db(), DataRoot::new(data.path()), vec![]);
  let receiver = standing.receiver.clone();
  let first = now_ms() - 20;
  let moved = first + 10;

  set_window(scratch.db(), ROOM, AGENT, first).await.unwrap();
  set_window(scratch.db(), ROOM, AGENT, moved).await.unwrap();

  let coming = watch_window(&receiver, -200, AGENT, now_ms() + LATER_MS);

  watch_window(&receiver, ROOM, AGENT, first).await.unwrap();
  assert!(
    standing.heard.events("window-closed").is_empty(),
    "a window whose end moved has not closed"
  );

  watch_window(&receiver, ROOM, AGENT, moved).await.unwrap();
  assert_eq!(
    standing.heard.events("window-closed"),
    [
      json!({ "type": "window-closed", "chatId": ROOM, "messageId": null, "agent": AGENT, "closedAt": moved })
    ],
  );
  assert!(!coming.is_finished(), "an end still to come is waited for");
  coming.abort();
}

#[tokio::test]
async fn the_ends_still_to_come_are_watched_as_the_receiver_starts() {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let standing = standing(scratch.db(), DataRoot::new(data.path()), vec![]);
  // The receiver starts at `started`: one end stands just past it, the other passed long before.
  let started = now_ms() - 20;
  let coming = started + 10;

  set_window(scratch.db(), ROOM, AGENT, coming).await.unwrap();
  set_window(scratch.db(), -200, AGENT, started - 1_000).await.unwrap();

  let watches = watch_windows_after(&standing.receiver, started).await.unwrap();

  assert_eq!(watches.len(), 1, "an end already past at the start is not watched");

  for watch in watches {
    watch.await.unwrap();
  }

  let closed = standing.heard.events("window-closed");

  assert_eq!(closed.len(), 1, "an end already past is not told again: {closed:?}");
  assert_eq!(
    (closed[0]["chatId"].as_i64(), closed[0]["closedAt"].as_i64()),
    (Some(ROOM), Some(coming))
  );
}

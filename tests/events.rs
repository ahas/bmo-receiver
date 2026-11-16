//! The stream's own ring, run and picture: the ids it numbers in its run, the cursor replayed only against the
//! run it was taken in and a reader naming no run started at the present, the last stretch kept, the events that
//! stand for a screen connecting later, the folds and queued messages drawn of what this process runs, and every
//! listener told as it comes.

use serde_json::json;
use tokio::sync::mpsc::UnboundedReceiver;

use bmo_receiver::events::{Events, RING, STOPPING_FRAME};

// The opening and the frames of a stream, taken under the one lock the stream keeps its listeners under.
fn frames_of(events: &Events, after: i64, run: Option<&str>) -> (String, UnboundedReceiver<String>) {
  events.frames(after, run)
}

#[test]
fn numbers_each_event_in_its_run_and_replays_only_after_a_cursor_of_that_run() {
  let events = Events::new();
  let first = events.publish(json!({ "type": "run-started", "chatId": -101, "messageId": 1 }));
  let second = events.publish(json!({ "type": "sent", "chatId": -101, "messageId": 2 }));
  let run = events.run();

  assert_eq!((first["id"].as_i64(), second["id"].as_i64()), (Some(1), Some(2)));

  let (same_run, _) = frames_of(&events, 1, Some(&run));

  assert!(same_run.starts_with(&format!("event: run\ndata: {}\n\n", json!({ "run": run }))));
  assert!(same_run.contains("id: 2\nevent: bot\n") && !same_run.contains("id: 1\n"));

  // A cursor of another run counts events that are not these; a reader naming no run has nothing to pick up.
  let (whole, _) = frames_of(&events, 1, Some("an-earlier-run"));
  let (fresh, _) = frames_of(&events, 0, None);

  assert!(whole.contains("id: 1\nevent: bot\n") && whole.contains("id: 2\nevent: bot\n"));
  assert!(!fresh.contains("id: "), "a reader naming no run starts at the present: {fresh}");
}

#[test]
fn hands_the_picture_after_the_replay_with_nothing_of_this_process_running_yet() {
  let events = Events::new();
  let (empty, _) = frames_of(&events, 0, None);

  assert!(empty.ends_with("\n\n") && empty.contains("event: live\ndata: {\"at\":"));
  assert_eq!(
    (events.picture()["turns"].clone(), events.picture()["queued"].clone()),
    (json!([]), json!([])),
  );

  events.begin_turn(json!({ "chatId": -104 }));

  let (opening, _) = frames_of(&events, 0, Some(&events.run()));
  let live = opening.rfind("event: live").unwrap();

  assert!(live > opening.find("id: 1\nevent: bot").unwrap());
  assert!(!opening[live - 10..live].contains("id:"));
  assert_eq!(events.picture()["turns"][0]["chatId"], -104);
}

#[test]
fn keeps_the_last_five_hundred() {
  let events = Events::new();

  for message in 0..RING + 3 {
    events.publish(json!({ "type": "sent", "chatId": 1, "messageId": message }));
  }

  let (opening, _) = frames_of(&events, 0, Some(&events.run()));

  assert!(!opening.contains("id: 3\n") && opening.contains("id: 4\n") && opening.contains("id: 503\n"));
}

#[test]
fn a_reader_starting_at_the_present_is_handed_each_working_turn_s_standing_picture_after_the_picture() {
  let events = Events::new();

  events.publish(json!({ "type": "sent", "chatId": -105, "messageId": 1 }));
  events.publish_standing(
    1,
    json!({ "type": "run-progress", "chatId": -105, "progress": { "phase": "tool" } }),
  );
  events.publish_standing(
    2,
    json!({ "type": "run-progress", "chatId": -106, "progress": { "phase": "tool" } }),
  );
  events.publish_standing(
    1,
    json!({ "type": "run-progress", "chatId": -105, "progress": { "phase": "writing" } }),
  );
  events.clear_standing(2);

  let (opening, _) = frames_of(&events, 0, None);
  let live = opening.find("event: live").unwrap();
  let standing = opening.find("id: 4\nevent: bot\n").unwrap();

  assert!(live < standing, "the picture names the turn first: {opening}");
  assert!(opening[standing..].contains("\"writing\""));
  for gone in ["id: 1\n", "id: 2\n", "id: 3\n"] {
    assert!(!opening.contains(gone), "{gone} is neither replayed nor standing: {opening}");
  }
}

#[test]
fn a_cursor_is_handed_a_standing_picture_the_ring_let_go_of_ahead_of_the_replay() {
  let events = Events::new();
  let run = events.run();

  events.publish_standing(
    1,
    json!({ "type": "run-progress", "chatId": -107, "progress": { "phase": "tool" } }),
  );

  for message in 0..RING {
    events.publish(json!({ "type": "sent", "chatId": 1, "messageId": message }));
  }

  let (behind, _) = frames_of(&events, 0, Some(&run));
  let (past_it, _) = frames_of(&events, 1, Some(&run));
  let standing = behind.find("id: 1\nevent: bot\n").unwrap();

  assert!(standing < behind.find("id: 2\nevent: bot\n").unwrap(), "the ids still rise: {behind}");
  assert!(!past_it.contains("id: 1\n"), "a reader past it saw it already");
}

#[test]
fn draws_its_folds_and_queued_messages_and_a_turn_reading_the_queued_one_takes_it() {
  let events = Events::new();

  events.begin_fold(-7);
  events.begin_fold(-105);
  events.queue_message(-7, 1);
  events.queue_message(-105, 9);

  let picture = events.picture();

  assert_eq!(picture["compacting"].as_array().map(Vec::len), Some(2));
  assert_eq!(picture["queued"].as_array().map(Vec::len), Some(2));
  assert_eq!(picture["queued"][0]["messageId"], 9);

  // A turn reading another message leaves the queued one standing; one reading it takes it.
  events.begin_turn(json!({ "chatId": -105, "messageId": 8 }));

  assert_eq!(events.picture()["queued"][0]["messageId"], 9);

  events.begin_turn(json!({ "chatId": -105, "messageId": 9 }));
  events.end_fold(-105);

  let picture = events.picture();

  assert_eq!(picture["queued"].as_array().map(Vec::len), Some(1));
  assert_eq!(picture["queued"][0]["chatId"], -7);
  assert_eq!(picture["compacting"].as_array().map(Vec::len), Some(1));
  assert_eq!(picture["compacting"][0]["chatId"], -7);

  // A turn shown as another speaker and reading the queued message takes it as any turn does: what says
  // the message was taken is that the turn read it.
  events.queue_message(-105, 10);
  events.begin_turn(json!({ "chatId": -105, "messageId": 10, "speaker": { "agent": "agent-d" } }));

  assert_eq!(events.picture()["queued"].as_array().map(Vec::len), Some(1));
  assert_eq!(events.picture()["queued"][0]["chatId"], -7);
}

#[test]
fn a_turn_whose_batch_carries_the_queued_message_takes_it() {
  let events = Events::new();

  events.queue_message(-5, 10);
  events.begin_turn(json!({ "chatId": -5, "messageId": 12, "messageIds": [11, 10, 12] }));

  assert_eq!(events.picture()["queued"].as_array().map(Vec::len), Some(0));
}

#[test]
fn a_turn_moved_on_to_words_it_took_while_it_works_reads_them_and_takes_the_queued_one() {
  let events = Events::new();
  let turn = events.begin_turn(json!({ "chatId": -5, "messageId": 10, "messageIds": [10], "messages": 1 }));

  events.queue_message(-5, 11);
  events.move_turn(turn, &[11]);

  let picture = events.picture();

  assert_eq!(picture["queued"], json!([]), "the turn read the queued message");
  assert_eq!(picture["turns"][0]["messageId"], 11, "it reads the newest it took last");
  assert_eq!(picture["turns"][0]["messageIds"], json!([10, 11]));
  assert_eq!(picture["turns"][0]["messages"], 2);
  assert_eq!(
    events.held().last().map(|event| event["type"].clone()),
    Some(json!("live")),
    "the picture went out again"
  );

  // A message that came after the words it took stands queued until the turn takes it too; a turn no longer in
  // the picture moves nothing.
  events.queue_message(-5, 13);
  events.move_turn(turn, &[12]);

  assert_eq!(events.picture()["queued"][0]["messageId"], 13);
  assert_eq!(events.picture()["turns"][0]["messageIds"], json!([10, 11, 12]));

  events.end_turn(turn);
  events.move_turn(turn, &[13]);

  assert_eq!(events.picture()["turns"], json!([]));
  assert_eq!(events.picture()["queued"][0]["messageId"], 13);
}

#[test]
fn a_room_whose_registration_is_removed_leaves_its_fold_and_its_queued_message() {
  let events = Events::new();

  events.begin_fold(-9);
  events.queue_message(-9, 4);
  events.begin_turn(json!({ "chatId": -9 }));
  events.forget_room(-9);

  let picture = events.picture();

  assert_eq!((picture["compacting"].clone(), picture["queued"].clone()), (json!([]), json!([])));
  // A round still running in the room ends on its own, and its turn with it.
  assert_eq!(picture["turns"].as_array().map(Vec::len), Some(1));
}

// A stream that is never read waits out the flush it is given before the process goes on to end, so the wait
// here is the real one and no clock is stood still.
#[tokio::test]
async fn tells_every_listener_as_it_comes_and_ends_each_stream_after_saying_it_is_stopping() {
  let events = Events::new();
  let (_, mut frames) = frames_of(&events, 0, None);
  let (_, gone) = frames_of(&events, 0, None);

  drop(gone);
  events.publish(json!({ "type": "message", "chatId": 0, "messageId": null }));

  assert!(frames.try_recv().unwrap().starts_with("id: 1\nevent: bot\ndata: "));

  // The stop waits until every stream told has let its listener go, which a stream does once the word is out.
  let stopped = tokio::spawn({
    let events = events.clone();

    async move { events.stopping().await }
  });

  assert_eq!(frames.recv().await.unwrap(), STOPPING_FRAME);
  assert!(frames.try_recv().is_err());
  assert!(!stopped.is_finished(), "the stop waits for the stream that has not let go");
  drop(frames);
  stopped.await.unwrap();

  // Nothing of the stop goes into the ring a later listener is replayed from.
  let (opening, _) = frames_of(&events, 0, None);

  assert!(!opening.contains("stopping"));
}

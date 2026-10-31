//! The chat receiver's event stream (docs/receiver.md, 「The event stream」): the ring of the last events, the run
//! their ids count, the picture of what the process runs, and every screen listening.
//!
//! Nothing here is stored. The archive keeps what was decided; this keeps the last stretch of activity, so a
//! screen that attaches late picks up from the id it last saw. The ids count this process's own run.

pub mod relay;
mod stream;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use axum::response::Response;
use futures_util::future::join_all;
use serde_json::{Value, json};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::time::timeout;

use crate::now_ms;
use crate::receiver::{News, Receiver};
use crate::room_record::RoomForget;
use crate::room_record::forget::Forgetting;

pub use relay::relay_caller;

// Long enough that a screen reconnecting after a dropped stream still finds where it was.
pub const RING: usize = 500;

pub const STOPPING_FRAME: &str = "event: stopping\ndata: {}\n\n";

// How long the word that the streams are stopping is given to leave before the process goes on to end.
const STOPPING_FLUSH: Duration = Duration::from_millis(250);

/// The stream and everything listening to it. Cloned freely: every clone is the one stream.
#[derive(Clone)]
pub struct Events {
  hub: Arc<Mutex<Hub>>,
  relays: Arc<relay::RelayBurst>,
}

struct Hub {
  // The moment the stream began, in milliseconds, which its run names.
  started: i64,
  run: String,
  next_id: i64,
  ring: VecDeque<Value>,
  // The turns this process runs, by the id each is moved and ended by.
  turns: std::collections::BTreeMap<u64, Value>,
  next_turn: u64,
  // The rooms whose session this process is folding, and since when.
  compacting: std::collections::BTreeMap<i64, i64>,
  // The last message each room queued for a turn of this process's that no turn has taken yet.
  queued: std::collections::BTreeMap<i64, Value>,
  listeners: Vec<UnboundedSender<String>>,
}

impl Default for Events {
  fn default() -> Self {
    Self::new()
  }
}

impl Events {
  /// A stream whose run is this moment.
  pub fn new() -> Self {
    let started = now_ms();

    Self {
      hub: Arc::new(Mutex::new(Hub {
        started,
        run: started.to_string(),
        next_id: 1,
        ring: VecDeque::new(),
        turns: std::collections::BTreeMap::new(),
        next_turn: 1,
        compacting: std::collections::BTreeMap::new(),
        queued: std::collections::BTreeMap::new(),
        listeners: Vec::new(),
      })),
      relays: Arc::new(relay::RelayBurst::default()),
    }
  }

  // A listener that panicked while holding the lock left nothing half written that matters more than
  // the stream going on.
  fn hub(&self) -> MutexGuard<'_, Hub> {
    self.hub.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
  }

  /// The run the ids count.
  pub fn run(&self) -> String {
    self.hub().run.clone()
  }

  /// The moment the stream began, in milliseconds: the process's start, as `GET /health` names it.
  pub fn started(&self) -> i64 {
    self.hub().started
  }

  /// One event put into the stream, numbered and stamped here, and told to every listener.
  pub fn publish(&self, event: Value) -> Value {
    self.hub().publish(event)
  }

  /// A screen's stream: the run, what the ring holds after its cursor where the cursor is this run's,
  /// the picture as it stands, then everything as it comes, with a ping between.
  pub fn listen(&self, after: i64, run: Option<&str>) -> Response {
    let (opening, frames) = self.frames(after, run);

    stream::response(opening, frames)
  }

  /// The opening a screen is handed, and every frame as the stream is told it: taken under the one lock, so
  /// nothing published can fall between the replay and the listening.
  pub fn frames(&self, after: i64, run: Option<&str>) -> (String, UnboundedReceiver<String>) {
    self.hub().listen(after, run)
  }

  /// The events the ring holds, oldest first.
  pub fn held(&self) -> Vec<Value> {
    self.hub().ring.iter().cloned().collect()
  }

  /// The picture of what the process runs, as it stands.
  pub fn picture(&self) -> Value {
    self.hub().picture_now()
  }

  /// Every stream told this process is stopping on purpose, then ended: each ends once the word has
  /// gone out on it, and this returns once every one has, or once the flush has passed.
  pub async fn stopping(&self) {
    let told: Vec<UnboundedSender<String>> = self
      .hub()
      .listeners
      .drain(..)
      .filter(|listener| listener.send(STOPPING_FRAME.to_owned()).is_ok())
      .collect();
    let gone = join_all(told.iter().map(UnboundedSender::closed));

    let _ = timeout(STOPPING_FLUSH, gone).await;
  }

  /// One relay of the news counted; the line to write where it completed a burst.
  pub fn relay_seen(&self, who: &str) -> Option<String> {
    self.relays.saw(who, now_ms())
  }

  /// A turn of this process's own begun in a room, as the picture's turns hold one: the id it is moved and
  /// ended by. The picture goes out again with it.
  pub fn begin_turn(&self, turn: Value) -> u64 {
    let mut hub = self.hub();
    let id = hub.next_turn;

    hub.next_turn += 1;
    hub.take_queued(&turn);
    hub.turns.insert(id, turn);
    hub.redraw();

    id
  }

  /// A turn of this process's own moved on to messages of its room it read while it works, pushed into it or shown
  /// under a question: it reads the newest of them last, among the rest it read, and a queued one among them is
  /// taken (docs/receiver.md, 「The event stream」). The picture goes out again with it.
  pub fn move_turn(&self, id: u64, message_ids: &[i64]) {
    let mut hub = self.hub();
    let Some(turn) = hub.turns.get_mut(&id) else {
      return;
    };
    let Some(newest) = message_ids.last() else {
      return;
    };
    let mut read: Vec<i64> =
      turn["messageIds"].as_array().into_iter().flatten().filter_map(Value::as_i64).collect();
    let fresh: Vec<i64> = message_ids.iter().copied().filter(|id| !read.contains(id)).collect();
    let messages = turn["messages"].as_u64().unwrap_or_default() + fresh.len() as u64;

    read.extend(fresh);
    turn["messageId"] = json!(newest);
    turn["messageIds"] = json!(read);
    turn["messages"] = json!(messages);

    let turn = turn.clone();

    hub.take_queued(&turn);
    hub.redraw();
  }

  /// A turn of this process's own over.
  pub fn end_turn(&self, id: u64) {
    let mut hub = self.hub();

    if hub.turns.remove(&id).is_some() {
      hub.redraw();
    }
  }

  /// A room's session being folded by this process, from now.
  pub fn begin_fold(&self, chat_id: i64) {
    let mut hub = self.hub();

    hub.compacting.insert(chat_id, now_ms() / 1000);
    hub.redraw();
  }

  /// A room's fold over.
  pub fn end_fold(&self, chat_id: i64) {
    let mut hub = self.hub();

    if hub.compacting.remove(&chat_id).is_some() {
      hub.redraw();
    }
  }

  /// A message put into its room's queue for a turn of this process's, which stands as the room's queued one
  /// until a turn takes it or another is put in after it (docs/receiver.md, 「The event stream」).
  pub fn queue_message(&self, chat_id: i64, message_id: i64) {
    let mut hub = self.hub();

    hub
      .queued
      .insert(chat_id, json!({ "chatId": chat_id, "messageId": message_id, "since": now_ms() / 1000 }));
    hub.redraw();
  }

  /// A room's queued message let go unread.
  pub fn drop_queued(&self, chat_id: i64) {
    let mut hub = self.hub();

    if hub.queued.remove(&chat_id).is_some() {
      hub.redraw();
    }
  }

  /// A room drawn no more, as its registration is removed: neither its fold nor its queued message.
  pub fn forget_room(&self, chat_id: i64) {
    let mut hub = self.hub();
    let folding = hub.compacting.remove(&chat_id).is_some();

    if hub.queued.remove(&chat_id).is_some() || folding {
      hub.redraw();
    }
  }

  /// The room's screens told that a message was kept or written again.
  pub fn message(&self, chat_id: i64, message_id: i64) {
    self.publish(message(chat_id, message_id));
  }

  /// The room's screens told that its window for an agent was opened or restarted.
  pub fn window(&self, chat_id: i64) {
    self.publish(window(chat_id));
  }

  /// The room's screens told that its window for an agent closed, and when.
  pub fn window_closed(&self, chat_id: i64, agent: &str, at: i64) {
    self.publish(window_closed(chat_id, agent, at));
  }

  /// A room whose registration was removed, so the sidebar drops it and an open room screen leaves it.
  pub fn room_removed(&self, chat_id: i64) {
    self.publish(room_removed(chat_id));
  }

  /// The profile pictures of the rooms and people moved, taken by a chat service.
  pub fn avatars(&self) {
    self.publish(avatars());
  }
}

impl Hub {
  fn publish(&mut self, mut event: Value) -> Value {
    let id = self.next_id;

    self.next_id += 1;
    event["id"] = json!(id);
    event["at"] = json!(now_ms() / 1000);

    self.ring.push_back(event.clone());

    while self.ring.len() > RING {
      self.ring.pop_front();
    }

    let frame = event_frame(&event);

    self.listeners.retain(|listener| listener.send(frame.clone()).is_ok());

    event
  }

  // The picture goes after the replay: the ring may hold an earlier picture of the same run, and a
  // screen keeps whichever arrives last.
  fn listen(&mut self, after: i64, run: Option<&str>) -> (String, UnboundedReceiver<String>) {
    let after = if run == Some(self.run.as_str()) { after } else { 0 };
    let mut opening = format!("event: run\ndata: {}\n\n", json!({ "run": self.run }));

    for event in self.ring.iter().filter(|event| event["id"].as_i64().is_some_and(|id| id > after)) {
      opening.push_str(&event_frame(event));
    }

    opening.push_str(&format!("event: live\ndata: {}\n\n", self.picture_now()));

    let (listener, frames) = unbounded_channel();

    self.listeners.push(listener);

    (opening, frames)
  }

  // The picture of what runs: this process's turns, its folds and each room's queued message.
  fn picture_now(&self) -> Value {
    let folds: Vec<Value> = self
      .compacting
      .iter()
      .map(|(chat, since)| json!({ "chatId": chat, "since": since }))
      .collect();

    json!({
      "at": now_ms() / 1000,
      "turns": self.turns.values().cloned().collect::<Vec<_>>(),
      "compacting": folds,
      "queued": self.queued.values().cloned().collect::<Vec<_>>(),
    })
  }

  // A turn that read the room's queued message has taken it; one reading another leaves it standing. What
  // says it was taken is the message standing among the turn's own, whether or not the room's road gives the
  // turn a speaker to be shown as — any agent's turn reading the message answered it as surely as the chat
  // agent's.
  fn take_queued(&mut self, turn: &Value) {
    let Some(chat) = turn["chatId"].as_i64() else {
      return;
    };

    if !self.queued.contains_key(&chat) {
      return;
    }

    let reading = turn["messageId"].as_i64();
    let read: Vec<i64> =
      turn["messageIds"].as_array().into_iter().flatten().filter_map(Value::as_i64).collect();
    let taken = self.queued.get(&chat).is_some_and(|queued| {
      queued["messageId"].as_i64().is_some_and(|id| Some(id) == reading || read.contains(&id))
    });

    if taken {
      self.queued.remove(&chat);
    }
  }

  // The picture sent out again as it stands.
  fn redraw(&mut self) {
    let picture = self.picture_now();

    self.publish(live_event(picture));
  }
}

fn event_frame(event: &Value) -> String {
  format!("id: {}\nevent: bot\ndata: {event}\n\n", event["id"])
}

fn live_event(picture: Value) -> Value {
  json!({ "type": "live", "chatId": 0, "messageId": null, "live": picture })
}

/// A room's screen told that a message was kept or written again.
pub fn message(chat_id: i64, message_id: i64) -> Value {
  json!({ "type": "message", "chatId": chat_id, "messageId": message_id })
}

/// The room's screens told that its window for an agent was opened or restarted (docs/wake.md, 「The names
/// and the window」).
pub fn window(chat_id: i64) -> Value {
  json!({ "type": "window", "chatId": chat_id, "messageId": null })
}

/// The room's screens told that its window for an agent closed, by its time or early, at `at` in epoch
/// milliseconds (docs/wake.md, 「The names and the window」).
pub fn window_closed(chat_id: i64, agent: &str, at: i64) -> Value {
  json!({ "type": "window-closed", "chatId": chat_id, "messageId": null, "agent": agent, "closedAt": at })
}

/// A verdict the receiver wrote of a round, as the gate keeps it: the room, the message it stands under where it
/// names one, the stage, the verdict and what it said beside it.
pub fn judgement(
  chat_id: i64,
  message_id: Option<i64>,
  stage: &str,
  verdict: &str,
  detail: Option<&str>,
) -> Value {
  json!({
    "type": "judgement",
    "chatId": chat_id,
    "messageId": message_id,
    "stage": stage,
    "verdict": verdict,
    "detail": detail,
  })
}

/// One row of a round's draft process, as the round log keeps it (docs/drafting.md, 「The round log」).
pub fn draft(chat_id: i64, row: &Value) -> Value {
  let mut event = json!({ "type": "draft", "chatId": chat_id, "messageId": null });

  if let (Some(event), Some(row)) = (event.as_object_mut(), row.as_object()) {
    for (key, value) in row {
      if key != "answer" {
        event.insert(key.clone(), value.clone());
      }
    }
  }

  event
}

/// A room whose registration was removed, so the sidebar drops it and an open room screen leaves it
/// (docs/receiver.md, 「Removing a room's registration」).
pub fn room_removed(chat_id: i64) -> Value {
  json!({ "type": "room-removed", "chatId": chat_id, "messageId": null })
}

/// An act the record kept on a message: a reaction, a pin, an edit or a delete.
pub fn message_changed(chat_id: i64, message_id: i64) -> Value {
  json!({ "type": "message-changed", "chatId": chat_id, "messageId": message_id })
}

/// The profile pictures of the rooms and people moved, taken by a chat service.
pub fn avatars() -> Value {
  json!({ "type": "avatars-updated", "chatId": 0, "messageId": null })
}

// A room whose registration is removed leaves the picture.
fn forget_room(receiver: &Receiver, room: i64) -> Forgetting<'_> {
  receiver.news().forget_room(room);
  Box::pin(async {})
}

crate::room_forget!(RoomForget {
  what: "the room's fold and queued message in the live picture",
  forget: forget_room,
});

/// The stream, heard as the receiver's own news: every part of it the receiver makes.
impl News for Events {
  fn message(&self, chat_id: i64, message_id: i64) {
    self.message(chat_id, message_id);
  }

  fn window(&self, chat_id: i64) {
    self.window(chat_id)
  }

  fn window_closed(&self, chat_id: i64, agent: &str, at: i64) {
    self.window_closed(chat_id, agent, at)
  }

  fn room_removed(&self, chat_id: i64) {
    self.room_removed(chat_id)
  }

  fn publish(&self, event: &Value) {
    let _ = self.publish(event.clone());
  }

  fn queue_message(&self, chat_id: i64, message_id: i64) {
    self.queue_message(chat_id, message_id)
  }

  fn drop_queued(&self, chat_id: i64) {
    self.drop_queued(chat_id)
  }

  fn begin_fold(&self, chat_id: i64) {
    self.begin_fold(chat_id)
  }

  fn end_fold(&self, chat_id: i64) {
    self.end_fold(chat_id)
  }

  fn forget_room(&self, chat_id: i64) {
    self.forget_room(chat_id)
  }

  fn begin_turn(&self, turn: &Value) -> u64 {
    self.begin_turn(turn.clone())
  }

  fn move_turn(&self, id: u64, message_ids: &[i64]) {
    self.move_turn(id, message_ids)
  }

  fn end_turn(&self, id: u64) {
    self.end_turn(id)
  }

  fn picture(&self) -> Value {
    self.hub().picture_now()
  }

  fn held(&self) -> Vec<Value> {
    self.hub().ring.iter().cloned().collect()
  }

  fn started(&self) -> i64 {
    self.hub().started
  }

  fn listen(&self, after: i64, run: Option<&str>) -> Response {
    self.listen(after, run)
  }
}

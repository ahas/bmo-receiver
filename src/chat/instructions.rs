//! The instruction pass (docs/bmo-window.md, 「BMO's queue」, 「Rows that ride」): every conversational agent's
//! pending rows handed to that agent's conversation in their rooms — on the tick, at once where the receiver
//! queued one itself, and within a moment of a row another process wrote — a cold wake held until the gap since
//! the last has passed, a row no turn could ever take let go, a rider left for the next round its room opens and
//! handed to it as that round starts, and, as the process starts, every row a stopped round left handed and
//! unsettled made pending again.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use receiver_policy::chat_round::CHAT_ROUND;
use sea_orm::{DatabaseConnection, TransactionTrait};
use tokio::sync::Notify;

use super::conversation::{BoxFuture, Conversations, unified_session};
use super::runner::Runner;
use crate::js;
use crate::receiver::{Receiver, wake};
use crate::{Fault, all, now_ms, one, run};
use receiver_ports::agency::Scope;
use receiver_ports::gate::TurnEvent;

/// What the pass hands its rows to: each agent's conversations in their rooms.
pub trait Rooms: Send + Sync {
  /// A quiet instruction for an agent in a room, with the row it was queued as, the road it names and its events.
  fn instruct(
    &self,
    agent: &str,
    chat_id: i64,
    text: String,
    id: i64,
    via: Option<String>,
    events: Vec<TurnEvent>,
  ) -> BoxFuture<'_, ()>;

  /// Whether the agent's conversation already stands in the room, so a row for it wakes nothing cold.
  fn is_active(&self, agent: &str, chat_id: i64) -> bool;

  /// Whether the agent's conversations stand at all, so a row of its own is not written for nothing.
  fn standing(&self, agent: &str) -> bool;

  /// Whether the room has fallen silent for the agent: the rest closes it and nothing else would answer it.
  fn silenced(&self, agent: &str, chat_id: i64) -> BoxFuture<'_, bool>;
}

impl Rooms for Runner {
  fn instruct(
    &self,
    agent: &str,
    chat_id: i64,
    text: String,
    id: i64,
    via: Option<String>,
    events: Vec<TurnEvent>,
  ) -> BoxFuture<'_, ()> {
    let conversations = self.of(agent);

    Box::pin(async move { conversations.instruct(chat_id, text, Some(id), via, events).await })
  }

  fn is_active(&self, agent: &str, chat_id: i64) -> bool {
    self.of(agent).is_active(chat_id)
  }

  fn standing(&self, agent: &str) -> bool {
    self.standing(agent).is_some()
  }

  fn silenced(&self, agent: &str, chat_id: i64) -> BoxFuture<'_, bool> {
    let conversations: Conversations = self.of(agent);

    Box::pin(async move { conversations.silenced(chat_id).await })
  }
}

// The event kinds a row may wait on, and which of them names an item (docs/bmo-window.md, 「Events」).
const EVENT_KINDS: [(&str, bool); 3] = [("deployed", true), ("job-ended", true), ("queue-changed", false)];

// One event of the column's comma-joined form, `kind` or `kind:#item`; an unknown kind is no event.
fn read_event(text: &str) -> Option<TurnEvent> {
  let parts: Vec<&str> = js::trim(text).split(':').collect();
  let head = js::trim(parts.first().map_or("", |one| *one)).to_owned();
  let (kind, names_item) = EVENT_KINDS
    .iter()
    .find(|(one, _)| *one == head.as_str())
    .map(|(one, two)| (one.to_owned(), *two))?;

  match parts.as_slice() {
    [_] => Some(TurnEvent {
      kind: kind.to_string(),
      item: None,
    }),
    [_, item] if names_item => {
      let item = js::trim(item).trim_start_matches('#').parse::<i64>().ok().filter(|one| *one > 0)?;

      Some(TurnEvent {
        kind: kind.to_string(),
        item: Some(item),
      })
    }
    _ => None,
  }
}

// One pending row of an agent's, as the pass reads it.
struct Row {
  id: i64,
  agent: String,
  chat_id: i64,
  text: String,
  expires_at: Option<i64>,
  restarts: i64,
  via: Option<String>,
  events: Vec<TurnEvent>,
  // None for a row that opens a turn; for a rider, until when in epoch seconds where it says, and whether any
  // room its session answers may carry it.
  rides: Option<(Option<i64>, bool)>,
}

impl Row {
  // Whether it opens a turn of its own at this moment: one queued to, or a rider whose while is up.
  fn opens_turn(&self, now_ms: i64) -> bool {
    match self.rides {
      None => true,
      Some((until, _)) => until.is_some_and(|until| until * 1000 <= now_ms),
    }
  }
}

async fn pending(db: &DatabaseConnection) -> Result<Vec<Row>, Fault> {
  let rows = all(
    db,
    r#"SELECT "id", "agent", "chat_id", "text", "expires_at", "restarts", "via", "events", "rides", "ride_until"
       FROM "instructions" WHERE "delivered_at" IS NULL AND "handed_at" IS NULL ORDER BY "id""#,
    vec![],
  )
    .await?;
  let mut read = Vec::new();

  for row in rows {
    let rides: i64 = row.try_get("", "rides")?;
    let events: Option<String> = row.try_get("", "events")?;

    read.push(Row {
      id: row.try_get("", "id")?,
      agent: row.try_get("", "agent")?,
      chat_id: row.try_get("", "chat_id")?,
      text: row.try_get("", "text")?,
      expires_at: row.try_get("", "expires_at")?,
      restarts: row.try_get("", "restarts")?,
      via: row.try_get("", "via")?,
      events: events
        .unwrap_or_default()
        .split(',')
        .filter(|one| !one.trim().is_empty())
        .filter_map(read_event)
        .collect(),
      rides: (rides != 0).then(|| (row.try_get("", "ride_until").ok().flatten(), rides == 2)),
    });
  }

  Ok(read)
}

async fn discard(db: &DatabaseConnection, id: i64) -> Result<(), Fault> {
  run(db, r#"DELETE FROM "instructions" WHERE "id" = $1"#, vec![id.into()])
    .await
    .map(drop)
}

// The scope a room's turns go on: the agent's one session where its registration says so, the room's own
// otherwise.
async fn session_of(receiver: &Receiver, agent: &str, chat_id: i64) -> Scope {
  if unified_session(receiver, agent).await {
    return Scope::Agent;
  }

  Scope::Room { room: chat_id }
}

// A row no turn could ever take is let go rather than left waiting: one past its expiry, whose news may no
// longer be true, and one for a room its agent does not answer, or of an agent that holds no conversation.
async fn deliverable(
  receiver: &Receiver,
  rooms: &dyn Rooms,
  row: &Row,
  now_ms: i64,
) -> Result<bool, Fault> {
  if row.expires_at.is_some_and(|at| at * 1000 <= now_ms) {
    println!("instruction {} for chat {} expired undelivered, discarding", row.id, row.chat_id);
    discard(receiver.db(), row.id).await?;

    return Ok(false);
  }

  if !rooms.standing(&row.agent) {
    println!("instruction {} of {} has no conversation to take it, discarding", row.id, row.agent);
    discard(receiver.db(), row.id).await?;

    return Ok(false);
  }

  let room = receiver.roads().room_facts(row.chat_id).await;
  let answerers = wake::agents::room_designated(receiver, row.chat_id, room).await.ok();
  let answers = match answerers {
    Some((answerers, _)) => answerers.iter().any(|agent| agent.name == row.agent),
    None => false,
  };

  if !answers {
    println!(
      "instruction {} of {} for chat {} has no conversation to take it, discarding",
      row.id, row.agent, row.chat_id
    );
    discard(receiver.db(), row.id).await?;

    return Ok(false);
  }

  Ok(true)
}

// A row goes to its own room's conversation, or to the one whose round it rides, marked handed as it goes.
async fn hand(receiver: &Receiver, rooms: &dyn Rooms, row: &Row, chat_id: i64) -> Result<(), Fault> {
  run(
    receiver.db(),
    r#"UPDATE "instructions" SET "handed_at" = $1 WHERE "id" = $2"#,
    vec![(now_ms() / 1000).into(), row.id.into()],
  )
  .await?;

  let text = if row.restarts > 0 {
    format!("{}\n\n{}", CHAT_ROUND.instructions.restarted, row.text)
  } else {
    row.text.clone()
  };

  rooms
    .instruct(&row.agent, chat_id, text, row.id, row.via.clone(), row.events.clone())
    .await;

  Ok(())
}

/// One pass over the pending rows: each that opens a turn handed to its room, a room fallen silent keeping its
/// rows, and a cold wake held where the last was within the gap; whether any was held, so the next pass comes
/// round at the gap rather than the tick.
pub async fn deliver_pending(
  receiver: &Receiver,
  rooms: &dyn Rooms,
  last_cold_wake: &mut i64,
  now_ms: i64,
  gap_ms: i64,
) -> Result<bool, Fault> {
  let db = receiver.db();
  let mut held = false;

  for row in pending(db).await? {
    if !deliverable(receiver, rooms, &row, now_ms).await? || !row.opens_turn(now_ms) {
      continue;
    }

    if rooms.silenced(&row.agent, row.chat_id).await {
      continue;
    }

    let cold = !rooms.is_active(&row.agent, row.chat_id);

    if cold && now_ms - *last_cold_wake < gap_ms {
      held = true;
      continue;
    }

    if cold {
      *last_cold_wake = now_ms;
    }

    hand(receiver, rooms, &row, row.chat_id).await?;
  }

  Ok(held)
}

/// The rows of an agent's that ride a round of this room, handed to it as the round starts: the room's own
/// riders, and a rider of any room the same session answers where the row says the whole session may carry it.
/// The rows handed.
pub async fn deliver_riders(
  receiver: &Receiver,
  rooms: &dyn Rooms,
  agent: &str,
  chat_id: i64,
  now_ms: i64,
) -> Result<Vec<i64>, Fault> {
  let session = session_of(receiver, agent, chat_id).await;
  let db = receiver.db();
  let mut handed = Vec::new();

  for row in pending(db).await?.into_iter().filter(|row| row.agent == agent) {
    let rideable = match row.rides {
      None => false,
      Some((_, whole_session)) => {
        row.chat_id == chat_id
          || (whole_session && session_of(receiver, agent, row.chat_id).await == session)
      }
    };

    if !rideable || !deliverable(receiver, rooms, &row, now_ms).await? {
      continue;
    }

    handed.push(row.id);
    hand(receiver, rooms, &row, chat_id).await?;
  }

  Ok(handed)
}

/// What a stopped process left handed and unsettled goes back to pending before anything is delivered, each
/// row given back at most the policy's times before it is dropped.
pub async fn requeue_interrupted(db: &DatabaseConnection) -> Result<(), Fault> {
  let rows = all(
    db,
    r#"SELECT "id", "restarts" FROM "instructions" WHERE "handed_at" IS NOT NULL AND "delivered_at" IS NULL ORDER BY "id""#,
    vec![],
  )
    .await?;
  let mut requeued = Vec::new();
  let mut dropped = Vec::new();

  for row in rows {
    let id: i64 = row.try_get("", "id")?;
    let restarts: i64 = row.try_get("", "restarts")?;

    if restarts >= CHAT_ROUND.instructions.restart_limit {
      discard(db, id).await?;
      dropped.push(id.to_string());
    } else {
      run(
        db,
        r#"UPDATE "instructions" SET "handed_at" = NULL, "restarts" = "restarts" + 1 WHERE "id" = $1"#,
        vec![id.into()],
      )
      .await?;
      requeued.push(id.to_string());
    }
  }

  if !requeued.is_empty() {
    println!("instructions {} were cut off by a restart and are pending again", requeued.join(", "));
  }

  if !dropped.is_empty() {
    eprintln!(
      "instructions {} were cut off by restarts {} times and are dropped",
      dropped.join(", "),
      CHAT_ROUND.instructions.restart_limit
    );
  }

  Ok(())
}

async fn newest(db: &DatabaseConnection) -> i64 {
  one(db, r#"SELECT MAX("id") AS "id" FROM "instructions""#, vec![])
    .await
    .ok()
    .flatten()
    .and_then(|row| row.try_get::<Option<i64>>("", "id").ok().flatten())
    .unwrap_or(0)
}

/// Everything a quiet instruction may carry beyond its text, each under the column it is kept in.
#[derive(Default)]
pub struct Queued<'a> {
  /// A kind replaces the pending row of the same kind in the chat.
  pub kind: Option<&'a str>,
  /// When it lapses, in epoch milliseconds.
  pub expires_at_ms: Option<i64>,
  /// The events its delivery carries, each as the column keeps one.
  pub events: Vec<String>,
  /// Whether it rides the next turn rather than opening one, and until when in epoch milliseconds.
  pub rides_until_ms: Option<i64>,
  /// Whether it rides a round of any room its session answers rather than its own room's alone: such a row rides
  /// until `rides_until_ms`, and for good where that names no moment.
  pub rides_session: bool,
  pub sort: Option<&'a str>,
  pub summary: Option<&'a str>,
  /// The road the session it is for was read on, which it is delivered on.
  pub via: Option<&'a str>,
  /// The run whose result it is, which a call waiting on that run takes it by.
  pub run_id: Option<i64>,
}

// How a row rides as its column keeps it: not at all, its own room's rounds, or any room's its session answers.
fn rides(queued: &Queued<'_>) -> i64 {
  match (queued.rides_session, queued.rides_until_ms) {
    (true, _) => 2,
    (false, Some(_)) => 1,
    (false, None) => 0,
  }
}

/// The instruction written for one agent's session in the chat with everything it carries, a pending row of the
/// same kind in the chat replaced; its id.
pub async fn queue(
  db: &DatabaseConnection,
  agent: &str,
  chat_id: i64,
  text: &str,
  queued: &Queued<'_>,
) -> Result<i64, Fault> {
  let transaction = db.begin().await.map_err(Fault::from)?;

  if let Some(kind) = queued.kind {
    run(
      &transaction,
      r#"DELETE FROM "instructions" WHERE "chat_id" = $1 AND "kind" = $2 AND "delivered_at" IS NULL AND "handed_at" IS NULL"#,
      vec![chat_id.into(), kind.into()],
    )
      .await?;
  }

  let written = one(
    &transaction,
    r#"INSERT INTO "instructions" ("agent", "chat_id", "text", "created_at", "kind", "expires_at", "via", "events", "rides", "ride_until", "sort", "summary", "run_id")
       VALUES ($1, $2, $3, $4, $5, $6, $12, $7, $8, $9, $10, $11, $13)
       RETURNING "id""#,
    vec![
      agent.into(),
      chat_id.into(),
      text.into(),
      (now_ms() / 1000).into(),
      queued.kind.into(),
      queued.expires_at_ms.map(|at| at.div_euclid(1000)).into(),
      queued.events.join(",").into(),
      rides(queued).into(),
      queued.rides_until_ms.map(|at| at.div_euclid(1000)).into(),
      queued.sort.into(),
      queued.summary.into(),
      queued.via.into(),
      queued.run_id.into(),
    ],
  )
    .await?;
  let Some(written) = written else {
    return Err(Fault::new("the instruction was not written"));
  };
  let id = written.try_get::<i64>("", "id")?;

  transaction.commit().await.map_err(Fault::from)?;

  Ok(id)
}

/// The pass as the running process keeps it.
pub struct Pass {
  receiver: Receiver,
  rooms: Arc<dyn Rooms>,
  last_cold_wake: Mutex<i64>,
  kicked: Notify,
}

impl Pass {
  pub fn new(receiver: Receiver, rooms: Arc<dyn Rooms>) -> Arc<Self> {
    Arc::new(Self {
      receiver,
      rooms,
      last_cold_wake: Mutex::new(0),
      kicked: Notify::new(),
    })
  }

  /// The next pass brought forward to now, for a row the receiver queued that should not wait for the tick.
  pub fn kick(&self) {
    self.kicked.notify_one();
  }

  /// The rows of an agent's riding a round handed to it as the round starts.
  pub async fn riders(&self, agent: &str, chat_id: i64) {
    if let Err(why) = deliver_riders(&self.receiver, self.rooms.as_ref(), agent, chat_id, now_ms()).await {
      eprintln!("instructions: {}", why.detail);
    }
  }

  async fn once(&self) -> bool {
    let mut last = *self.last_cold_wake.lock().unwrap_or_else(PoisonError::into_inner);
    let held = deliver_pending(
      &self.receiver,
      self.rooms.as_ref(),
      &mut last,
      now_ms(),
      CHAT_ROUND.instructions.cold_wake_gap_ms,
    )
    .await;

    *self.last_cold_wake.lock().unwrap_or_else(PoisonError::into_inner) = last;

    held.unwrap_or_else(|why| {
      eprintln!("instructions: {}", why.detail);

      false
    })
  }

  /// The pass for as long as the process stands: the rows a stopped process left handed made pending first,
  /// then a pass at once and one each tick after — at the gap where a cold wake was held — brought forward by
  /// a kick or by a row newer than any seen.
  pub fn keep(self: Arc<Self>) {
    let policy = &CHAT_ROUND.instructions;
    let watching = self.clone();

    tokio::spawn(async move {
      if let Err(why) = requeue_interrupted(self.receiver.db()).await {
        eprintln!("instructions: {}", why.detail);
      }

      loop {
        let held = self.once().await;
        let wait = if held {
          Duration::from_millis(policy.cold_wake_gap_ms.max(0) as u64)
        } else {
          Duration::from_secs(policy.tick_seconds)
        };

        tokio::select! {
          () = tokio::time::sleep(wait) => {}
          () = self.kicked.notified() => {}
        }
      }
    });

    tokio::spawn(async move {
      let mut seen = newest(watching.receiver.db()).await;

      loop {
        tokio::time::sleep(Duration::from_millis(policy.watch_ms)).await;

        let now = newest(watching.receiver.db()).await;

        if now > seen {
          seen = now;
          watching.kick();
        }
      }
    });
  }
}

impl crate::receiver::Pass for Pass {
  fn kick(&self) {
    self.kicked.notify_one();
  }

  fn riders<'a>(&'a self, agent: &'a str, chat_id: i64) -> BoxFuture<'a, ()> {
    Box::pin(async move { self.riders(agent, chat_id).await })
  }
}

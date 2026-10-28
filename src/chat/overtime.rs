//! A run past its clock, as the agency asks about it (docs/voiced-notices.md, 「A turn past its clock」): the order
//! to keep or cut it queued for the chat agent in the owner's own chat on `overtime-request.created`, under the
//! run's own kind, and withdrawn on `overtime-request.changed` where no round has taken it yet.

use chrono::DateTime;
use futures_util::future::BoxFuture;
use receiver_policy::fill_marks;
use receiver_policy::owner_facts::OWNER_FACTS;
use receiver_ports::agency::Event;
use serde::Deserialize;

use super::instructions::{Queued, queue};
use super::restart::kick;
use crate::hearing::Hearer;
use crate::receiver::Receiver;
use crate::store::Fault;
use crate::{now_ms, run};

/// The kind an ask on a clocked run is published under.
pub const ASKED: &str = "overtime-request.created";

/// The kind an ask settled is published under.
pub const CLOSED: &str = "overtime-request.changed";

/// An ask on a run past its clock, as the agency publishes it: times in epoch milliseconds.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asked {
  pub ask: i64,
  #[serde(default)]
  pub agent: String,
  pub run: String,
  #[serde(default)]
  pub what: String,
  #[serde(default)]
  pub started_at: Option<i64>,
  #[serde(default)]
  pub bound_ms: Option<i64>,
  #[serde(default)]
  pub last_activity_at: Option<i64>,
  #[serde(default)]
  pub last_activity: Option<String>,
  pub answer_by: i64,
  pub verb: String,
}

/// An ask settled: the run it was on.
#[derive(Debug, Deserialize)]
pub struct Closed {
  pub run: String,
}

/// What a duration reads as in an order: whole minutes, seconds, or both.
pub fn spoken(ms: i64) -> String {
  let seconds = (ms.max(0) as f64 / 1000.0).round() as i64;

  if seconds % 60 == 0 {
    return if seconds == 60 { "1 minute".to_owned() } else { format!("{} minutes", seconds / 60) };
  }

  if seconds < 60 {
    return format!("{seconds} seconds");
  }

  format!("{} minutes {} seconds", seconds.div_euclid(60), seconds % 60)
}

/// A moment as an order names it, in UTC.
pub fn moment(ms: i64) -> String {
  DateTime::from_timestamp_millis(ms)
    .map(|at| at.format("%Y-%m-%d %H:%M:%S UTC").to_string())
    .unwrap_or_else(|| ms.to_string())
}

/// The order an ask hands the chat agent, written at the moment it is heard (`now`, in epoch milliseconds).
pub fn order(asked: &Asked, now: i64) -> String {
  let said = &OWNER_FACTS.overtime;
  let ask = asked.ask.to_string();
  let activity = match asked.last_activity_at {
    None => asked.last_activity.clone().unwrap_or_else(|| said.activity_none.clone()),
    Some(at) => fill_marks(
      &said.activity_at,
      &[
        ("at", &moment(at)),
        ("before", &spoken(now - at)),
        ("what", asked.last_activity.as_deref().unwrap_or(&said.activity_unknown)),
      ],
    ),
  };
  let started = asked.started_at.unwrap_or(now);
  let mut lines = vec![
    said.head.clone(),
    fill_marks(&said.ask, &[("ask", &ask)]),
    fill_marks(&said.what, &[("what", &asked.what)]),
    fill_marks(&said.run, &[("agent", &asked.agent), ("run", &asked.run)]),
    fill_marks(&said.running, &[("running", &spoken(now - started)), ("since", &moment(started))]),
  ];

  if let Some(bound) = asked.bound_ms {
    lines.push(fill_marks(&said.bound, &[("bound", &spoken(bound))]));
  }

  lines.push(fill_marks(&said.activity, &[("activity", &activity)]));
  lines.push(fill_marks(&said.nothing_cut, &[("by", &moment(asked.answer_by))]));
  lines.push(String::new());
  lines.extend(said.steps.iter().map(|step| fill_marks(step, &[("verb", &asked.verb), ("ask", &ask)])));

  lines.join("\n")
}

/// The kind the order of one run stands under.
pub fn kind(run: &str) -> String {
  format!("{}:{run}", OWNER_FACTS.overtime.kind)
}

/// The order queued for the chat agent in the owner's chat, in place of one for the same run not yet handed over.
pub async fn ask(receiver: &Receiver, asked: &Asked) -> Result<i64, Fault> {
  let kind = kind(&asked.run);
  let queued = Queued {
    kind: Some(&kind),
    sort: Some(&OWNER_FACTS.overtime.sort),
    summary: Some(&asked.what),
    agent_only: true,
    ..Queued::default()
  };

  queue(
    receiver.db(),
    &receiver.deployment().agent,
    receiver.notices_room(),
    &order(asked, now_ms()),
    &queued,
  )
  .await
}

/// The order for the run withdrawn where no round has taken it; how many rows went.
pub async fn withdraw(receiver: &Receiver, closed: &Closed) -> Result<u64, Fault> {
  run(
    receiver.db(),
    r#"DELETE FROM "instructions" WHERE "chat_id" = $1 AND "kind" = $2 AND "handed_at" IS NULL AND "delivered_at" IS NULL"#,
    vec![receiver.notices_room().into(), kind(&closed.run).into()],
  )
  .await
}

/// The asks as the service hears them on the agency's stream.
pub struct Overtime {
  receiver: Receiver,
}

impl Overtime {
  pub fn new(receiver: Receiver) -> Self {
    Overtime { receiver }
  }
}

impl Hearer for Overtime {
  fn kinds(&self) -> &'static [&'static str] {
    &[ASKED, CLOSED]
  }

  fn hear<'a>(&'a self, event: &'a Event) -> BoxFuture<'a, ()> {
    Box::pin(async move {
      let done = if event.kind == ASKED {
        match serde_json::from_value::<Asked>(event.data.clone()) {
          Ok(asked) => ask(&self.receiver, &asked).await.map(|_| kick(&self.receiver)),
          Err(why) => Err(Fault::new(format!("it does not read: {why}"))),
        }
      } else {
        match serde_json::from_value::<Closed>(event.data.clone()) {
          Ok(closed) => withdraw(&self.receiver, &closed).await.map(drop),
          Err(why) => Err(Fault::new(format!("it does not read: {why}"))),
        }
      };

      if let Err(why) = done {
        eprintln!("bmo-receiver: the {} was not heard: {}", event.kind, why.detail);
      }
    })
  }
}

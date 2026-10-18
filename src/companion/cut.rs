//! The receiver's half of a companion's reset (the agency's `docs/companions.md`, 「What the reset cuts off」;
//! docs/wake.md, 「A companion's round」): each move of the companion's reset point heard on the agency's event
//! stream, and everything waiting for its turns let go unread under the hold its answers are kept under.

use futures_util::future::BoxFuture;
use receiver_ports::agency::Event;
use serde_json::Value;

use crate::chat::runner::Runner;
use crate::hearing::Hearer;
use crate::receiver::Receiver;
use crate::{now_ms, run};

/// The event a reset's move of the point is published as.
pub const CUT_OFF: &str = "companion-cut-off";

/// The cut-off as the service hears it on the agency's stream: each one lets go what waited for the companion's
/// turns in the conversations the runner holds.
pub struct CutOffs {
  receiver: Receiver,
  runner: Runner,
}

impl CutOffs {
  pub fn new(receiver: Receiver, runner: Runner) -> Self {
    CutOffs { receiver, runner }
  }
}

impl Hearer for CutOffs {
  fn kinds(&self) -> &'static [&'static str] {
    &[CUT_OFF]
  }

  fn hear<'a>(&'a self, event: &'a Event) -> BoxFuture<'a, ()> {
    Box::pin(cut_off(&self.receiver, &self.runner, &event.data))
  }
}

/// What one cut-off lets go, once no answer of the companion's is being kept: the words its rooms' queues are
/// gathering and those its conversations hold or wait on, unread, and its quiet instructions, whose rows are
/// settled as delivered at the point. A companion whose conversations do not stand here has nothing waiting.
pub async fn cut_off(receiver: &Receiver, runner: &Runner, event: &Value) {
  let Some(agent) = event.get("agent").and_then(Value::as_str) else {
    return;
  };
  let Some(conversations) = runner.standing(agent) else {
    return;
  };
  let _cutting = conversations.cutting().await;
  let cut = conversations.cut_off();
  let at = event.get("point").and_then(Value::as_i64).unwrap_or_else(now_ms) / 1000;

  if cut.instructions.is_empty() {
    return;
  }

  let numbered = (2..cut.instructions.len() + 2)
    .map(|place| format!("${place}"))
    .collect::<Vec<_>>()
    .join(", ");
  let mut values: Vec<sea_orm::Value> = vec![at.into()];

  values.extend(cut.instructions.iter().map(|id| (*id).into()));

  if let Err(why) = run(
    receiver.db(),
    &format!(
      r#"UPDATE "instructions" SET "delivered_at" = $1 WHERE "id" IN ({numbered}) AND "delivered_at" IS NULL"#
    ),
    values,
  )
  .await
  {
    eprintln!("companion {agent}: the instructions its reset let go were not settled: {}", why.detail);
  }
}

//! The cut-off of an agent's reset (docs/wake.md, 「An agent's reset」): each move of an agent's reset point heard on
//! the agency's event stream, and everything waiting for its turns let go unread.

use futures_util::future::BoxFuture;
use receiver_ports::agency::Event;
use serde_json::Value;

use crate::chat::runner::Runner;
use crate::hearing::Hearer;
use crate::receiver::Receiver;
use crate::run;

/// The kind a reset's move of the point is published under, the agency's own name for it.
pub const CUT_OFF: &str = "companion-reset.created";

/// The cut-off as the service hears it on the agency's stream: each one lets go what waited for the agent's
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

/// What one cut-off lets go: the words the agent's rooms' queues are gathering and those its conversations hold or
/// wait on, unread, and its quiet instructions, whose rows are settled as delivered at the point. An agent whose
/// conversations do not stand here has nothing waiting.
pub async fn cut_off(receiver: &Receiver, runner: &Runner, event: &Value) {
  let Some(agent) = event.get("agent").and_then(Value::as_str) else {
    return;
  };
  // A reset's own part shares the kind and names its ranges, and only a move of the point cuts anything off.
  let Some(at) = event.get("point").and_then(Value::as_i64) else {
    return;
  };
  let Some(conversations) = runner.standing(agent) else {
    return;
  };
  let cut = conversations.cut_off();

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
    eprintln!("agent {agent}: the instructions its reset let go were not settled: {}", why.detail);
  }
}

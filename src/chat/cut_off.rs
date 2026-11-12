//! The cut-off of an agent's reset (docs/wake.md, 「An agent's reset」): each move of an agent's reset point
//! heard on the agency's event stream, kept as the agent's point, and everything waiting for its turns let
//! go unread; and the refusal of a send asked in a turn the point overtook, judged by the agency's point
//! read afresh beside the kept one (docs/chat-mcp.md, 「How a call runs」).

use futures_util::future::BoxFuture;
use receiver_ports::agency::Event;
use room_calls::sends::Asker;
use room_calls::turn::{overtaken_refusal, overtakes, reset_point_key};
use sea_orm::ConnectionTrait;
use serde_json::Value;

use crate::chat::runner::Runner;
use crate::hearing::Hearer;
use crate::receiver::Receiver;
use crate::run;
use crate::store::{Fault, settings};

/// The agent's reset point as the receiver keeps it, in epoch milliseconds; none where no cut-off was
/// heard.
pub async fn point(db: &impl ConnectionTrait, agent: &str) -> Result<Option<i64>, Fault> {
  Ok(
    settings::get(db, &reset_point_key(agent))
      .await?
      .and_then(|kept| kept.trim().parse().ok()),
  )
}

/// The agent's reset point kept at `at`, where no later one is kept already.
pub async fn keep_point(db: &impl ConnectionTrait, agent: &str, at: i64) -> Result<(), Fault> {
  if point(db, agent).await?.is_some_and(|kept| kept >= at) {
    return Ok(());
  }

  settings::set(db, &reset_point_key(agent), &at.to_string()).await
}

/// The agent's reset point as a turn is judged by it: the agency's, read afresh at its reset-point read and
/// kept, and the kept one where that is later or the agency does not answer; none where neither names
/// one.
pub async fn reset_point(receiver: &Receiver, agent: &str) -> Option<i64> {
  let kept = match point(receiver.db(), agent).await {
    Ok(point) => point,
    Err(why) => {
      eprintln!("agent {agent}: its reset point was not read: {}", why.detail);
      None
    }
  };
  let read = match receiver.units().agency.reset_point(agent).await {
    Ok(point) => point.and_then(|point| point.reset_point),
    Err(why) => {
      eprintln!("agent {agent}: the agency's reset point was not read, so the kept one stands: {why}");
      None
    }
  };

  if let Some(at) = read
    && let Err(why) = keep_point(receiver.db(), agent, at).await
  {
    eprintln!("agent {agent}: its reset point was not kept: {}", why.detail);
  }

  kept.max(read)
}

/// The refusal of a send whose asking turn its agent's reset overtook; none for an asker naming no agent or
/// no moment, or an agent no reset is known of.
pub async fn overtaken(receiver: &Receiver, asker: &Asker) -> Option<String> {
  let agent = asker.agent_name.as_deref().map(str::trim).filter(|agent| !agent.is_empty())?;
  let began = asker.began?;
  let point = reset_point(receiver, agent).await;

  overtakes(point, began).then(|| overtaken_refusal(agent))
}

/// The kind a reset's move of the point is published under.
pub const CUT_OFF: &str = "supervisor-reset.created";

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

/// What one cut-off keeps and lets go: the agent's reset point, kept; the words the agent's rooms' queues
/// are gathering and those its conversations hold or wait on, let go unread; and its quiet instructions,
/// whose rows are settled as delivered at the point. An agent whose conversations do not stand here has
/// nothing waiting.
pub async fn cut_off(receiver: &Receiver, runner: &Runner, event: &Value) {
  let Some(agent) = event.get("agent").and_then(Value::as_str) else {
    return;
  };
  // A reset's own part shares the kind and names its ranges, and only a move of the point cuts anything off.
  let Some(at) = event.get("point").and_then(Value::as_i64) else {
    return;
  };

  if let Err(why) = keep_point(receiver.db(), agent, at).await {
    eprintln!("agent {agent}: its reset point was not kept: {}", why.detail);
  }

  let Some(conversations) = runner.standing(agent) else {
    return;
  };
  let cut = conversations.cut_off();

  if cut.instructions.is_empty() {
    return;
  }

  let numbered = (3..cut.instructions.len() + 3)
    .map(|place| format!("${place}"))
    .collect::<Vec<_>>()
    .join(", ");
  let mut values: Vec<sea_orm::Value> = vec![at.into(), super::instructions::why::RESET.into()];

  values.extend(cut.instructions.iter().map(|id| (*id).into()));

  if let Err(why) = run(
    receiver.db(),
    &format!(
      r#"UPDATE "instructions" SET "delivered_at" = $1, "let_go" = $2 WHERE "id" IN ({numbered}) AND "delivered_at" IS NULL"#
    ),
    values,
  )
  .await
  {
    eprintln!("agent {agent}: the instructions its reset let go were not settled: {}", why.detail);
  }
}

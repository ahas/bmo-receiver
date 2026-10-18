//! The receiver's half of a companion's reset (the agency's `docs/companions.md`, 「What the reset cuts off」;
//! docs/wake.md, 「A companion's round」): each move of the companion's reset point heard on the agency's event
//! stream, and everything waiting for its turns let go unread under the hold its answers are kept under.

use std::time::Duration;

use futures_util::StreamExt;
use serde_json::Value;

use crate::chat::runner::Runner;
use crate::receiver::Receiver;
use crate::{now_ms, run};

/// The event a reset's move of the point is published as.
pub const CUT_OFF: &str = "companion-cut-off";

// How long a stream that dropped or could not be opened waits before it is opened again.
const REOPEN: Duration = Duration::from_secs(5);

/// The cut-off heard for as long as the service runs: the agency waits on no reader, so a stream that drops is
/// opened again, and what it published meanwhile is the round's reading of the point to catch.
pub fn keep_hearing(receiver: Receiver, runner: Runner) {
  tokio::spawn(async move {
    loop {
      match receiver.units().agency.events(&[CUT_OFF]).await {
        Ok(mut stream) => {
          while let Some(read) = stream.next().await {
            match read {
              Ok(event) if event.kind == CUT_OFF => cut_off(&receiver, &runner, &event.data).await,
              Ok(_) => {}
              Err(why) => {
                eprintln!("bmo-receiver: the companions' cut-off stream dropped: {why}");
                break;
              }
            }
          }
        }
        Err(why) => eprintln!("bmo-receiver: the companions' cut-off stream did not open: {why}"),
      }

      tokio::time::sleep(REOPEN).await;
    }
  });
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

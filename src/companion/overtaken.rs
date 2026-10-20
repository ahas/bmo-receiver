//! The point a companion's reset cuts it off at, as the agency keeps it (the agency's `docs/companions.md`, 「What the
//! reset cuts off」; docs/wake.md, 「A companion's round」): a turn begun at or before it is overtaken, and what was
//! said, or what hands back a run begun, at or before it is no longer the companion's to answer.

use room_calls::turn::overtakes;

use crate::all;
use crate::receiver::{Inbound, Receiver};

/// The agent's reset point, where it was ever reset and the agency answers.
pub async fn point(receiver: &Receiver, agent: &str) -> Option<i64> {
  receiver.units().agency.reset_point(agent).await.ok().flatten()
}

/// Whether the agent's reset overtook a turn begun at `began`.
pub async fn overtaken(receiver: &Receiver, agent: &str, began: i64) -> bool {
  overtakes(point(receiver, agent).await, began)
}

/// The words said after the point. Both moments are epoch milliseconds, so a message said in the point's own
/// millisecond goes with those before it.
pub fn said_after(words: Vec<Inbound>, point: Option<i64>) -> Vec<Inbound> {
  match point {
    Some(point) => words.into_iter().filter(|word| word.sent_at > point).collect(),
    None => words,
  }
}

/// Of the quiet instruction rows given, those handing back the result of a run begun at or before the point. A
/// run's moment is a whole second, as the agency keeps it; a run the agency cannot answer for keeps its
/// instruction.
pub async fn results_before(receiver: &Receiver, ids: &[i64], point: Option<i64>) -> Vec<i64> {
  let Some(point) = point.filter(|_| !ids.is_empty()) else {
    return vec![];
  };
  let listed = (1..=ids.len()).map(|at| format!("${at}")).collect::<Vec<_>>().join(", ");
  let rows = all(
    receiver.db(),
    &format!(
      r#"SELECT "id", "run_id" FROM "instructions" WHERE "id" IN ({listed}) AND "run_id" IS NOT NULL"#
    ),
    ids.iter().map(|id| (*id).into()).collect(),
  )
  .await
  .unwrap_or_default();
  let mut late = vec![];

  for row in rows {
    let (Ok(id), Ok(run)) = (row.try_get::<i64>("", "id"), row.try_get::<i64>("", "run_id")) else {
      continue;
    };

    if let Ok(Some(run)) = receiver.units().agency.run(run).await
      && run.started_at * 1000 <= point
    {
      late.push(id);
    }
  }

  late
}

//! Whether work stands under way that a restart of the receiver would cut (docs/boundary-api.md, 「The service and
//! its commands」, `quiet`): the agency's runs still open and the double's slots the store still keeps, each that
//! counts named.

use receiver_ports::agency::{Agency, OpenRun, RunningJob};
use sea_orm::DatabaseConnection;

use crate::chat::double_slots;

/// Whether a restart of the receiver would cut what follows the run: a turn the receiver carries, every one of
/// which names a round. What a run is decides, never a word of its name; a room it answers is no sign, since a
/// job's run names its room too.
pub fn counts(run: &OpenRun) -> bool {
  run.round.is_some()
}

/// The open runs that count as under way, each named as a line, however long it has run.
pub fn under_way(open: &[OpenRun]) -> Vec<String> {
  open
    .iter()
    .filter(|run| counts(run))
    .map(|run| format!("run {} {} under way since {}", run.id, run.name, run.started_at))
    .collect()
}

/// Every job starting or running, each named as a line: its role, and its repository with the checkout of
/// it beside it, or the checkout alone where no repository holds it any longer.
pub fn jobs_under_way(jobs: &[RunningJob]) -> Vec<String> {
  jobs
    .iter()
    .map(|job| {
      let checkout = job.worker_name.as_deref().unwrap_or("a checkout");
      let place = match job.repository_name.as_deref() {
        Some(repository) => format!("{repository} (checkout {checkout})"),
        None => format!("checkout {checkout}"),
      };

      format!(
        "job {} as {} in {place} on item {} under way since {}",
        job.id,
        job.role_name.as_deref().unwrap_or("a role"),
        job.item_id.map_or_else(|| "none".to_owned(), |item| item.to_string()),
        job.started_at.unwrap_or_default()
      )
    })
    .collect()
}

/// What stands under way now: the runs that count as the agency answers them, every double's slot the store still
/// keeps, since the agency opens no run for a double's turn, and with `jobs` every job starting or running; the
/// failure of whichever did not answer.
pub async fn asked(
  agency: &dyn Agency,
  db: &DatabaseConnection,
  jobs: bool,
) -> Result<Vec<String>, String> {
  let mut lines = agency.open_runs().await.map(|open| under_way(&open)).map_err(|why| why.to_string())?;

  lines.extend(double_slots::under_way(db).await.map_err(|why| why.detail)?);

  if jobs {
    lines.extend(
      agency
        .running_jobs()
        .await
        .map(|running| jobs_under_way(&running))
        .map_err(|why| why.to_string())?,
    );
  }

  Ok(lines)
}

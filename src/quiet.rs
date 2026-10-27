//! Whether work stands under way that a restart of the receiver would cut (docs/boundary-api.md, 「The service and
//! its commands」, `quiet`): the agency's runs still open, each that counts named.

use receiver_policy::quiet::QUIET;
use receiver_ports::agency::{Agency, OpenRun, RunningJob};

/// The open runs that count as under way at `now_ms`, each named as a line: one whose name carries a long-work
/// word however long it has run, a chat turn's started within the policy's span; no other.
pub fn under_way(open: &[OpenRun], now_ms: i64) -> Vec<String> {
  let span = QUIET.recent_minutes * 60_000;

  open
    .iter()
    .filter(|run| {
      let carries = |words: &[String]| words.iter().any(|word| run.name.contains(word.as_str()));

      carries(&QUIET.long_words) || (carries(&QUIET.turn_words) && now_ms - run.started_at < span)
    })
    .map(|run| format!("run {} {} under way since {}", run.id, run.name, run.started_at))
    .collect()
}

/// What stands under way as the agency answers now; the agency's failure where it did not answer.
/// Every job starting or running, each named as a line.
pub fn jobs_under_way(jobs: &[RunningJob]) -> Vec<String> {
  jobs
    .iter()
    .map(|job| {
      format!(
        "job {} of {} on item {} under way since {}",
        job.id,
        job.worker_name.as_deref().unwrap_or("a worker"),
        job.item_id.map_or_else(|| "none".to_owned(), |item| item.to_string()),
        job.started_at.unwrap_or_default()
      )
    })
    .collect()
}

/// What stands under way as the agency answers now: the runs that count, and with `jobs` every job starting or
/// running; the agency's failure where it did not answer either.
pub async fn asked(agency: &dyn Agency, now_ms: i64, jobs: bool) -> Result<Vec<String>, String> {
  let mut lines = agency
    .open_runs()
    .await
    .map(|open| under_way(&open, now_ms))
    .map_err(|why| why.to_string())?;

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

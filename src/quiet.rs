//! Whether work stands under way that a restart of the receiver would cut (docs/boundary-api.md, 「The service and
//! its commands」, `quiet`): the agency's runs still open, each that counts named.

use receiver_policy::quiet::QUIET;
use receiver_ports::agency::{Agency, OpenRun, RunningJob};

/// The open runs that count as under way, each named as a line: one whose name carries a long-work word or a
/// chat turn's word, however long it has run; no other.
pub fn under_way(open: &[OpenRun]) -> Vec<String> {
  open
    .iter()
    .filter(|run| {
      let carries = |words: &[String]| words.iter().any(|word| run.name.contains(word.as_str()));

      carries(&QUIET.long_words) || carries(&QUIET.turn_words)
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
        "job {} as {} in {} on item {} under way since {}",
        job.id,
        job.role_name.as_deref().unwrap_or("a role"),
        job.worker_name.as_deref().unwrap_or("a checkout"),
        job.item_id.map_or_else(|| "none".to_owned(), |item| item.to_string()),
        job.started_at.unwrap_or_default()
      )
    })
    .collect()
}

/// What stands under way as the agency answers now: the runs that count, and with `jobs` every job starting or
/// running; the agency's failure where it did not answer either.
pub async fn asked(agency: &dyn Agency, jobs: bool) -> Result<Vec<String>, String> {
  let mut lines = agency.open_runs().await.map(|open| under_way(&open)).map_err(|why| why.to_string())?;

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

//! Whether work stands under way that a restart of the receiver would cut (docs/boundary-api.md, 「The service and
//! its commands」, `quiet`): the agency's runs still open, each that counts named.

use receiver_policy::quiet::QUIET;
use receiver_ports::agency::{Agency, OpenRun};

/// The open runs that count as under way at `now_ms`, each named as a line: one whose name carries a long-work
/// word however long it has run, any other started within the policy's span.
pub fn under_way(open: &[OpenRun], now_ms: i64) -> Vec<String> {
  let span = QUIET.recent_minutes * 60_000;

  open
    .iter()
    .filter(|run| {
      QUIET.long_words.iter().any(|word| run.name.contains(word.as_str())) || now_ms - run.started_at < span
    })
    .map(|run| format!("run {} {} under way since {}", run.id, run.name, run.started_at))
    .collect()
}

/// What stands under way as the agency answers now; the agency's failure where it did not answer.
pub async fn asked(agency: &dyn Agency, now_ms: i64) -> Result<Vec<String>, String> {
  agency
    .open_runs()
    .await
    .map(|open| under_way(&open, now_ms))
    .map_err(|why| why.to_string())
}

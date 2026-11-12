//! A round of a room's session: everything run to produce one answer (the web unit's usage chips). The round
//! carries a name of its own, and whatever is recorded while it goes — a gate's road, a judgement, the
//! message the answer goes out as — carries that name, so what one answer cost is read by the name. The name
//! travels with the round's own work rather than as an argument, since a gate is asked far below the turn
//! that set it going; work the round starts and does not wait on is started outside it.

use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::now_ms;

tokio::task_local! {
  static ROUND: String;
}

static ISSUED: AtomicU64 = AtomicU64::new(0);

/// A fresh name for a round of one room: the room and the moment make it readable in the record, and the
/// count keeps two rounds of the same millisecond apart.
pub fn new_name(chat_id: i64) -> String {
  let issued = ISSUED.fetch_add(1, Ordering::Relaxed) + 1;

  format!("round:{chat_id}:{}:{issued}", now_ms())
}

/// Work done inside a round.
pub async fn within<F: Future>(round: String, work: F) -> F::Output {
  ROUND.scope(round, work).await
}

/// The round the work under way belongs to, where it belongs to one.
pub fn current() -> Option<String> {
  ROUND.try_with(Clone::clone).ok()
}

/// The round a turn sent for the room names: the work's own, or one of the turn's own outside any round, so every
/// run of a turn the receiver follows names a round (docs/boundary-api.md, `quiet`).
pub fn of_turn(chat_id: i64) -> String {
  current().unwrap_or_else(|| new_name(chat_id))
}

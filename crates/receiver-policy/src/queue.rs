//! What the receiver holds of a job's question as the owner answers it (docs/rooms.md).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueueLimits {
  /// How long the owner's stop of a question's clock holds it, in seconds.
  pub question_stop_seconds: i64,
}

crate::part!(pub QUEUE: QueueLimits = "queue.toml");

//! Every line the system says in a chat on its own account (docs/voiced-notices.md): the facts handed to the
//! room's own voice as a quiet instruction row, in the one frame that asks for a short line said in that room's
//! manner. The frame is shared with the receiver's own acknowledgements; each producer owns the facts it writes.

use super::instructions::{Queued, queue};
use crate::now_ms;
use crate::receiver::notices::frame;
use crate::receiver::wake::agents::voice_of;
use crate::receiver::{NotReceived, Receiver};

/// What a notice hands the room's voice: the facts as the producer recorded them, what the line is for, what
/// follows it where it does, whether the session may leave it unsaid, the kind it stands under, which a newer
/// notice of the same kind replaces while it waits, and, where it waits for the round after rather than opening
/// one, how long it rides.
#[derive(Debug, Clone)]
pub struct Voiced {
  pub fact: String,
  pub intent: String,
  pub follow_up: Option<String>,
  pub optional: bool,
  pub kind: Option<String>,
  pub events: Vec<String>,
  pub rides_ms: Option<i64>,
}

/// The frame a notice is handed the session in.
pub fn instruction(notice: &Voiced) -> String {
  frame(&notice.fact, &notice.intent, notice.follow_up.as_deref(), notice.optional)
}

/// The notice queued for the session of the room's voice, in place of one of the same kind not yet handed over;
/// its row.
pub async fn queue_notice(receiver: &Receiver, chat_id: i64, notice: &Voiced) -> Result<i64, NotReceived> {
  let now = now_ms();
  let voice = voice_of(receiver, chat_id).await?;
  let text = instruction(notice);
  let queued = Queued {
    kind: notice.kind.as_deref(),
    events: notice.events.clone(),
    rides_until_ms: notice.rides_ms.map(|ride| now + ride),
    sort: Some("notice"),
    summary: Some(notice.fact.trim()),
    ..Queued::default()
  };

  Ok(queue(receiver.db(), voice.as_ref(), chat_id, &text, &queued).await?)
}

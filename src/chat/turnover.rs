//! What a turn leaves of the session its room's turns go on, and what the round is written down as (docs/unified-
//! bmo.md, 「The one session」): the read position the conversation holds, the rooms the one shared session has
//! answered, the launch the one shared session last stood on — which the touch of a quiet room reads — and the
//! round's verdict, kept beside the room by the gate. The road a turn walked and what it spent are published by
//! the agency as `road-walked` and `usage-spent`, and the receiver writes neither.

use receiver_policy::providers::Handover;
use receiver_ports::gate::Judgement;
use serde_json::json;

use super::conversation::{BoxFuture, Held, Noting, Reply, unified_session};
use super::fold::{compact, hand_off, handover_of};
use super::sentinels::{is_skip, is_wait};
use crate::form::without_control_markers;
use crate::receiver::Receiver;
use crate::rounds;
use crate::run as execute;
use crate::store::{Fault, settings};

// How much of an answer stands in the verdict written beside the round.
const VERDICT_CHARS: usize = 120;

/// The setting the last launch of an agent's one shared session is kept under, which the touch reads.
pub fn last_launch_key(agent: &str) -> String {
  format!("{agent}_last_chat_launch")
}

/// The answer as one line of its verdict, cut to `limit` characters.
pub fn verdict_line(text: &str, limit: usize) -> String {
  let line = text.trim().split('\n').find(|line| !line.trim().is_empty()).unwrap_or_default();
  let units: Vec<u16> = line.encode_utf16().take(limit).collect();

  String::from_utf16_lossy(&units)
}

/// The verdict a round's answer is written down under: an answer's words as the room would read them, with no
/// control marker standing in them, and one that says nothing once they are out a skip.
pub fn verdict_of(text: &str) -> String {
  let said = without_control_markers(text);

  if is_wait(text) {
    "WAIT".to_owned()
  } else if is_skip(text) || said.trim().is_empty() {
    "SKIP".to_owned()
  } else {
    format!("REPLY: {}", verdict_line(&said, VERDICT_CHARS))
  }
}

/// A decision written into a room's log by the gate.
pub async fn judge(
  receiver: &Receiver,
  chat_id: i64,
  message: Option<i64>,
  stage: &str,
  verdict: &str,
  detail: Option<&str>,
  ms: Option<i64>,
) {
  let asked = Judgement {
    room_id: chat_id,
    message_id: message,
    gate_name: stage.to_owned(),
    verdict: verdict.to_owned(),
    detail: detail.map(str::to_owned),
    ms,
    round: rounds::current(),
  };

  match receiver.units().gate.judge(&asked).await {
    // The room's screens hear the verdict as it lands, so its chip stands without a read of the room.
    Ok(_) => receiver
      .news()
      .publish(&crate::events::judgement(chat_id, message, stage, verdict, detail)),
    Err(why) => eprintln!("chat {chat_id}: {stage} could not be written down: {why}"),
  }
}

/// What a turn leaves of the session its room's turns go on: the launch the one shared session last stood on, and
/// the room among the ones it has answered.
pub async fn note_session(
  receiver: &Receiver,
  agent: &str,
  chat_id: i64,
  privileged: bool,
  session: Option<&str>,
  tools_allowed: bool,
) -> Result<(), Fault> {
  if session.is_none() || !unified_session(receiver, agent).await {
    return Ok(());
  }

  let db = receiver.db();
  let launch = json!({
    "chatId": chat_id,
    "privileged": privileged,
    "toolsAllowed": tools_allowed,
  });

  settings::set(db, &last_launch_key(agent), &launch.to_string()).await?;

  execute(
    db,
    r#"INSERT INTO "bmo_rooms" VALUES ($1) ON CONFLICT DO NOTHING"#,
    vec![chat_id.into()],
  )
  .await
  .map(drop)
}

/// The notes and the fold, as the conversations hold them.
#[derive(Clone)]
pub struct Noter {
  pub receiver: Receiver,
}

impl Noter {
  pub fn new(receiver: Receiver) -> Self {
    Noter { receiver }
  }
}

impl Noting for Noter {
  fn note<'a>(
    &'a self,
    _receiver: &'a Receiver,
    agent: &'a str,
    held: &'a Held,
    reply: &'a Reply,
    ms: i64,
  ) -> BoxFuture<'a, ()> {
    Box::pin(async move {
      let receiver = &self.receiver;
      let chat_id = held.chat_id;

      if let Err(why) =
        note_session(receiver, agent, chat_id, held.all_from_owner, reply.session.as_deref(), true).await
      {
        eprintln!("chat {chat_id}: the session's launch: {}", why.detail);
      }

      judge(
        receiver,
        held.chat_id,
        held.last_consumed_id,
        "chat",
        &verdict_of(&reply.text),
        Some(&reply.text),
        Some(ms),
      )
      .await;
    })
  }

  fn fold<'a>(
    &'a self,
    receiver: &'a Receiver,
    agent: &'a str,
    chat_id: i64,
    held: &'a Held,
  ) -> BoxFuture<'a, ()> {
    Box::pin(async move {
      // A full session its provider hands over by its own note is replaced at its next turn, not folded.
      if unified_session(receiver, agent).await && handover_of(receiver, agent).await == Handover::Handoff {
        hand_off(receiver, agent).await;

        return;
      }

      compact(receiver, agent, chat_id, held.session.as_deref()).await;
    })
  }
}

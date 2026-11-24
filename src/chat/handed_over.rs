//! The room's line that an agent's session was handed over (docs/unified-bmo.md, 「The room's line」): one fixed
//! line, `session-handed-over`, kept in one room's record alone for each handover, written where the handover is
//! made so every road shows it, and sent to no chat service. Nothing of the room's record goes with the session.

use receiver_codes::{FixedLine, HandoverCause, SessionReason};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};

use crate::receiver::Receiver;
use crate::store::{Fault, settings};

/// A handover a person or the agent asked through the route (docs/api.md, 「Handovers」): the asker's words, the
/// room its line goes to where the asking named one, and the account key of the person who asked where it was
/// named.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asked {
  pub reason: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub room: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub by: Option<i64>,
}

/// Where the handover asked for an agent is kept until the replacement that answers it is made.
pub fn asked_key(agent: &str) -> String {
  format!("{agent}_handover_asked")
}

/// The handover asked for an agent kept, in place of any asked before it: one replacement answers every asking
/// made before it, under the last asker's words.
pub async fn keep_asked(db: &DatabaseConnection, agent: &str, asked: &Asked) -> Result<(), Fault> {
  let held = serde_json::to_string(asked).map_err(|why| Fault::new(why.to_string()))?;

  settings::set(db, &asked_key(agent), &held).await
}

/// The handover asked for an agent taken, by the replacement that answers it; none where none was asked.
pub async fn take_asked(db: &DatabaseConnection, agent: &str) -> Option<Asked> {
  match settings::take(db, &asked_key(agent)).await {
    Ok(held) => held.and_then(|held| serde_json::from_str(&held).ok()),
    Err(why) => {
      eprintln!("{agent}: the handover asked was not read: {}", why.detail);

      None
    }
  }
}

/// The line kept in the room's record alone; a line that could not be kept is the journal's.
pub async fn tell(receiver: &Receiver, chat_id: i64, line: &FixedLine) {
  match receiver.speech().keep_line(chat_id, line).await {
    Ok(_) => {}
    Err(why) => eprintln!("chat {chat_id}: the session-handed-over line was not kept: {why}"),
  }
}

/// The line for a replacement of the agent's one session the receiver made for `reason`: into the room the asking
/// named, else the room of the round behind which it was made (`behind`), else the owner's private room. A handover
/// asked through the route is taken here, so its line names the asker's words and who asked; a replacement made
/// for another reason while one was asked answers that asking too.
pub async fn replaced(receiver: &Receiver, agent: &str, reason: SessionReason, behind: Option<i64>) {
  let asked = take_asked(receiver.db(), agent).await;
  let room = asked
    .as_ref()
    .and_then(|asked| asked.room)
    .or(behind)
    .unwrap_or_else(|| receiver.owner_room());
  let line = match asked {
    Some(asked) => FixedLine::SessionHandedOver {
      agent: agent.to_owned(),
      cause: HandoverCause::Asked,
      reason: Some(asked.reason).filter(|reason| !reason.trim().is_empty()),
      by: asked.by,
    },
    None => FixedLine::SessionHandedOver {
      agent: agent.to_owned(),
      cause: HandoverCause::of(reason),
      reason: None,
      by: None,
    },
  };

  tell(receiver, room, &line).await;
}

/// The asking let go where the replacement it asked for had nothing to hand over: no line is kept.
pub async fn let_go(receiver: &Receiver, agent: &str) {
  let _ = take_asked(receiver.db(), agent).await;
}

/// The line for a handover the agency asked about (docs/api.md, 「Handoffs」), for the cause and the asker it
/// named: into `room`, the room the asking named or the room of the round behind which it was asked, else the
/// owner's private room.
pub async fn handed_off(
  receiver: &Receiver,
  agent: &str,
  room: Option<i64>,
  cause: HandoverCause,
  by: Option<i64>,
) {
  let room = room.unwrap_or_else(|| receiver.owner_room());
  let line = FixedLine::SessionHandedOver {
    agent: agent.to_owned(),
    cause,
    reason: None,
    by,
  };

  tell(receiver, room, &line).await;
}

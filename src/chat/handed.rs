//! Words handed to the room's supervisor (docs/messages.md, 「Words handed to the room's supervisor」): a send is
//! carried only in its agent's own round and only where it holds no words, and every other send — words for
//! another room, a command at a shell, a job, a worker, a unit's process, a script — is queued as an order for the
//! room's supervisor carrying what was asked, which that supervisor's round there writes as its message, checked
//! by its instruction stages, or lets go.

use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::fill_marks;
use room_calls::sends::{Act, Asker, TalkInput};

use crate::chat::instructions::{Queued, queue};
use crate::receiver::wake::agents;
use crate::receiver::{NotReceived, Receiver};
use crate::secrets;

/// What a handing came to: the agent it went to, the room it is for and the order's number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Handing {
  pub agent: String,
  pub chat_id: i64,
  pub id: i64,
}

impl Handing {
  /// The line the asker is answered with.
  pub fn line(&self) -> String {
    fill_marks(
      &CHAT_ROUND.handed.answered,
      &[
        ("agent", &self.agent),
        ("room", &self.chat_id.to_string()),
        ("id", &self.id.to_string()),
      ],
    )
  }
}

/// The room the asker's turn works in, read off the chat that set the send off (`chat:<room>`).
pub fn turn_room(asker: &Asker) -> Option<i64> {
  asker.by.as_deref()?.trim().strip_prefix("chat:")?.trim().parse().ok()
}

/// Whether the send is the asking agent's own, in its own round: the asker names an agent, no job asks it, and a
/// round of that agent is working in the room that set the send off.
pub fn own_round(receiver: &Receiver, asker: &Asker) -> bool {
  let (Some(agent), Some(room)) = (asker.agent_name.as_deref(), turn_room(asker)) else {
    return false;
  };

  !asker.job && receiver.runner().is_some_and(|runner| runner.in_round(agent.trim(), room))
}

/// Who handed the words, as the order names them.
fn who(asker: &Asker) -> String {
  let handed = &CHAT_ROUND.handed;
  let who = match (asker.job, asker.agent_name.as_deref()) {
    (true, _) => handed.job.clone(),
    (false, Some(agent)) => fill_marks(&handed.agent, &[("agent", agent)]),
    (false, None) => handed.nobody.clone(),
  };

  match asker.from.as_deref().map(str::trim).filter(|from| !from.is_empty()) {
    Some(from) => fill_marks(&handed.through, &[("who", &who), ("from", from)]),
    None => who,
  }
}

// A spoken talk's words: the voice, the written words and the spoken words where given.
fn spoken(input: &TalkInput) -> Vec<String> {
  let handed = &CHAT_ROUND.handed;
  let mut lines = vec![fill_marks(&handed.spoken, &[("voice", &input.voice), ("text", &input.text)])];

  if let Some(spoken) = input.spoken.as_deref() {
    lines.push(fill_marks(&handed.spoken_words, &[("spoken", spoken)]));
  }

  lines
}

/// What was asked, as the order hands it over: a line's words, a spoken talk's words, or any other act by its
/// tool's name and its arguments; with the message it answers, an answer wanted back and the road it named.
pub fn asked(asker: &Asker, act: &Act) -> String {
  let handed = &CHAT_ROUND.handed;
  let (mut lines, reply_to) = match act {
    Act::Say(input) => {
      let mut lines = vec![fill_marks(&handed.said, &[("text", &input.text)])];

      if input.reply {
        lines.push(handed.answer_wanted.clone());
      }

      (lines, input.reply_to)
    }
    Act::Talk(input) => (spoken(input), input.reply_to),
    other => {
      let input = serde_json::to_value(other)
        .ok()
        .and_then(|asked| asked.get("input").cloned())
        .map(|input| input.to_string())
        .unwrap_or_default();

      (vec![fill_marks(&handed.act, &[("tool", other.tool()), ("input", &input)])], None)
    }
  };

  if let Some(message) = reply_to {
    lines.push(fill_marks(&handed.reply_to, &[("message", &message.to_string())]));
  }

  if let Some(via) = asker.via.as_deref() {
    lines.push(fill_marks(&handed.via, &[("via", via)]));
  }

  lines.join("\n")
}

/// The line the asker is told where the answer it wanted back will not be brought: it has asked several rooms in a
/// short stretch, while the words themselves were still handed.
pub fn relay_dropped() -> String {
  CHAT_ROUND.handed.relay_dropped.clone()
}

/// The supervisor the words go to: the asker's own agent where it answers the room, and the room's voice
/// otherwise.
pub async fn supervisor(receiver: &Receiver, asker: &Asker, chat_id: i64) -> Result<String, NotReceived> {
  if let Some(agent) = asker.agent_name.as_deref().map(str::trim).filter(|agent| !agent.is_empty())
    && agents::answers(receiver, agent, chat_id).await?
  {
    return Ok(agent.to_owned());
  }

  agents::voice_of(receiver, chat_id).await
}

/// The words handed to `agent` as an order in the room they are for, which opens a turn there and never rides;
/// secret-shaped runs are taken out before the session is handed them.
pub async fn hand(
  receiver: &Receiver,
  asker: &Asker,
  agent: &str,
  chat_id: i64,
  asked: &str,
) -> Result<Handing, NotReceived> {
  let asked = secrets::redacted(asked);
  let text = format!(
    "{}\n{}",
    CHAT_ROUND.handed.marker,
    fill_marks(
      &CHAT_ROUND.handed.instruction,
      &[("room", &chat_id.to_string()), ("who", &who(asker)), ("asked", &asked)],
    )
  );
  let summary = asked.lines().next().unwrap_or_default().to_owned();
  let queued = Queued {
    sort: Some(&CHAT_ROUND.handed.sort),
    summary: Some(&summary),
    via: asker.via.as_deref(),
    ..Queued::default()
  };
  let id = queue(receiver.db(), agent, chat_id, &text, &queued).await?;

  if let Some(pass) = receiver.pass() {
    pass.kick();
  }

  Ok(Handing {
    agent: agent.to_owned(),
    chat_id,
    id,
  })
}

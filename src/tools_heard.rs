//! The tool host's catalogue as the receiver hears it (docs/boundary-api.md, 「The tool host」; docs/wake.md,
//! 「Quiet instructions」): the host's event stream heard for as long as the service runs, resumed after the last
//! event read and opened afresh where the host holds that event no longer, and each tool placed, updated or
//! removed told to every chat agent whose standing tool session holds one of its namespaces, as a fact that rides
//! the agent's next round in any room and wakes nobody.

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use receiver_policy::fill_marks;
use receiver_policy::tool_host::TOOL_HOST;
use receiver_ports::agency::Agent;
use receiver_ports::tool_host::{StandingSession, ToolEvent, ToolHost};
use serde_json::Value;

use crate::chat::instructions::{Queued, queue};
use crate::receiver::Receiver;

// The kind of an event that takes a tool away.
const REMOVED: &str = "tool.removed";
// The status the host answers a cursor it holds no event after.
const GONE: u16 = 410;
// The kind of a chat agent, as the agency registers one.
const CHAT: &str = "chat";

/// The fact a chat agent is told of a change: what became of the tool and each entry with how it is reached and
/// its summary, then the reading rule and that nothing needs saying.
pub fn instruction(event: &ToolEvent) -> String {
  let said = &*TOOL_HOST;
  let change = &event.data;
  let entries: Vec<String> = change
    .entries
    .iter()
    .map(|entry| {
      fill_marks(
        &said.entry,
        &[("name", &entry.tool), ("reached", &entry.reached_by), ("summary", entry.summary.trim())],
      )
    })
    .collect();
  let entries = if entries.is_empty() { said.no_entries.clone() } else { entries.join(&said.joint) };
  let line = if event.kind == REMOVED {
    &said.removed
  } else if change.replaced {
    &said.updated
  } else {
    &said.placed
  };
  let fact = fill_marks(line, &[("tool", &change.tool), ("entries", &entries)]);

  format!("{fact}\n\n{}\n{}", said.read_first, said.nothing_said)
}

/// The chat agents a change concerns: each one the agency registers as a chat agent whose standing tool session
/// holds one of the change's namespaces.
pub fn concerned(agents: &[Agent], sessions: &[StandingSession], namespaces: &[String]) -> Vec<String> {
  agents
    .iter()
    .filter(|agent| agent.row.get("kind").and_then(Value::as_str) == Some(CHAT))
    .filter(|agent| {
      sessions
        .iter()
        .filter(|session| session.holder_name == agent.name)
        .any(|session| session.namespaces.iter().any(|namespace| namespaces.contains(namespace)))
    })
    .map(|agent| agent.name.clone())
    .collect()
}

/// One change told to every chat agent it concerns, each one instruction in the owner's private chat that rides
/// any room's round of its session; the agents told.
pub async fn tell(
  receiver: &Receiver,
  host: &dyn ToolHost,
  event: &ToolEvent,
) -> Result<Vec<String>, String> {
  let agents = receiver.agents().await.map_err(|why| why.to_string())?;
  let sessions = host.sessions().await.map_err(|why| why.to_string())?;
  let told = concerned(&agents.agents, &sessions, &event.data.namespaces);
  let said = &*TOOL_HOST;
  let text = instruction(event);
  let summary = fill_marks(&said.summary, &[("tool", &event.data.tool)]);

  // A kind replaces a pending row of its own in the chat whatever agent it is for, so each agent's names it.
  for agent in &told {
    let kind = format!("{}:{agent}:{}", said.kind, event.data.tool);
    let queued = Queued {
      kind: Some(&kind),
      sort: Some(&said.sort),
      summary: Some(&summary),
      rides_session: true,
      agent_only: true,
      ..Queued::default()
    };

    queue(receiver.db(), agent, receiver.notices_room(), &text, &queued)
      .await
      .map_err(|why| why.to_string())?;
  }

  Ok(told)
}

/// The host's stream heard for as long as the service runs: resumed after the last event read, opened again with
/// no cursor where the host answers that it holds that event no longer, and after the policy's wait where it
/// dropped or did not open.
pub fn keep(receiver: Receiver, host: Arc<dyn ToolHost>) {
  tokio::spawn(async move {
    hear(&receiver, host.as_ref(), Duration::from_secs(TOOL_HOST.reopen_seconds), None).await
  });
}

/// The stream heard, as `keep` hears it, until `rounds` openings have been made where it names a number; the
/// cursor it ended on.
pub async fn hear(
  receiver: &Receiver,
  host: &dyn ToolHost,
  reopen: Duration,
  rounds: Option<usize>,
) -> Option<i64> {
  let mut after: Option<i64> = None;
  let mut opened = 0;

  while rounds.is_none_or(|rounds| opened < rounds) {
    opened += 1;

    match host.events(after).await {
      Ok(mut stream) => {
        while let Some(read) = stream.next().await {
          match read {
            Ok(event) => {
              after = Some(event.id);

              match tell(receiver, host, &event).await {
                Ok(told) => {
                  println!("tool host: {} {} told to {}", event.kind, event.data.tool, told.join(", "))
                }
                Err(why) => eprintln!("tool host: {} {} was not told: {why}", event.kind, event.data.tool),
              }
            }
            Err(why) => {
              eprintln!("tool host: the event stream dropped: {why}");
              break;
            }
          }
        }
      }
      Err(why) if why.status() == Some(GONE) => {
        eprintln!("tool host: the stream holds no event after {after:?}; it is heard from now on");
        after = None;
        continue;
      }
      Err(why) => eprintln!("tool host: the event stream did not open: {why}"),
    }

    tokio::time::sleep(reopen).await;
  }

  after
}

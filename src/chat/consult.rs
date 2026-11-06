//! What an agent's turn is shown to see another agent's work (docs/consult.md, 「What an agent is shown」): in a
//! room more than one agent answers, each one's window there; with a notice of the queue among its instructions,
//! the other agents' work and the consult room after them. Every agent is read from its registration, so nothing
//! here names one.

use receiver_policy::consult::CONSULT;
use receiver_policy::fill_marks;

use crate::Fault;
use crate::receiver::{Receiver, rows, wake};
use crate::store::settings;
use receiver_ports::agency::Agent;
use receiver_ports::gate::TurnEvent;
use serde_json::Value;

/// One agent's window as a line of the block: open with its time left, or closed.
fn time_left(seconds: i64) -> String {
  let lines = &CONSULT.windows;
  let minutes = seconds / 60;

  if minutes == 0 {
    fill_marks(&lines.seconds, &[("seconds", &seconds.to_string())])
  } else {
    fill_marks(
      &lines.minutes,
      &[("minutes", &minutes.to_string()), ("seconds", &(seconds % 60).to_string())],
    )
  }
}

/// The name an agent answers to, its registration's label where it gives one.
pub async fn label_of(receiver: &Receiver, name: &str) -> String {
  let listed = receiver.agents().await.ok();

  listed
    .as_ref()
    .and_then(|listed| listed.agents.iter().find(|one| one.name == name).map(|one| one.label.clone()))
    .flatten()
    .unwrap_or(name.to_owned())
}

/// One agent's window in one room: until when it is open, or none where its row keeps none.
pub struct WindowState {
  pub agent: String,
  pub label: String,
  pub open_until: Option<i64>,
}

/// Every designated agent of a room with its window there.
pub async fn window_states(receiver: &Receiver, chat_id: i64) -> Vec<WindowState> {
  let room = receiver.roads().room_facts(chat_id).await;
  let (agents, _) = wake::agents::room_designated(receiver, chat_id, room).await.unwrap_or_default();
  let mut states: Vec<WindowState> = vec![];

  for agent in agents {
    let open_until = match agent.window_seconds {
      None => None,
      Some(_) => {
        Some(wake::agents::window_until(receiver.db(), chat_id, &agent.name).await.ok().unwrap_or(0))
      }
    };

    states.push(WindowState {
      agent: agent.name.clone(),
      label: label_of(receiver, &agent.name).await,
      open_until,
    });
  }

  states
}

/// One agent's window as a line: open with its time left, or closed.
pub fn window_line(state: &WindowState, own: &str, now_ms: i64) -> String {
  let lines = &CONSULT.windows;
  let left = state.open_until.unwrap_or(0) - now_ms;
  let mine = state.agent == own;
  let who = if mine {
    fill_marks(&lines.you, &[("label", &state.label)])
  } else {
    state.label.clone()
  };

  if left > 0 {
    let hears = if mine { "" } else { lines.hears.as_ref() };

    return fill_marks(&lines.open, &[("who", &who), ("left", &time_left(left / 1_000)), ("hears", hears)]);
  }

  let keeps = if state.open_until.is_none() { lines.keeps_none.as_ref() } else { "" };

  fill_marks(&lines.closed, &[("who", &who), ("keeps", keeps)])
}

/// The windows of a room more than one agent answers, as every turn there shows them; none for a room one agent
/// answers alone.
pub async fn room_windows_block(
  receiver: &Receiver,
  chat_id: i64,
  own: &str,
  now_ms: i64,
) -> Result<Option<String>, Fault> {
  let states = window_states(receiver, chat_id).await;

  if states.len() < 2 {
    return Ok(None);
  }

  let mut lines = vec![CONSULT.windows.heading.clone()];

  lines.extend(states.iter().map(|state| window_line(state, own, now_ms)));

  Ok(Some(lines.join("\n")))
}

/// Whether a turn's events tell of the queue: a job ended, or the queue changed.
pub fn tells_of_queue(events: &[TurnEvent]) -> bool {
  events.iter().any(|event| event.kind == "job-ended" || event.kind == "queue-changed")
}

/// What an agent is shown to see an overlap: the other agents' work, read from bmo-job, and the consult room
/// with each agent's window there. None where no consult room is set or the agent does not answer in it.
pub async fn consult_block(receiver: &Receiver, own: &str, now_ms: i64) -> Result<Option<String>, Fault> {
  let work = &CONSULT.work;
  let room = settings::get(receiver.db(), &CONSULT.room_setting)
    .await
    .ok()
    .flatten()
    .and_then(|stored| stored.trim().parse::<i64>().ok());
  let Some(room) = room else { return Ok(None) };
  let states = window_states(receiver, room).await;

  if !states.iter().any(|state| state.agent == own) {
    return Ok(None);
  }

  let listed = receiver.agents().await.ok();
  let agents: Vec<Agent> = listed.as_ref().map(|listed| listed.agents.clone()).unwrap_or_default();
  let default = wake::agents::default_designated(receiver)
    .await
    .ok()
    .map(|one| one.name)
    .unwrap_or(receiver.default_answerer());
  let items = receiver.units().job.items().await.unwrap_or_default();
  let mut listed_items: Vec<String> = vec![];

  for item in items {
    let owner = item
      .worker
      .as_ref()
      .and_then(|worker| {
        agents
          .iter()
          .find(|one| {
            one.row.get("repository").and_then(Value::as_str).map(str::to_owned).unwrap_or_default()
              == worker.as_str()
          })
          .map(|one| one.name.clone())
      })
      .unwrap_or(default.clone());

    if owner == own {
      continue;
    }

    listed_items.push(fill_marks(
      &work.item,
      &[
        ("id", &item.id.to_string()),
        ("owner", &label_of(receiver, &owner).await),
        ("approval", if item.approved { &work.approved } else { &work.awaiting }),
        ("status", &item.status),
        ("title", &item.title),
      ],
    ));
  }

  let rest = listed_items.len().saturating_sub(CONSULT.shown_items);
  let room_name = match rows::chat_title(receiver.db(), room).await.ok().flatten() {
    Some(title) => fill_marks(&work.room_titled, &[("title", &title), ("chat", &room.to_string())]),
    None => fill_marks(&work.room_bare, &[("chat", &room.to_string())]),
  };

  let mut lines = vec![work.marker.clone(), work.heading.clone()];

  if listed_items.is_empty() {
    lines.push(work.nothing.clone());
  }

  lines.extend(listed_items.iter().take(CONSULT.shown_items).map(String::clone));

  if rest > 0 {
    lines.push(fill_marks(&work.more, &[("count", &rest.to_string())]));
  }

  lines.push(fill_marks(&work.room, &[("room", &room_name)]));
  lines.extend(states.iter().map(|state| window_line(state, own, now_ms)));
  lines.push(work.closing.clone());

  Ok(Some(lines.join("\n")))
}

//! The wake (docs/wake.md, 「Who reaches the session」): which designated agents of the room a message wakes,
//! decided for every one of them by one rule, and each woken agent's turn handed on to its conversations in the
//! one runner, every agent the same way. Everything else stops here, before any queue, so no gate and no turn is
//! spent on it. A conversation already running in the room is no ground: an instruction or a skipped round keeps
//! one alive with no window, and a room is never listened to on that account. A room added `record-only` and a
//! read-only room wake no agent at all.

pub mod agents;
pub mod question;
pub mod rest;
pub mod standing;

use futures_util::future::BoxFuture;

use receiver_policy::wake::WAKE;

use super::{How, Inbound, NotReceived, Receiver, Room, Wake, rows};
use crate::roads::added::{self, RoomMode};
use crate::roads::providers;
use crate::secrets;
use agents::{
  Designated, default_designated, group_room, holds_own, own_number, owned_by, room_designated,
  window_until,
};
use rest::{resting, stand_in};

/// How a message is woken on: the words its call is read from, a file's caption where it carries one, and
/// whether an agent filed it.
pub struct Basis<'a> {
  text: &'a str,
  by_agent: bool,
}

/// Why a message wakes one designated agent: it is called, answered, alone in the room, or inside its window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ground {
  Called,
  Answered,
  Alone,
  Window,
}

impl Ground {
  // A call or a reply to the agent's own words opens its window in the room, as an answer of its own does.
  fn opens_window(self) -> bool {
    matches!(self, Self::Called | Self::Answered)
  }
}

// A unit's answer that did not come is said once and read as no answer, so one unit that does not answer
// never stops a room another unit holds.
pub(crate) fn asked<T>(what: &str, answer: Result<T, impl std::fmt::Display>) -> Option<T> {
  answer.map_err(|why| eprintln!("bmo-receiver: the wake asked {what}: {why}")).ok()
}

/// The wake: the one rule every message is judged by.
pub struct Waking;

impl Wake for Waking {
  fn words<'a>(
    &'a self,
    receiver: &'a Receiver,
    inbound: &'a Inbound,
    room: Room,
    owner: bool,
    how: How,
  ) -> BoxFuture<'a, Result<(), NotReceived>> {
    Box::pin(async move {
      // Words that are nothing but the secret marker are no words, and wake nobody (docs/wake.md, 「The names and
      // the window」).
      if secrets::only_marked(&inbound.text) {
        return Ok(());
      }

      woken_on(
        receiver,
        inbound,
        room,
        owner,
        &Basis {
          text: &inbound.text,
          by_agent: how == How::Filed,
        },
      )
      .await
    })
  }

  fn file<'a>(
    &'a self,
    receiver: &'a Receiver,
    inbound: &'a Inbound,
    room: Room,
    owner: bool,
    caption: &'a str,
  ) -> BoxFuture<'a, Result<(), NotReceived>> {
    Box::pin(wake_file(receiver, inbound, room, owner, caption))
  }
}

/// A message woken on: every designated agent of the room it wakes is handed it.
pub async fn woken_on(
  receiver: &Receiver,
  inbound: &Inbound,
  room: Room,
  owner: bool,
  basis: &Basis<'_>,
) -> Result<(), NotReceived> {
  let archive = receiver.db();
  let chat_id = inbound.chat_id;
  let now = crate::now_ms();

  // The console reads a job's questions beside the queue, so a question settled here is news of the same kind
  // as a moved row, and so is one whose clock a talk has stopped.
  if owner && room.owner_room && question::answered(receiver, chat_id, &inbound.text, basis.text).await? {
    asked("the queue's standing moved", receiver.units.job.queue_moved(None).await);

    return Ok(());
  }

  if wakes_nobody(receiver, chat_id).await? {
    return Ok(());
  }

  // Dropped here even where it calls an agent, so it is said: a road that filed a message too early in the
  // room's line is found in the journal, not taken for one never sent.
  if rows::is_read(archive, chat_id, inbound.message_id).await? {
    let position = rows::last_read(archive, chat_id).await?.unwrap_or_default();

    println!(
      "routing: chat {chat_id} message {} stands at or before the read position (message {position}), so it wakes nobody",
      inbound.message_id
    );

    return Ok(());
  }

  if !basis.by_agent
    && !owner
    && let Some(sender) = inbound.sender_id
    && standing::is_muted(receiver, chat_id, sender).await?
  {
    return Ok(());
  }

  let rest_gated = !room.owner_room && resting(receiver).await?;
  let (agents, alone) = room_designated(receiver, chat_id, room).await?;

  for agent in &agents {
    let Some(ground) = ground(receiver, agent, inbound, room, alone, basis, now).await? else {
      continue;
    };
    let files = if !owner && !basis.by_agent {
      files_leading_up(receiver, inbound, &agents).await?
    } else {
      Vec::new()
    };

    // Every woken agent's turn goes to its own conversations, which open and close its window in the room.
    hand(
      receiver,
      &agent.name,
      inbound,
      &files,
      ground.opens_window(),
      ground == Ground::Window,
      room,
      rest_gated,
    )
    .await;
  }

  if let Some(own) = called_own(receiver, inbound, room, &agents, basis).await? {
    hand(receiver, &own.name, inbound, &[], true, false, room, rest_gated).await;
  }

  Ok(())
}

/// The caller's own supervisor a person calls by name into a group room that does not name it, where the room
/// grants the person `ask` (docs/wake.md, 「Who reaches the session」); none otherwise.
async fn called_own(
  receiver: &Receiver,
  inbound: &Inbound,
  room: Room,
  agents: &[Designated],
  basis: &Basis<'_>,
) -> Result<Option<Designated>, NotReceived> {
  if !group_room(room) || basis.by_agent {
    return Ok(None);
  }

  let Some(sender) = inbound.sender_id else {
    return Ok(None);
  };
  let Some(user) = receiver.units.web.user_of_sender(sender).await.ok().flatten() else {
    return Ok(None);
  };
  let Some(own) = owned_by(receiver, &user).await? else {
    return Ok(None);
  };

  if agents.iter().any(|agent| agent.name == own.name) || !own.called_by(basis.text) {
    return Ok(None);
  }

  let check = receiver_ports::web::RoomCheck {
    room: inbound.chat_id,
    sender: Some(sender),
    relation: "ask".to_owned(),
  };
  let granted = receiver.units.web.check_room(&check).await.is_ok_and(|checked| checked.allowed);

  Ok(granted.then_some(own))
}

/// A message carrying a file, once its file is gathered, woken on by the same rule as words, read off its
/// caption: a room rule naming the sender is no ground by itself, so a picture with no caption in a group whose
/// window is closed wakes nobody.
pub async fn wake_file(
  receiver: &Receiver,
  inbound: &Inbound,
  room: Room,
  owner: bool,
  caption: &str,
) -> Result<(), NotReceived> {
  woken_on(
    receiver,
    inbound,
    room,
    owner,
    &Basis {
      text: caption,
      by_agent: false,
    },
  )
  .await
}

/// Whether a message wakes one agent of its room, by the rule every message is judged by.
pub async fn wakes(
  receiver: &Receiver,
  agent: &str,
  inbound: &Inbound,
  room: Room,
) -> Result<bool, NotReceived> {
  if secrets::only_marked(&inbound.text) || wakes_nobody(receiver, inbound.chat_id).await? {
    return Ok(false);
  }

  let (agents, alone) = room_designated(receiver, inbound.chat_id, room).await?;
  let Some(agent) = agents.iter().find(|one| one.name == agent) else {
    return Ok(false);
  };

  Ok(
    ground(
      receiver,
      agent,
      inbound,
      room,
      alone,
      &Basis {
        text: &inbound.text,
        by_agent: false,
      },
      crate::now_ms(),
    )
    .await?
    .is_some(),
  )
}

// Whether the room wakes no agent at all: one added for the user alone, or a read-only room, which only the
// owner's account reads (docs/receiver.md, 「A read-only room」).
async fn wakes_nobody(receiver: &Receiver, chat_id: i64) -> Result<bool, NotReceived> {
  if added::mode_of(receiver.db(), chat_id).await? == Some(RoomMode::RecordOnly) {
    return Ok(true);
  }

  // An archived room is locked: its record stands and nothing in it wakes an agent (docs/receiver.md, 「An archived
  // room」).
  if crate::roads::archived::archived(receiver.db(), chat_id).await?.is_some() {
    return Ok(true);
  }

  Ok(providers::read_only(receiver.db(), receiver.roads(), chat_id).await?)
}

// Why a message wakes one agent of its room, or nothing where it does not. One rule for every agent, whoever
// wrote the message: its own words never wake it.
async fn ground(
  receiver: &Receiver,
  agent: &Designated,
  inbound: &Inbound,
  room: Room,
  alone: bool,
  basis: &Basis<'_>,
  now: i64,
) -> Result<Option<Ground>, NotReceived> {
  let archive = receiver.db();
  let own = own_number(receiver, agent, &inbound.via, room.identity).await?;

  // Its own words are those filed under its number, and those the record keeps as its send.
  if (inbound.sender_id.is_some() && inbound.sender_id == own)
    || holds_own(archive, inbound.chat_id, inbound.message_id, &agent.name, None).await?
  {
    return Ok(None);
  }

  if agent.called_by(basis.text) {
    return Ok(Some(Ground::Called));
  }

  if let Some(replied) = inbound.reply_to_message_id
    && holds_own(archive, inbound.chat_id, replied, &agent.name, own).await?
  {
    return Ok(Some(Ground::Answered));
  }

  // A private chat needs no name for a person's words alone, so two agents answering it do not wake each other on
  // every answer.
  if alone && !basis.by_agent {
    return Ok(Some(Ground::Alone));
  }

  Ok((window_until(archive, inbound.chat_id, &agent.name).await? > now).then_some(Ground::Window))
}

/// The files a person posted just before words of theirs that woke the default answerer, which woke nobody
/// themselves: a file is held to the same grounds as words, so a member's file and the request typed after it
/// reach the session only where the request brings the file along.
async fn files_leading_up(
  receiver: &Receiver,
  inbound: &Inbound,
  agents: &[Designated],
) -> Result<Vec<Inbound>, NotReceived> {
  let archive = receiver.db();
  let Some(sender) = inbound.sender_id else {
    return Ok(Vec::new());
  };

  let earlier =
    rows::recent_messages(archive, inbound.chat_id, WAKE.file_lead_messages, &inbound.via).await?;
  let before = earlier
    .iter()
    .position(|one| one.message_id == inbound.message_id)
    .map_or(&earlier[..], |at| &earlier[..at]);
  let mut files = Vec::new();

  for one in before {
    let leads = one.sender_id == Some(sender)
      && one.message_id != inbound.message_id
      && (inbound.sent_at - WAKE.file_lead_seconds * 1_000..=inbound.sent_at).contains(&one.sent_at)
      && !agents.iter().any(|agent| agent.called_by(&one.text));

    if leads && rows::carries_file(archive, one.chat_id, one.message_id).await? {
      files.push(one.clone());
    }
  }

  Ok(files)
}

/// A woken agent's turn handed to its conversations: the files leading up to the message first, then the
/// message, opening the window where its ground does, each read through a reader where it came by a mode its
/// road cannot pin an answer to; whether it was taken. A turn the rest's gate closed is taken only where a
/// stand-in answers on the agent's road.
#[allow(clippy::too_many_arguments)]
pub async fn hand(
  receiver: &Receiver,
  agent: &str,
  inbound: &Inbound,
  files: &[Inbound],
  opens_window: bool,
  window_only: bool,
  room: Room,
  rest_gated: bool,
) -> bool {
  if rest_gated
    && asked("whether a stand-in answers the agent", stand_in(receiver, agent).await) != Some(true)
  {
    return false;
  }

  let Some(runner) = receiver.runner() else {
    eprintln!(
      "chat {}: no conversations stand to take message {}",
      inbound.chat_id, inbound.message_id
    );

    return false;
  };

  // An agent of an account whose machine is gone is woken nowhere (docs/wake.md, 「Who reaches the session」).
  if !agents::account_stands(receiver, agent).await {
    println!(
      "chat {}: {agent}'s account has no machine attached, so message {} wakes it not",
      inbound.chat_id, inbound.message_id
    );

    return false;
  }

  let pinnable = receiver.roads().pinnable(&inbound.via, inbound.mode.as_deref()).await;
  let read_through = inbound.via_reader.unwrap_or(!pinnable);
  let carried = |message: &Inbound| Inbound {
    via_reader: Some(read_through),
    ..message.clone()
  };
  let carried_files = files.iter().map(carried).collect::<Vec<Inbound>>();

  runner
    .hand(agent, &carried_files, &carried(inbound), room, opens_window, window_only)
    .await
}

/// Words the owner said through another road — the screen, a talk — handed straight to his own conversation as
/// his own, opening nothing; whether a conversation took them.
pub async fn hand_owner_words(receiver: &Receiver, message: Inbound) -> Result<bool, NotReceived> {
  let Some(runner) = receiver.runner() else {
    eprintln!(
      "chat {}: no conversations stand to take message {}",
      message.chat_id, message.message_id
    );

    return Ok(false);
  };

  // His own chat, where who the default answerer is there is the road's to say and these words name none.
  let room = Room {
    owner_room: true,
    identity: None,
    private_to: None,
  };
  let agent = default_designated(receiver).await?;

  Ok(
    runner
      .hand(
        &agent.name,
        &[],
        &Inbound {
          via_reader: Some(false),
          ..message
        },
        room,
        false,
        false,
      )
      .await,
  )
}

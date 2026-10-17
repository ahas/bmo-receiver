//! A person's own thread, followed into their private room (docs/rust-migration.md, 「Continuity and room
//! isolation」): in a user's private room, the turn is handed what that user and BMO said to each other in the
//! group rooms the user is in and BMO answers over the last days, and nothing anyone else said nor what the user said
//! to a companion there. It is a reference document like any other, so it passes the same sanitiser and goes to a
//! session in full once per version.

use crate::js;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::fill_marks;

use super::shape::iso_instant;
use crate::Fault;
use crate::receiver::wake::agents::Designated;
use crate::receiver::{Inbound, Receiver, rows, wake};
use crate::store::settings;
use crate::{all, now_ms};
use receiver_ports::gate::{Entry, Source};

struct ThreadLine {
  room: i64,
  at: i64,
  text: String,
}

fn said_line(message: &Inbound, who: &str) -> String {
  let thread = &CHAT_ROUND.thread;
  let mut flat = String::new();
  let mut spaced = false;

  for character in message.text.chars() {
    if js::is_space(character) {
      spaced = true;
    } else {
      if spaced && !flat.is_empty() {
        flat.push(' ');
      }

      spaced = false;
      flat.push(character);
    }
  }

  let kept = if js::units(&flat) > thread.line_chars {
    let units: Vec<u16> = flat.encode_utf16().take(thread.line_chars).collect();

    format!("{}{}", String::from_utf16_lossy(&units), thread.cut)
  } else {
    flat
  };
  let at: String = iso_instant(message.sent_at).chars().take(16).collect();

  fill_marks(&thread.line, &[("at", &at), ("who", who), ("text", &kept)])
}

/// Whom one of the user's messages in a group room was said to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Said {
  ToBmo,
  ToCompanion,
  /// Neither its words nor what it replies to tell; the agent answering it first does.
  Open,
}

// One room's part of the thread: the user's messages said to BMO, and BMO's lines answering them — a line replying
// to one of theirs, or BMO's lines that follow theirs before anyone else speaks. A message whose addressee is
// open is kept only where BMO's line follows it before anyone else's.
fn exchanges(
  room: i64,
  messages: &[Inbound],
  own: &[i64],
  bot_name: &str,
  said: impl Fn(&Inbound) -> Said,
) -> Vec<ThreadLine> {
  let mut lines = Vec::new();
  let mut theirs = std::collections::HashSet::new();
  let mut open: Vec<&Inbound> = Vec::new();
  let mut answering = false;

  for message in messages {
    if message.sender_id.is_some_and(|sender| own.contains(&sender)) {
      match said(message) {
        Said::ToBmo => {
          theirs.insert(message.message_id);
          lines.push(ThreadLine {
            room,
            at: message.sent_at,
            text: said_line(message, &message.sender_name),
          });
          answering = true;
        }
        Said::ToCompanion => {
          open.clear();
          answering = false;
        }
        Said::Open => open.push(message),
      }
    } else if message.sender_name == bot_name {
      let replied = message.reply_to_message_id;
      let answers_open =
        !open.is_empty() && replied.is_none_or(|id| open.iter().any(|one| one.message_id == id));

      if answers_open {
        for one in open.drain(..) {
          theirs.insert(one.message_id);
          lines.push(ThreadLine {
            room,
            at: one.sent_at,
            text: said_line(one, &one.sender_name),
          });
        }

        answering = true;
      }

      if answering || replied.is_some_and(|id| theirs.contains(&id)) {
        lines.push(ThreadLine {
          room,
          at: message.sent_at,
          text: said_line(message, bot_name),
        });
      }
    } else {
      open.clear();
      answering = false;
    }
  }

  lines
}

// The answerers of a room BMO shares with companions, and the numbers the companions' lines are filed under.
struct Hearers {
  bmo: Designated,
  companions: Vec<Designated>,
  numbers: Vec<i64>,
  sender_name: String,
}

// Whom the user's messages in one room were said to. In a room BMO answers alone, everything is said to BMO; in
// one it shares with companions, a call by name tells first, then whose line the message replies to.
async fn hearers(receiver: &Receiver, room: i64, bmo: &str) -> Result<Option<Hearers>, Fault> {
  let db = receiver.db();
  let named = wake::agents::named_designated(receiver, room).await.unwrap_or_default();
  let (bmo, companions): (Vec<Designated>, Vec<Designated>) =
    named.into_iter().partition(|agent| agent.name == bmo);
  let Some(bmo) = bmo.into_iter().next() else {
    return Ok(None);
  };

  if companions.is_empty() {
    return Ok(None);
  }

  let names: Vec<sea_orm::Value> = companions.iter().map(|agent| agent.name.clone().into()).collect();
  let marks: Vec<String> = (1..=names.len()).map(|at| format!("${at}")).collect();
  let numbers = all(
    db,
    &format!(r#"SELECT "sender_id" FROM "provider_users" WHERE "user_id" IN ({})"#, marks.join(", ")),
    names,
  )
  .await?
  .iter()
  .filter_map(|row| row.try_get::<i64>("", "sender_id").ok())
  .collect();

  Ok(Some(Hearers {
    bmo,
    companions,
    numbers,
    sender_name: receiver.deployment().sender_name.clone(),
  }))
}

impl Hearers {
  fn said(&self, message: &Inbound, room: &[Inbound]) -> Said {
    if self.bmo.called_by(&message.text) {
      return Said::ToBmo;
    }

    if self.companions.iter().any(|agent| agent.called_by(&message.text)) {
      return Said::ToCompanion;
    }

    let replied = message.reply_to_message_id.and_then(|id| room.iter().find(|one| one.message_id == id));

    match replied {
      Some(line) if line.sender_name == self.sender_name => Said::ToBmo,
      Some(line) if line.sender_id.is_some_and(|sender| self.numbers.contains(&sender)) => {
        Said::ToCompanion
      }
      _ => Said::Open,
    }
  }
}

/// The thread for a turn in a room, or none where the room is no user's private room, the model of users is not
/// in force, the setting turns it off, or the user said nothing in a group room lately.
pub async fn thread_entry(receiver: &Receiver, chat_id: i64) -> Result<Option<Entry>, Fault> {
  let db = receiver.db();
  let thread = &CHAT_ROUND.thread;

  if !receiver.units().web.model_in_force().await.unwrap_or(false) {
    return Ok(None);
  }

  if settings::get(db, &thread.setting).await.ok().flatten().as_deref().map(str::trim) == Some("off") {
    return Ok(None);
  }

  let room = receiver.roads().room_facts(chat_id).await;
  let Some(person) = room.private_to else { return Ok(None) };
  let Some(user) = receiver.units().web.user_of_sender(person).await.ok().flatten() else {
    return Ok(None);
  };
  let senders: Vec<i64> = receiver.units().web.senders_of_user(&user).await.unwrap_or_default();
  let since = now_ms() / 1000 - thread.days * 86_400;
  let mut lines = Vec::new();

  if !senders.is_empty() {
    let marks: Vec<String> = (1..=senders.len()).map(|at| format!("${at}")).collect();
    let mut values: Vec<sea_orm::Value> = senders.iter().map(|sender| (*sender).into()).collect();

    values.push(since.into());

    let rooms = all(
      db,
      &format!(
        r#"SELECT DISTINCT "chat_id" FROM "messages" WHERE "sender_id" IN ({}) AND "sent_at" >= ${} ORDER BY "chat_id""#,
        marks.join(", "),
        senders.len() + 1
      ),
      values,
    )
      .await?;
    let bmo = wake::agents::default_designated(receiver)
      .await
      .ok()
      .map(|agent| agent.name)
      .unwrap_or(receiver.deployment().agent.clone());

    for row in rooms {
      let other: i64 = row.try_get("", "chat_id")?;

      // Every one-to-one apart, since those are no group's.
      if receiver.roads().room_facts(other).await.private_to.is_some() {
        continue;
      }

      // A room the agent does not answer is a companion's: what the user says there is said to it, not to the
      // agent. In a room it shares with one, what the user says to the companion stays out too.
      let (answerers, _) =
        wake::agents::room_designated(receiver, other, receiver.roads().room_facts(other).await)
          .await
          .unwrap_or_default();

      if !answerers.iter().any(|agent| agent.name == bmo) {
        continue;
      }

      let recent: Vec<Inbound> = rows::recent_messages(db, other, thread.lookback, "")
        .await?
        .into_iter()
        .filter(|message| message.sent_at >= since)
        .collect();

      lines.extend(match hearers(receiver, other, &bmo).await? {
        Some(hearers) => {
          exchanges(other, &recent, &senders, &receiver.deployment().sender_name, |message| {
            hearers.said(message, &recent)
          })
        }
        None => exchanges(other, &recent, &senders, &receiver.deployment().sender_name, |_| Said::ToBmo),
      });
    }
  }

  if lines.is_empty() {
    return Ok(None);
  }

  // The newest kept where the whole runs past the ceiling.
  lines.sort_by_key(|line| line.at);

  let mut total: usize = lines.iter().map(|line| js::units(&line.text) + 1).sum();
  let mut first = 0;

  while lines.len() - first > 1 && total > thread.ceiling_chars {
    total -= js::units(&lines[first].text) + 1;
    first += 1;
  }

  let kept = &lines[first..];
  let mut rooms: Vec<i64> = Vec::new();

  for line in kept {
    if !rooms.contains(&line.room) {
      rooms.push(line.room);
    }
  }

  let mut text = vec![thread.heading.clone()];

  for room in rooms {
    text.push(fill_marks(&thread.room, &[("room", &room.to_string())]));
    text.extend(kept.iter().filter(|line| line.room == room).map(|line| line.text.clone()));
  }

  Ok(Some(Entry {
    path: fill_marks(&thread.path, &[("user", &user.to_string())]),
    text: text.join("\n"),
    source: Source::Archive,
    fragment: None,
  }))
}

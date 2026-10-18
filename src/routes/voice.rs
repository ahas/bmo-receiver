//! The voice briefing's switches and a line's full reading (docs/boundary-api.md, 「The voice briefing」;
//! docs/voiced-notices.md): the whole briefing's switch and its playing by itself set from the console, and a
//! line's full reading asked for from its chip — answered as it stands where one is made or being made, launched
//! as a kept reading otherwise. Only an agent's own line, in a voice its row names, has one.
//! reading asked for from its chip — answered as it stands where one is made or being made, launched as a kept
//! reading otherwise. Only an agent's own line, in a voice its row names, has one.

use axum::{body::Bytes, extract::State, http::StatusCode};
use serde::Deserialize;
use serde_json::json;

use receiver_codes::{VoiceFact, VoiceRefusal};
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::roads::ROADS;
use receiver_ports::voice::{Briefing, BriefingKind};

use sea_orm::ConnectionTrait;

use crate::chat::instructions::{Queued, queue};
use crate::chat::voice::{switched_off, voiced};
use crate::events;
use crate::receiver::notices::frame;
use crate::receiver::wake::agents::{own_number, room_designated, voice_of};
use crate::receiver::{NotReceived, Receiver};
use crate::routes::{Answered, Refusal, Served, answered, code, read, refused, store, unit};
use crate::store::{Fault, settings};
use crate::{now_ms, one};

// The settings the two switches are kept under, each on unless it holds "off".
const BRIEFING: &str = "voice_briefing";
const AUTOPLAY: &str = "voice_briefing_autoplay";

#[derive(Deserialize)]
pub struct Switched {
  #[serde(default)]
  on: Option<bool>,

  #[serde(default)]
  autoplay: Option<bool>,
}

#[derive(Deserialize)]
pub struct LineAsked {
  #[serde(rename = "chatId")]
  chat_id: i64,

  #[serde(rename = "messageId")]
  message_id: i64,
}

async fn switch_on(db: &impl ConnectionTrait) -> bool {
  settings::get(db, BRIEFING).await.ok().flatten().as_deref() != Some("off")
}

async fn autoplay_on(db: &impl ConnectionTrait) -> bool {
  settings::get(db, AUTOPLAY).await.ok().flatten().as_deref() != Some("off")
}

/// The owner told the whole briefing moved, as a voiced notice for the chat agent's session.
async fn told(receiver: &Receiver, fact: &VoiceFact) -> Result<(), Fault> {
  let text = frame(&fact.to_string(), &CHAT_ROUND.voice.switch_intent, None, false);
  let queued = Queued {
    kind: Some(&CHAT_ROUND.voice.switch_kind),
    expires_at_ms: Some(now_ms() + CHAT_ROUND.voice.switch_ttl_ms),
    ..Queued::default()
  };

  queue(receiver.db(), &receiver.deployment().agent, receiver.owner_id(), &text, &queued)
    .await
    .map(drop)
}

/// One switch set — the whole briefing or its playing by itself — answered with both as they now stand.
pub async fn switch(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<Switched>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();
  let db = receiver.db();

  match (asked.on, asked.autoplay) {
    (Some(whole), None) => {
      let was = switch_on(db).await;

      settings::set(db, BRIEFING, if whole { "on" } else { "off" }).await.map_err(store)?;

      // A write that changes nothing says nothing, and one kind stands for the switch, so flipping it
      // twice before the session speaks leaves one line, about where it ended.
      if was != whole {
        let fact = if whole { VoiceFact::BriefingOn } else { VoiceFact::BriefingOff };

        told(&receiver, &fact).await.map_err(store)?;

        if let Some(pass) = receiver.pass() {
          pass.kick();
        }
      }
    }
    (None, Some(autoplay)) => {
      settings::set(db, AUTOPLAY, if autoplay { "on" } else { "off" }).await.map_err(store)?;
    }
    _ => return Err(code(&VoiceRefusal::OneSwitch)),
  }

  answered(json!({
    "ok": true,
    "on": switch_on(db).await,
    "autoplay": autoplay_on(db).await,
  }))
}

/// Whose line a message is among the designated agents of its room, or none where it is nobody's.
async fn line_agent(
  receiver: &Receiver,
  chat_id: i64,
  sender_id: Option<i64>,
  sender_name: &str,
) -> Result<Option<String>, NotReceived> {
  if sender_name == receiver.deployment().sender_name {
    return voice_of(receiver, chat_id).await.map(Some);
  }

  let Some(road) = receiver.roads().room_road(chat_id).await else { return Ok(None) };
  let room = receiver.roads().room_facts(chat_id).await;
  let (designated, _) = room_designated(receiver, chat_id, room).await?;

  for agent in designated {
    let Some(own) = own_number(receiver, &agent, &road, room.identity).await? else {
      continue;
    };

    if Some(own) == sender_id {
      return Ok(Some(agent.name));
    }
  }

  Ok(None)
}

// A part of the receiver's own record that could not be read.
fn reached(why: NotReceived) -> Refusal {
  refused(StatusCode::BAD_GATEWAY, "unit-failed", &why.to_string())
}

fn absent_line(asked: &LineAsked) -> Refusal {
  let (chat, message) = (asked.chat_id, asked.message_id);

  refused(
    StatusCode::NOT_FOUND,
    "not-found",
    &format!("No agent's line {message} in chat {chat} to read."),
  )
}

/// A line's full reading, asked for from its chip.
pub async fn line(State(served): State<Served>, body: Bytes) -> Answered {
  let asked = read::<LineAsked>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();

  if switched_off(&receiver).await {
    return Err(code(&VoiceRefusal::SwitchedOff));
  }

  let row = one(
    receiver.db(),
    r#"SELECT "sender_id", "sender_name", "text" FROM "messages" WHERE "chat_id" = $1 AND "message_id" = $2"#,
    vec![asked.chat_id.into(), asked.message_id.into()],
  )
    .await
    .map_err(store)?;
  let Some(row) = row else { return Err(absent_line(&asked)) };

  let text: String = row.try_get("", "text").ok().unwrap_or_default();
  let sender_id: Option<i64> = row.try_get("", "sender_id").ok().flatten();
  let sender_name: String = row.try_get("", "sender_name").ok().unwrap_or_default();
  let mark = ROADS.marks.voice.as_str();

  if text.trim().is_empty() || text.starts_with(mark) {
    return Err(absent_line(&asked));
  }

  let agent = line_agent(&receiver, asked.chat_id, sender_id, &sender_name).await.map_err(reached)?;
  let Some(agent) = agent else { return Err(absent_line(&asked)) };

  if !voiced(&receiver, &agent).await {
    return Err(code(&VoiceRefusal::Voiceless { label: agent }));
  }

  let briefings = receiver.units().voice.briefings(asked.chat_id, false).await.ok().unwrap_or_default();

  if let Some(standing) = briefings.iter().find(|one| one.message == Some(asked.message_id)) {
    let state = if standing.finished { "ready" } else { "making" };

    return answered(json!({ "ok": true, "state": state }));
  }

  let reading = Briefing {
    chat: asked.chat_id,
    message: Some(asked.message_id),
    kind: BriefingKind::Summary,
    title: CHAT_ROUND.voice.untitled.clone(),
    awaits_line: true,
    text,
  };

  // The switch was asked above, so what is left to refuse is the launch failing, which is the unit's
  // own trouble rather than an answer to the owner.
  if let Err(why) = receiver.units().voice.brief(&reading).await {
    return Err(unit(why));
  }

  receiver.news().publish(&events::message(asked.chat_id, asked.message_id));

  answered(json!({ "ok": true, "state": "making" }))
}

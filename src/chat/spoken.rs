//! An answer said aloud (docs/voiced-notices.md, 「An answer said aloud」): an agent's round asks with `speak` that
//! the answer it writes in the room it answers go out as a voice note in the agent's own voice. The answer is
//! written and checked by the round's stages as every answer is; once it has passed them, the voice engine speaks
//! it as a briefing of the kind `speech`, the agent speaking as itself, which a listener showing the speaker plays,
//! and the note goes out on the room's road with the answer's words beside it.

use std::time::Duration;

use receiver_codes::SendRefusal;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::fill_marks;
use receiver_policy::voice_briefing::VOICE_BRIEFING;
use receiver_ports::voice::NewBriefing;
use room_calls::sends::{Asker, SpeakInput};

use super::briefing::voice_of;
use super::handed::{own_round, turn_room};
use crate::receiver::Receiver;
use crate::verbs::sends::Sends;

/// The kind of briefing an answer said aloud is: the agent speaking as itself in a room.
pub const SPEECH: &str = "speech";

/// How a round's answer goes out said aloud: the emotion it is said with, where the round named one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Spoken {
  pub emotion: Option<String>,
}

/// An answer's speech as the engine made it: the briefing that spoke it and its sound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Voiced {
  pub briefing: i64,
  pub sound: Vec<u8>,
}

// The emotion named, where it names one: a word of the set an answer is said with, or the refusal naming the set.
fn emotion_of(input: &SpeakInput) -> Result<Option<String>, String> {
  let Some(named) = input.emotion.as_deref().map(str::trim).filter(|named| !named.is_empty()) else {
    return Ok(None);
  };

  if CHAT_ROUND.voice.knows(named) {
    return Ok(Some(named.to_owned()));
  }

  Err(
    SendRefusal::SpeakEmotion {
      emotion: named.to_owned(),
      emotions: CHAT_ROUND.voice.emotions.join(", "),
    }
    .to_string(),
  )
}

/// `speak` asked: the answer of the asker's round in the room it answers marked to go out said aloud, answered with
/// the line saying so, or the one sentence saying why nothing was marked. Only a round of the asker's own, in the
/// room it answers, is marked, and only where the agent has a voice, a voice engine is named, the emotion is one of
/// the set and the room's road performs a voice note.
pub async fn mark(receiver: &Receiver, asker: &Asker, input: &SpeakInput) -> Result<String, String> {
  let agent = asker.agent_name.as_deref().map(str::trim).unwrap_or_default().to_owned();
  let room = asker.room.or_else(|| turn_room(asker)).unwrap_or_default();
  let outside = || {
    SendRefusal::SpeakOutsideRound {
      agent: agent.clone(),
      room,
    }
    .to_string()
  };

  if agent.is_empty() || !own_round(receiver, asker) || turn_room(asker) != Some(room) {
    return Err(outside());
  }

  let emotion = emotion_of(input)?;

  if receiver.units().voice.is_none() {
    return Err(SendRefusal::SpeakNoEngine.to_string());
  }

  if voice_of(receiver, &agent).await.is_none() {
    return Err(SendRefusal::SpeakVoiceless { agent }.to_string());
  }

  Sends::new(receiver.clone(), Some(agent.clone()), asker.via.clone())
    .road(room, "voice")
    .await
    .map_err(|why| why.to_string())?;

  let marked = receiver.runner().is_some_and(|runner| {
    runner.speak(
      &agent,
      room,
      Spoken {
        emotion: emotion.clone(),
      },
    )
  });

  if !marked {
    return Err(outside());
  }

  let said = emotion.map_or_else(|| "plainly".to_owned(), |emotion| format!("with the emotion {emotion}"));

  Ok(fill_marks(&CHAT_ROUND.voice.speak_marked, &[("emotion", &said)]))
}

/// The answer's words spoken in the agent's own voice as a `speech` of the room, its sound once it is done, or why
/// there is none: no voice engine, no voice, a refusal or failure of the engine, or a speech that did not end
/// within the policy's bound, which is stopped. The key names the line, so a send tried again takes the same speech.
pub async fn voiced(
  receiver: &Receiver,
  agent: &str,
  chat_id: i64,
  text: &str,
  spoken: &Spoken,
  key: Option<String>,
) -> Result<Voiced, String> {
  let engine = receiver.units().voice.clone().ok_or_else(|| "no voice engine is named".to_owned())?;
  let voice = voice_of(receiver, agent).await.ok_or_else(|| format!("{agent} has no voice"))?;
  let taken = engine
    .brief(&NewBriefing {
      voice_name: voice,
      text: text.to_owned(),
      chat_id,
      message_id: None,
      kind: SPEECH.to_owned(),
      title: None,
      emotion: spoken.emotion.clone(),
      plays: true,
      key,
    })
    .await
    .map_err(|why| format!("the voice engine did not take the speech: {why}"))?;
  let every = Duration::from_millis(VOICE_BRIEFING.speech_read_every_ms);
  let within = Duration::from_millis(VOICE_BRIEFING.speech_within_ms);
  let started = tokio::time::Instant::now();
  let mut standing = taken;

  loop {
    match standing.state.as_str() {
      "done" => break,
      "failed" | "stopped" => {
        return Err(format!("speech {} {} in the voice engine", standing.id, standing.state));
      }
      _ if started.elapsed() >= within => {
        if let Err(why) = engine.stop_in(chat_id, SPEECH).await {
          eprintln!("chat {chat_id}: speech {} was not stopped: {why}", standing.id);
        }

        return Err(format!("speech {} did not end within {} seconds", standing.id, within.as_secs()));
      }
      _ => {}
    }

    tokio::time::sleep(every).await;
    standing = engine
      .briefing(standing.id)
      .await
      .map_err(|why| format!("speech {} could not be read: {why}", standing.id))?;
  }

  let sound = engine
    .sound(standing.id)
    .await
    .map_err(|why| format!("the sound of speech {} could not be fetched: {why}", standing.id))?;

  Ok(Voiced {
    briefing: standing.id,
    sound,
  })
}

/// The speech told the line it went out as, so the line carries its own voice; a failure is the journal's alone,
/// since the note is in the room either way.
pub async fn name_line(receiver: &Receiver, chat_id: i64, briefing: i64, message_id: i64) {
  let Some(engine) = receiver.units().voice.clone() else {
    return;
  };

  if let Err(why) = engine.name_line(briefing, message_id).await {
    eprintln!("chat {chat_id}: speech {briefing} was not told its line {message_id}: {why}");
  }
}

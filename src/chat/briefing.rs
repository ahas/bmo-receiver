//! The voice briefing (docs/voiced-notices.md, 「The voice briefing」): its two switches, the engine's briefings
//! stopped when the whole one is switched off, the voice an agent speaks in, the emotion a session's answer names,
//! a notice turn's summary handed to the voice engine, and a line's full reading launched or answered as it stands
//! there. The engine speaks and keeps every briefing; this decides when one is spoken.

use std::collections::HashSet;
use std::sync::{LazyLock, Mutex, PoisonError};

use receiver_codes::{VoiceFact, VoiceRefusal};
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_ports::http::Answer;
use receiver_ports::voice::{Briefing, NewBriefing, VoiceEngine};

use super::notices::{Voiced, queue_notice};
use crate::receiver::{NotReceived, Receiver};
use crate::store::settings;

/// The setting the whole briefing is switched by, on unless it holds `off`.
pub const SWITCH: &str = "voice_briefing";

/// The setting whether a summary plays by itself is switched by, on unless it holds `off`.
pub const AUTOPLAY: &str = "voice_briefing_autoplay";

/// The kind the owner's line about the switch stands under, one waiting at a time.
const SWITCH_KIND: &str = "voice-briefing";

// How short a line's first sentence is for its second to be spoken with it.
const SHORT_OPENING: usize = 40;

/// Whether a switch is on: any value but `off`, and no row at all.
pub async fn on(receiver: &Receiver, key: &str) -> bool {
  settings::get(receiver.db(), key).await.ok().flatten().as_deref() != Some("off")
}

/// The two switches as they stand.
pub async fn switches(receiver: &Receiver) -> (bool, bool) {
  (on(receiver, SWITCH).await, on(receiver, AUTOPLAY).await)
}

/// One switch set; setting the whole briefing off stops every briefing the engine holds, and moving it queues its
/// line for the owner's private chat, where a write that changes nothing says nothing.
pub async fn set(receiver: &Receiver, key: &str, value: bool) -> Result<(), NotReceived> {
  let was = on(receiver, key).await;

  settings::set(receiver.db(), key, if value { "on" } else { "off" }).await?;

  if key == SWITCH && !value {
    silence(receiver);
  }

  if key == SWITCH && was != value {
    let fact = if value { VoiceFact::BriefingOn } else { VoiceFact::BriefingOff };
    let notice = Voiced {
      fact: fact.to_string(),
      intent: "tell the owner that the voice briefing was switched, and what that means for what he hears."
        .to_owned(),
      follow_up: None,
      optional: false,
      kind: Some(SWITCH_KIND.to_owned()),
      events: vec![],
      rides_ms: None,
    };

    queue_notice(receiver, receiver.owner_room(), &notice).await?;
    super::restart::kick(receiver);
  }

  Ok(())
}

/// Every briefing the engine holds that has not ended stopped, as the whole briefing switched off asks: asked now,
/// and where the engine did not take the stop, asked again at the policy's pace for as long as the switch stays
/// off. Nothing waits on it, since a call to an engine that is down waits until it listens.
pub fn silence(receiver: &Receiver) {
  let Some(engine) = receiver.units().voice.clone() else {
    return;
  };
  let receiver = receiver.clone();

  tokio::spawn(async move {
    while !stopped(engine.as_ref()).await {
      tokio::time::sleep(receiver.voice_stop_pace()).await;

      if switched_back_on(&receiver).await {
        println!("voice briefing: switched on again, so its stop is asked no more");
        return;
      }
    }
  });
}

/// The engine asked once to stop every room's briefings; whether it did.
async fn stopped(engine: &dyn VoiceEngine) -> bool {
  match engine.stop_all().await {
    Ok(stopped) => {
      println!("voice briefing: switched off, {} briefings stopped", stopped.len());
      true
    }
    Err(why) => {
      eprintln!(
        "voice briefing: switched off, but the engine did not stop its briefings, asked again: {why}"
      );
      false
    }
  }
}

/// Whether the whole switch reads on again; a store that cannot be read leaves it off, so the stop is still asked.
async fn switched_back_on(receiver: &Receiver) -> bool {
  settings::get(receiver.db(), SWITCH)
    .await
    .is_ok_and(|value| value.as_deref() != Some("off"))
}

/// A briefing handed to the engine while the whole switch stands on; none where it is off. One the engine took
/// while the switch was being set off is stopped with the rest, since the switch is read again once it is taken.
async fn hand_over(
  receiver: &Receiver,
  engine: &dyn VoiceEngine,
  briefing: &NewBriefing,
) -> Option<Answer<Briefing>> {
  if !on(receiver, SWITCH).await {
    return None;
  }

  let taken = engine.brief(briefing).await;

  if taken.is_ok() && !on(receiver, SWITCH).await {
    silence(receiver);
  }

  Some(taken)
}

/// The voice an agent's lines are spoken in, the one its registration names; none where it names none.
pub async fn voice_of(receiver: &Receiver, agent: &str) -> Option<String> {
  let agents = receiver.agents().await.ok()?;
  let registered = agents.agents.iter().find(|one| one.name == agent)?;

  registered
    .row
    .get("voice")
    .and_then(serde_json::Value::as_str)
    .map(str::trim)
    .filter(|voice| !voice.is_empty())
    .map(str::to_owned)
}

// The label an agent is named by in a refusal.
async fn label_of(receiver: &Receiver, agent: &str) -> String {
  let agents = receiver.agents().await.ok();

  agents
    .and_then(|agents| agents.agents.into_iter().find(|one| one.name == agent))
    .and_then(|registered| registered.label)
    .filter(|label| !label.trim().is_empty())
    .unwrap_or_else(|| agent.to_owned())
}

/// The opening of a line, spoken for a notice turn whose session gave no summary: its first sentence, or its first
/// two where the first is short.
pub fn line_opening(text: &str) -> String {
  let mut sentences: Vec<String> = vec![];
  let mut current = String::new();
  let mut characters = text.chars().peekable();

  while let Some(character) = characters.next() {
    if character == '\n' {
      sentences.push(std::mem::take(&mut current));
      continue;
    }

    current.push(character);

    let ends = matches!(character, '.' | '?' | '!' | '…')
      && characters.peek().is_none_or(|next| next.is_whitespace());

    if ends {
      sentences.push(std::mem::take(&mut current));
    }
  }

  sentences.push(current);

  let sentences: Vec<&str> = sentences.iter().map(|one| one.trim()).filter(|one| !one.is_empty()).collect();
  let first = sentences.first().copied().unwrap_or_default();

  match sentences.get(1) {
    Some(second) if first.chars().count() < SHORT_OPENING => format!("{first} {second}"),
    _ => first.to_owned(),
  }
}

/// An answer's emotion line taken off: the emotion it names where that is one of the briefing's emotions, and the
/// rest of the answer. An answer whose first line does not open on the emotion label is all rest, with no emotion.
pub fn emotion_of(answer: &str) -> (Option<String>, &str) {
  let answer = answer.trim_start();
  let (first, rest) = answer.split_once('\n').unwrap_or((answer, ""));
  let Some(named) = first.trim().strip_prefix(CHAT_ROUND.voice.emotion_label.as_str()) else {
    return (None, answer);
  };
  let named = named.trim();

  (CHAT_ROUND.voice.knows(named).then(|| named.to_owned()), rest)
}

/// What a notice turn's summary says: the emotion it is said with where one is known, its title where it has one, and
/// its words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
  pub emotion: Option<String>,
  pub title: Option<String>,
  pub text: String,
}

/// A session's answer to the summary's instruction read as its emotion, title and words: the emotion line taken
/// off, then the first line the title and the rest the words, or the whole the words where it is one line; none
/// where it gave no summary.
pub fn summary_of(answer: &str) -> Option<Summary> {
  let (emotion, answer) = emotion_of(answer);
  let answer = answer.trim();

  if answer.is_empty() || super::sentinels::is_skip(answer) {
    return None;
  }

  let lines: Vec<&str> = answer.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
  let (title, text) = match lines.as_slice() {
    [] => return None,
    [only] => (None, (*only).to_owned()),
    [title, rest @ ..] => (Some((*title).to_owned()), rest.join(" ")),
  };

  Some(Summary {
    emotion,
    title,
    text,
  })
}

/// A line's full reading as it is spoken: the script's emotion and words where the session wrote a script with
/// words past its emotion line, the line as it stands with no emotion otherwise.
pub fn reading_of(script: Option<&str>, line: String) -> (Option<String>, String) {
  script
    .map(emotion_of)
    .map(|(emotion, words)| (emotion, words.trim().to_owned()))
    .filter(|(_, words)| !words.is_empty() && !super::sentinels::is_skip(words))
    .unwrap_or((None, line))
}

/// A notice turn's summary handed to the voice engine: the session's words where it gave any, the line's opening
/// where it did not. Whether it plays by itself is the second switch as it stands now.
pub async fn speak_summary(
  receiver: &Receiver,
  agent: &str,
  chat_id: i64,
  message_id: i64,
  line: &str,
  answer: Option<&str>,
) {
  let Some(engine) = receiver.units().voice.clone() else {
    return;
  };
  let Some(voice) = voice_of(receiver, agent).await else {
    return;
  };
  let Summary {
    emotion,
    title,
    text,
  } = answer.and_then(summary_of).unwrap_or_else(|| Summary {
    emotion: None,
    title: None,
    text: line_opening(line),
  });

  if text.trim().is_empty() {
    return;
  }

  let briefing = NewBriefing {
    voice_name: voice,
    text,
    chat_id,
    message_id: Some(message_id),
    kind: "summary".to_owned(),
    title,
    emotion,
    plays: on(receiver, AUTOPLAY).await,
  };

  match hand_over(receiver, engine.as_ref(), &briefing).await {
    Some(Ok(taken)) => println!("notice turn in {chat_id}: summary taken as briefing {}", taken.id),
    Some(Err(why)) => eprintln!("notice turn in {chat_id} could not be summarised: {why}"),
    None => {
      println!("notice turn in {chat_id}: the voice briefing was switched off, so no summary is spoken")
    }
  }
}

/// Whether a turn of this agent may be summarised at all: the switch on, a voice engine named and the agent's
/// registration naming a voice.
pub async fn speaks(receiver: &Receiver, agent: &str) -> bool {
  receiver.units().voice.is_some()
    && on(receiver, SWITCH).await
    && voice_of(receiver, agent).await.is_some()
}

/// Where a line's full reading stands: being made, or ready to be heard, with the engine's briefing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reading {
  pub ready: bool,
  /// The engine's briefing; none while the session writes the script.
  pub briefing: Option<i64>,
}

/// Why a line's full reading was not launched: a voice refusal, or no agent's line at that place.
#[derive(Debug)]
pub enum Unread {
  Refused(VoiceRefusal),
  NoLine,
  Failed(NotReceived),
}

impl From<NotReceived> for Unread {
  fn from(why: NotReceived) -> Self {
    Self::Failed(why)
  }
}

/// A line's full reading: answered as it stands in the engine where one is waiting, speaking or done, launched
/// otherwise. Only an agent's own line, in a voice its registration names, has one.
pub async fn reading(receiver: &Receiver, chat_id: i64, message_id: i64) -> Result<Reading, Unread> {
  if !on(receiver, SWITCH).await {
    return Err(Unread::Refused(VoiceRefusal::SwitchedOff));
  }

  let line = crate::one(
    receiver.db(),
    r#"SELECT "text", "agent_name", "sender_name" FROM "messages" WHERE "chat_id" = $1 AND "message_id" = $2"#,
    vec![chat_id.into(), message_id.into()],
  )
  .await
  .map_err(NotReceived::from)?
  .ok_or(Unread::NoLine)?;
  let text: Option<String> =
    line.try_get("", "text").map_err(|why| NotReceived::from(crate::Fault::from(why)))?;
  let agent: Option<String> = line
    .try_get("", "agent_name")
    .map_err(|why| NotReceived::from(crate::Fault::from(why)))?;
  let sender: Option<String> = line
    .try_get("", "sender_name")
    .map_err(|why| NotReceived::from(crate::Fault::from(why)))?;
  let deployment = receiver.deployment();
  // A line the receiver archived names its agent; one under the deployment's sender name is the configured agent's.
  let agent = agent.or_else(|| {
    (sender.as_deref() == Some(deployment.sender_name.as_str())).then(|| deployment.agent.clone())
  });
  let (Some(text), Some(agent)) = (text.filter(|text| !text.trim().is_empty()), agent) else {
    return Err(Unread::NoLine);
  };
  let Some(voice) = voice_of(receiver, &agent).await else {
    return Err(Unread::Refused(VoiceRefusal::Voiceless {
      label: label_of(receiver, &agent).await,
    }));
  };
  let unreachable = |reason: String| Unread::Refused(VoiceRefusal::Unreachable { reason });
  let Some(engine) = receiver.units().voice.clone() else {
    return Err(unreachable("no voice engine is configured".to_owned()));
  };
  let standing = engine
    .of_line(chat_id, message_id, "full")
    .await
    .map_err(|why| unreachable(why.to_string()))?;

  // A reading that failed or was stopped stands for nothing: asking for it again launches a new one.
  if let Some(standing) = standing
    .first()
    .filter(|standing| !["failed", "stopped"].contains(&standing.state.as_str()))
  {
    return Ok(Reading {
      ready: standing.state == "done",
      briefing: Some(standing.id),
    });
  }

  // The script is the session's to write, which takes a turn: the reading answers `making` at once, and a second
  // ask while it is written answers the same without asking again.
  if !WRITING.lock().unwrap_or_else(PoisonError::into_inner).insert((chat_id, message_id)) {
    return Ok(Reading {
      ready: false,
      briefing: None,
    });
  }

  let receiver = receiver.clone();

  tokio::spawn(async move {
    let script = match receiver.runner() {
      Some(runner) => runner.spoken(&agent, chat_id, &text).await,
      None => None,
    };
    let (emotion, text) = reading_of(script.as_deref(), text);
    let briefing = NewBriefing {
      voice_name: voice,
      text,
      chat_id,
      message_id: Some(message_id),
      kind: "full".to_owned(),
      title: None,
      emotion,
      plays: false,
    };

    match hand_over(&receiver, engine.as_ref(), &briefing).await {
      Some(Ok(_)) => {}
      Some(Err(why)) => eprintln!("chat {chat_id}: the full reading of {message_id} was not taken: {why}"),
      None => println!("chat {chat_id}: the voice briefing was switched off, so {message_id} is not read"),
    }

    WRITING.lock().unwrap_or_else(PoisonError::into_inner).remove(&(chat_id, message_id));
  });

  Ok(Reading {
    ready: false,
    briefing: None,
  })
}

// The lines whose full reading's script is being written now.
static WRITING: LazyLock<Mutex<HashSet<(i64, i64)>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

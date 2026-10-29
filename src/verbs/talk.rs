//! `bmo talk`: a line into a room, said as text or spoken in a voice (docs/boundary-api.md, 「The unit
//! command」). A spoke line is synthesised by the voice engine the configuration names, its sound written to
//! this machine and sent as the room's voice note, with the words as the caption.

use std::time::Duration;

use room_calls::sends::{Act, SayInput, VoiceInput};
use serde::Deserialize;
use serde_json::json;

use crate::command::{SayArgs, words};
use crate::config::Config;
use crate::verbs::{VerbError, asked_sends};

/// How long a synthesis is waited for before it is refused.
const SPEAK_WITHIN: Duration = Duration::from_secs(120);

/// How often a synthesis still being spoken is looked at again.
const LOOK_AGAIN: Duration = Duration::from_millis(500);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Spoken {
  id: i64,
  state: String,
}

/// `talk`: the words said in the room, or spoken in `voice` and sent as a voice note with them as its caption.
pub async fn talk(
  config: &Config,
  via: Option<String>,
  args: SayArgs,
  voice: Option<String>,
) -> Result<Vec<String>, VerbError> {
  let chat = args.chat.unwrap_or(config.owner);
  let asker = asked_sends::shell_asker(chat, via, args.by);
  let text = words(args.text)?;

  let Some(voice) = voice else {
    let input = SayInput {
      chat: None,
      via: None,
      text,
      reply: args.reply,
      reply_to: args.reply_to,
    };

    return asked_sends::ask(&config.data_root, asker, Act::Say(input)).await;
  };

  let Some(engine) = config.voice.as_ref() else {
    return Err(VerbError::Refused(
      "talk: a spoken line needs the voice engine's address and token in the configuration".to_owned(),
    ));
  };
  let sound = speak(engine, &voice, &text).await?;
  let path = std::env::temp_dir().join(format!("bmo-talk-{}.ogg", crate::now_ms()));

  std::fs::write(&path, sound)
    .map_err(|why| VerbError::Refused(format!("talk: the sound could not be put on the disk: {why}")))?;

  let input = VoiceInput {
    chat: None,
    via: None,
    path: path.to_string_lossy().into_owned(),
    caption: Some(text),
    said: None,
    reply_to: args.reply_to,
    transcript: None,
  };

  asked_sends::ask(&config.data_root, asker, Act::Voice(input)).await
}

/// The sound of one text spoken in a voice, waited for until the engine has spoken it.
async fn speak(engine: &receiver_ports::http::Peer, voice: &str, text: &str) -> Result<Vec<u8>, VerbError> {
  let base = engine.base.trim_end_matches('/');
  let client = reqwest::Client::builder()
    .timeout(SPEAK_WITHIN)
    .build()
    .map_err(|why| VerbError::Refused(format!("talk: the voice engine's client did not stand: {why}")))?;
  let answer = send(
    client
      .post(format!("{base}/syntheses"))
      .json(&json!({ "voiceName": voice, "text": text })),
    engine,
  )
  .await
  .map_err(|why| VerbError::Refused(format!("talk: the voice engine was not reached: {why}")))?;
  let status = answer.status();

  if !status.is_success() {
    let body = answer.text().await.unwrap_or_default();

    return Err(VerbError::Refused(format!(
      "talk: the voice engine refused the line ({status}): {body}"
    )));
  }

  let mut held: Spoken = answer
    .json()
    .await
    .map_err(|why| VerbError::Refused(format!("talk: the synthesis did not read: {why}")))?;
  let deadline = std::time::Instant::now() + SPEAK_WITHIN;

  while held.state == "waiting" || held.state == "speaking" {
    if std::time::Instant::now() >= deadline {
      return Err(VerbError::Refused(format!(
        "talk: synthesis {} was not spoken within {}s",
        held.id,
        SPEAK_WITHIN.as_secs()
      )));
    }

    tokio::time::sleep(LOOK_AGAIN).await;

    let next = send(client.get(format!("{base}/syntheses/{}", held.id)), engine)
      .await
      .map_err(|why| VerbError::Refused(format!("talk: the synthesis was not read: {why}")))?;

    held = next
      .json()
      .await
      .map_err(|why| VerbError::Refused(format!("talk: the synthesis did not read: {why}")))?;
  }

  if held.state != "done" {
    return Err(VerbError::Refused(format!("talk: synthesis {} ended {}", held.id, held.state)));
  }

  let sound = send(client.get(format!("{base}/syntheses/{}/sound", held.id)), engine)
    .await
    .map_err(|why| VerbError::Refused(format!("talk: the sound was not read: {why}")))?;

  if !sound.status().is_success() {
    return Err(VerbError::Refused(format!("talk: the sound was not answered ({})", sound.status())));
  }

  let bytes = sound
    .bytes()
    .await
    .map_err(|why| VerbError::Refused(format!("talk: the sound did not read: {why}")))?;

  Ok(bytes.to_vec())
}

// One request handed to the voice engine, carrying its token where the configuration names one.
async fn send(
  request: reqwest::RequestBuilder,
  engine: &receiver_ports::http::Peer,
) -> Result<reqwest::Response, reqwest::Error> {
  if engine.token.is_empty() {
    request.send().await
  } else {
    request.bearer_auth(&engine.token).send().await
  }
}

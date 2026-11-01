//! `bmo talk`: a line into a room, said as text or spoken in a voice (docs/boundary-api.md, 「The unit
//! command」). A spoken line is two texts: the words, what the room reads, and the spoken words the voice
//! synthesises, given or else derived by the voice engine the configuration names. Its sound is written to this
//! machine and sent as the room's voice note, with the words as what the note says, which a road that folds a long
//! reading shows folded under the note. The synthesis is followed until it is done or failed, however long it
//! waits for its card.

use std::time::Duration;

use room_calls::sends::{Act, SayInput, VoiceInput};
use serde::Deserialize;
use serde_json::json;

use crate::command::{SayArgs, words};
use crate::config::Config;
use crate::verbs::{VerbError, asked_sends};

/// A refusal as the voice engine answers it: its code, and the sentence saying why.
#[derive(Deserialize)]
struct Refusal {
  code: String,
  error: String,
}

/// How often a synthesis still being spoken is looked at again.
const LOOK_AGAIN: Duration = Duration::from_millis(500);

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Spoken {
  id: i64,
  state: String,
  // What a synthesis still waiting for its card waits on, as the engine says it.
  #[serde(default)]
  waiting_on: Option<String>,
}

/// `talk`: the words said in the room, or spoken in `voice` and sent as a voice note with them as what it says;
/// `spoken`, where given, is what the voice synthesises in their place.
pub async fn talk(
  config: &Config,
  via: Option<String>,
  args: SayArgs,
  voice: Option<String>,
  spoken: Option<String>,
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
  let sound = speak(engine, &voice, &text, spoken.as_deref(), |id, waiting_on| {
    eprintln!("talk: synthesis {id} waits: {waiting_on}")
  })
  .await?;
  let path = std::env::temp_dir().join(format!("bmo-talk-{}.ogg", crate::now_ms()));

  std::fs::write(&path, sound)
    .map_err(|why| VerbError::Refused(format!("talk: the sound could not be put on the disk: {why}")))?;

  let input = VoiceInput {
    chat: None,
    via: None,
    path: path.to_string_lossy().into_owned(),
    caption: None,
    said: Some(text),
    reply_to: args.reply_to,
    transcript: None,
  };

  asked_sends::ask(&config.data_root, asker, Act::Voice(input)).await
}

/// The sound of one text spoken in a voice, waited for until the engine has spoken it or says it failed, with no
/// bound on the wait; `spoken`, where given, is what the engine synthesises, `text` staying the transcript, and
/// where absent the engine derives it. `told` hears the synthesis's number and each `waitingOn` sentence once, as
/// it changes. A refusal carries the engine's code and sentence.
pub async fn speak(
  engine: &receiver_ports::http::Peer,
  voice: &str,
  text: &str,
  spoken: Option<&str>,
  mut told: impl FnMut(i64, &str),
) -> Result<Vec<u8>, VerbError> {
  let base = engine.base.trim_end_matches('/');
  let client = reqwest::Client::builder()
    .build()
    .map_err(|why| VerbError::Refused(format!("talk: the voice engine's client did not stand: {why}")))?;
  let mut synthesis = json!({ "voiceName": voice, "text": text });

  if let Some(spoken) = spoken {
    synthesis["spoken"] = json!(spoken);
  }

  let answer = send(client.post(format!("{base}/syntheses")).json(&synthesis), engine)
    .await
    .map_err(|why| VerbError::Refused(format!("talk: the voice engine was not reached: {why}")))?;
  let status = answer.status();

  if !status.is_success() {
    let body = answer.text().await.unwrap_or_default();
    let (code, why) = match serde_json::from_str::<Refusal>(&body) {
      Ok(refusal) => (format!(" {}", refusal.code), refusal.error),
      Err(_) => (String::new(), body),
    };

    return Err(VerbError::Refused(format!(
      "talk: the voice engine refused the line ({status}{code}): {why}"
    )));
  }

  let mut held: Spoken = answer
    .json()
    .await
    .map_err(|why| VerbError::Refused(format!("talk: the synthesis did not read: {why}")))?;
  let mut said: Option<String> = None;

  while held.state == "waiting" || held.state == "speaking" {
    if let Some(waiting_on) = held.waiting_on.take()
      && said.as_ref() != Some(&waiting_on)
    {
      told(held.id, &waiting_on);
      said = Some(waiting_on);
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

//! A voice note heard before its turn (docs/messages.md, 「Every kind of message」; docs/instruction.md, 「A voice
//! note」): the saved recording is handed to bmo-ear through the tool host, and what it heard, how long it ran and
//! whether it could be heard are kept on the message. A recording that cannot be heard is a state of its own, so
//! the turn says so instead of saying nothing.

use std::path::Path;

use bmo_chat_client::ChatFileKind;
use receiver_policy::roads::ROADS;

use super::files::{self, Handed};
use super::{Receiver, rows};
use crate::Fault;

/// Whether a message's files are a voice note: the service says a recording made to be heard as speech, or the
/// message is the voice mark the receiver itself writes.
fn voice(inbound: &super::Inbound, files: &[impl Handed]) -> bool {
  files.iter().any(|file| file.kind() == Some(ChatFileKind::VoiceNote))
    || (inbound.text.trim_start().starts_with(&ROADS.marks.voice)
      && files.iter().any(|file| file.mime().to_ascii_lowercase().starts_with("audio/")))
}

/// The transcript, length and hearing state of a voice note's message, once its recording stands. A recording
/// that cannot be heard leaves the message saying so; it never leaves it silent.
pub async fn hear(
  receiver: &Receiver,
  inbound: &super::Inbound,
  files: &[impl Handed],
) -> Result<(), Fault> {
  if !voice(inbound, files) {
    return Ok(());
  }

  let Some(path) = files::voice_path(receiver.db(), inbound).await? else {
    return Ok(());
  };

  let Some(hearing) = receiver.units().hearing.clone() else {
    rows::set_voice(receiver.db(), inbound.chat_id, inbound.message_id, None, None, "failed").await?;
    eprintln!(
      "bmo-receiver: the voice note of message {} in room {} could not be heard: no listening tool is configured",
      inbound.message_id, inbound.chat_id
    );

    return Ok(());
  };

  match hearing.transcribe(Path::new(&path)).await {
    Ok(heard) => {
      rows::set_voice(
        receiver.db(),
        inbound.chat_id,
        inbound.message_id,
        Some(&heard.transcript),
        heard.seconds,
        "heard",
      )
      .await?;

      println!(
        "chat {}: the voice note of message {} was heard ({} seconds)",
        inbound.chat_id,
        inbound.message_id,
        heard.seconds.map(|seconds| format!("{seconds:.1}")).unwrap_or_else(|| "?".to_owned())
      );
    }
    Err(why) => {
      rows::set_voice(receiver.db(), inbound.chat_id, inbound.message_id, None, None, "failed").await?;
      eprintln!(
        "bmo-receiver: the voice note of message {} in room {} could not be heard: {why}",
        inbound.message_id, inbound.chat_id
      );
    }
  }

  Ok(())
}

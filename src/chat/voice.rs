//! The voice briefing as the conversations hold it (docs/voiced-notices.md, 「Voice and digest notices」;
//! docs/boundary-api.md, 「The voice unit」): a briefing launched through the voice unit for a room, the line a
//! briefing awaits handed over once the round sent it, and the briefings a round lets go when it is over. What a
//! briefing speaks is either a notice or a summary of a room; the voice unit runs the making of it, so nothing
//! here names a runner.

use std::sync::LazyLock;

use regex::Regex;

use receiver_policy::chat_round::CHAT_ROUND;

use receiver_ports::voice::{Briefing, BriefingKind};

use super::conversation::NoticeTurn;
use crate::js::trim;
use crate::receiver::Receiver;
use crate::store::settings;
use crate::{all, one};

/// What the launcher tells whoever called it: the whole answer, so a caller lets go of it and does not say the
/// briefing in words as well.
pub fn launched(unit: &str, chat_id: i64) -> String {
  format!(
    "voice briefing launched as {unit}; it speaks to chat {chat_id} by itself when it is done. Do not wait for it and do not say the briefing in text as well."
  )
}

static SENTENCE_END: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"([.?!…])\s+|\n+").expect("a sentence's end"));

// How short a first sentence is for the second to join it.
const SHORT_OPENING: usize = 40;

/// The opening of a line, which is what is spoken for a notice turn whose session launched no summary: its first
/// sentence, or its first two where the first is short.
pub fn line_opening(text: &str) -> String {
  let mut sentences: Vec<String> = vec![];
  let mut from = 0;

  for found in SENTENCE_END.captures_iter(text) {
    let whole = found.get(0).expect("a match");
    let end = found.get(1).map_or(whole.start(), |mark| mark.end());

    sentences.push(text[from..end].to_owned());
    from = whole.end();
  }

  sentences.push(text[from..].to_owned());

  let sentences: Vec<String> =
    sentences.iter().map(|one| trim(one).to_owned()).filter(|one| !one.is_empty()).collect();
  let first = sentences.first().cloned().unwrap_or_default();

  match sentences.get(1) {
    Some(second) if first.chars().count() < SHORT_OPENING => format!("{first} {second}"),
    _ => first.to_owned(),
  }
}

/// Whether every briefing is refused now: the owner has the switch off.
pub async fn switched_off(receiver: &Receiver) -> bool {
  settings::get(receiver.db(), "voice_briefing").await.ok().flatten().as_deref() == Some("off")
}

/// Whether the chat agent's answers may be spoken at all: its row names a voice.
pub async fn voiced(receiver: &Receiver, agent: &str) -> bool {
  let listed = receiver.units().agency.agents().await.ok();

  listed
    .as_ref()
    .and_then(|listed| listed.agents.iter().find(|one| one.name == agent))
    .is_some_and(|found| found.row.get("voice").is_some_and(|held| !held.is_null()))
}

/// Why the turn's opening is not spoken in place of a summary, or none where it is.
pub async fn notice_skip(receiver: &Receiver, turn: &NoticeTurn) -> Option<String> {
  if switched_off(receiver).await {
    return Some("the voice briefing is switched off".to_owned());
  }

  if !voiced(receiver, &receiver.deployment().agent).await {
    return Some("the chat agent has no voice".to_owned());
  }

  let briefings = receiver.units().voice.briefings(turn.chat_id, false).await.ok().unwrap_or_default();

  if !briefings.is_empty() {
    return Some("its session already spoke a briefing".to_owned());
  }

  let failed = CHAT_ROUND.voice.failed_kind.clone();
  let kinds = one(
    receiver.db(),
    r#"SELECT "kind" FROM "instructions" WHERE "chat_id" = $1 AND "kind" = $2 AND "delivered_at" IS NULL AND "handed_at" IS NULL"#,
    vec![turn.chat_id.into(), failed.into()],
  )
    .await
    .ok().flatten();

  if kinds.is_some() {
    return Some("it tells of a voice briefing that failed".to_owned());
  }

  None
}

/// A notice turn of the owner's chat whose answer went out: the summaries its session launched tied to the line it
/// sent, or, where it launched none, the line's opening launched in their place.
pub async fn brief_notice_turn(receiver: &Receiver, turn: &NoticeTurn) -> Option<String> {
  if turn.chat_id != receiver.deployment().owner_id || notice_skip(receiver, turn).await.is_some() {
    return None;
  }

  let opening = line_opening(&turn.text);

  if opening.is_empty() {
    return None;
  }

  let briefing = Briefing {
    chat: turn.chat_id,
    message: Some(turn.message_id),
    kind: BriefingKind::Notice,
    title: CHAT_ROUND.voice.untitled.clone(),
    awaits_line: false,
    text: opening,
  };

  receiver.units().voice.brief(&briefing).await.ok()
}

/// The briefings of a room still awaiting a line, handed the line they await once the round sent it: the voice
/// message the room wrote, where one stands.
pub async fn let_go(receiver: &Receiver, chat_id: i64) {
  let awaiting = receiver.units().voice.briefings(chat_id, true).await.ok().unwrap_or_default();

  for briefed in awaiting.iter().filter(|briefed| !briefed.finished) {
    if let Some(message) = briefed.voice_message {
      let _ = receiver.units().voice.line(&briefed.run, message).await;
    }
  }
}

/// The voice briefings a round lets go once it is over: the room's own, or every room where none is named.
pub async fn let_go_all(receiver: &Receiver, chat_id: Option<i64>) {
  match chat_id {
    Some(chat_id) => let_go(receiver, chat_id).await,
    None => {
      let rooms = all(receiver.db(), r#"SELECT DISTINCT "chat_id" FROM "messages""#, vec![])
        .await
        .unwrap_or_default();

      for row in rooms {
        let chat_id = row.try_get::<i64>("", "chat_id").ok().unwrap_or(0);

        let_go(receiver, chat_id).await;
      }
    }
  }
}

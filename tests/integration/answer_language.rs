//! The answer language a turn names (`docs/answer-checks.md`, 「The language an answer goes out in」): the
//! language of the message that addresses the agent, read afresh each turn, and the room's configured language
//! only where that message shows none.

use bmo_receiver::chat::prompt::{addressing, answer_language_note};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::Inbound;
use bmo_receiver::wording::text_language;
use tempfile::tempdir;

use crate::common::Scratch;
use crate::common::units::{deployment, unrun};

const OWN: i64 = 90;

fn said(message_id: i64, sender: i64, text: &str) -> Inbound {
  Inbound {
    chat_id: -5,
    message_id,
    sender_id: Some(sender),
    sender_name: "someone".into(),
    sent_at: message_id,
    text: text.into(),
    image_path: None,
    reply_to_message_id: None,
    via: "road-a".into(),
    mode: None,
    via_reader: None,
    album: None,
  }
}

#[tokio::test]
async fn the_turn_names_the_language_of_the_message_that_addresses_the_agent() {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let receiver = unrun(scratch.db(), DataRoot::new(data.path()), vec![], deployment()).receiver;
  let korean = std::fs::read_to_string("tests/fixtures/addressing-korean.txt").unwrap();
  let korean = said(1, 2, korean.trim());
  let english = said(2, 3, "What time is the meeting tomorrow?");
  let room = &receiver.deployment().language;

  assert_eq!(text_language(&korean.text), Some("ko"));
  assert_eq!(text_language(&english.text), Some("en"));
  assert_eq!(text_language("12:30 — 42"), None);

  // A Korean message addressing the agent names Korean, an English one English, whatever else the page holds.
  let note = answer_language_note(
    &receiver,
    addressing(std::slice::from_ref(&korean), std::slice::from_ref(&english), None),
  );

  assert!(
    note.starts_with("ANSWER LANGUAGE: ko — the language of the message addressed to you"),
    "{note}"
  );

  let note = answer_language_note(
    &receiver,
    addressing(std::slice::from_ref(&english), std::slice::from_ref(&korean), None),
  );

  assert!(
    note.starts_with("ANSWER LANGUAGE: en — the language of the message addressed to you"),
    "{note}"
  );

  // With no message of the turn's own, the newest on the page that is not the agent's own addresses it.
  let page = [korean.clone(), said(3, OWN, "An answer of the agent's own.")];

  assert_eq!(addressing(&[], &page, Some(OWN)).map(|one| one.message_id), Some(1));
  assert!(
    answer_language_note(&receiver, addressing(&[], &page, Some(OWN))).starts_with("ANSWER LANGUAGE: ko")
  );

  // Where no message tells a language, the room's configured one stands in.
  let numbers = said(4, 2, "12:30");
  let note = answer_language_note(&receiver, addressing(std::slice::from_ref(&numbers), &[], None));

  assert!(note.starts_with(&format!("ANSWER LANGUAGE: {room} — this room's language")), "{note}");
  assert!(
    answer_language_note(&receiver, None).starts_with(&format!("ANSWER LANGUAGE: {room} — this room's"))
  );
}

//! The answer language, a person's and never a turn's (`docs/answer-checks.md`, 「The language an answer goes out
//! in」): each person's language read off their own prose in the room and remembered there, a message with no prose
//! never changing it, the turn's line naming each person it answers, and an answer held to the languages of the
//! people it answers.

use bmo_receiver::chat::draft::checks::{Check, Reading, errand, failing};
use bmo_receiver::chat::languages::{Addressee, addressees, clauses, held_to, line, people, remembered};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::language::{prose_language, text_language};
use bmo_receiver::receiver::Inbound;
use tempfile::tempdir;

use crate::common::Scratch;
use crate::common::units::{deployment, unrun};

const ROOM: i64 = -5;
const OWN: i64 = 90;
const OWNER: i64 = 1;

fn said(message_id: i64, sender: i64, name: &str, text: &str) -> Inbound {
  Inbound {
    chat_id: ROOM,
    message_id,
    sender_id: Some(sender),
    sender_name: name.into(),
    sent_at: message_id,
    text: text.into(),
    image_path: None,
    transcript: None,
    voice_seconds: None,
    voice_state: None,
    reply_to_message_id: None,
    via: "road-a".into(),
    mode: None,
    via_reader: None,
    album: None,
  }
}

fn fixture(name: &str) -> String {
  std::fs::read_to_string(format!("tests/fixtures/{name}")).unwrap().trim().to_owned()
}

#[test]
fn only_prose_names_its_writers_language() {
  assert_eq!(prose_language(&fixture("addressing-korean.txt")), Some("ko"));
  assert_eq!(prose_language("What time is the meeting tomorrow?"), Some("en"));

  for no_prose in [
    "[File taskNode.7z]",
    "https://example.com/a/long/path/to/a/page",
    "```\nlet value = compute(input);\n```",
    "12:30 — 42",
    "\u{1F600}\u{1F44D}",
    "ok",
  ] {
    assert_eq!(prose_language(no_prose), None, "{no_prose}");
  }

  // A placeholder's own Latin letters weigh nothing beside a caption written in another language.
  assert_eq!(
    prose_language(&format!("[File taskNode.7z] {}", fixture("addressing-korean.txt"))),
    Some("ko")
  );
  assert_eq!(
    text_language("[File taskNode.7z]"),
    Some("en"),
    "the bare script reading is no person's language"
  );
}

#[tokio::test]
async fn a_file_message_does_not_change_a_persons_language() {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let receiver = unrun(scratch.db(), DataRoot::new(data.path()), vec![], deployment()).receiver;
  let written = said(1, 2, "person-a", &fixture("addressing-korean.txt"));
  let file = said(2, 2, "person-a", "[File taskNode.7z]");

  // The person's Korean prose on the page names their language, and the file they send after it does not move it.
  let found = addressees(
    &receiver,
    ROOM,
    OWNER,
    std::slice::from_ref(&file),
    std::slice::from_ref(&written),
    Some(OWN),
  )
  .await
  .unwrap();

  assert_eq!(
    found,
    vec![Addressee {
      person: 2,
      name: "person-a".into(),
      language: "ko".into()
    }]
  );
  assert_eq!(remembered(&receiver, ROOM, 2).await.unwrap().as_deref(), Some("ko"));

  // Once the prose is off the page, the room remembers it: a file alone still answers in Korean.
  let found = addressees(&receiver, ROOM, OWNER, std::slice::from_ref(&file), &[], Some(OWN))
    .await
    .unwrap();

  assert_eq!(found[0].language, "ko");

  // A person who has written no prose yet stands on the deployment's language.
  let stranger = said(3, 3, "person-b", "[Photo]");
  let found = addressees(&receiver, ROOM, OWNER, std::slice::from_ref(&stranger), &[], Some(OWN))
    .await
    .unwrap();

  assert_eq!(found[0].language, receiver.deployment().language);
}

#[tokio::test]
async fn two_people_of_two_languages_get_two_clauses_and_an_answer_to_each_in_their_own_passes() {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let receiver = unrun(scratch.db(), DataRoot::new(data.path()), vec![], deployment()).receiver;
  let batch = [
    said(1, 2, "person-a", &fixture("addressing-korean.txt")),
    said(2, 3, "person-b", "What time is the meeting tomorrow?"),
    said(3, OWN, "the agent", "An earlier answer of the agent's own."),
  ];

  assert_eq!(
    people(ROOM == OWNER, OWNER, &batch, &[], Some(OWN)),
    vec![2, 3],
    "the agent's own words answer nobody"
  );

  let found = addressees(&receiver, ROOM, OWNER, &batch, &[], Some(OWN)).await.unwrap();
  let named = line(&receiver, &found);

  assert_eq!(clauses(&found), "person-a: ko; person-b: en");
  assert!(
    named.starts_with("ANSWER LANGUAGE — ") && named.ends_with(": person-a: ko; person-b: en."),
    "{named}"
  );
  assert_eq!(named.lines().count(), 1);

  let languages = held_to(&receiver, &found);
  let people = clauses(&found);
  let reading = Reading {
    languages: &languages,
    people: &people,
    language: true,
    own_lines: &[],
    max_chars: 4_000,
  };
  let korean_answer = fixture("answer-korean.txt");
  let english_answer = "The meeting starts at three in the afternoon, in the main hall.";
  let japanese_answer = &fixture("answer-japanese.txt");

  assert_eq!(failing(&format!("{korean_answer}\n\n{english_answer}"), &reading), vec![]);
  assert_eq!(
    failing(&korean_answer, &reading),
    vec![],
    "an answer holding one of their languages passes"
  );
  assert_eq!(failing(japanese_answer, &reading), vec![Check::Language]);
  assert!(errand(Check::Language, japanese_answer, &reading).contains("person-a: ko; person-b: en"));
}

#[tokio::test]
async fn one_person_is_one_clause_and_a_korean_answer_to_a_korean_speaker_passes() {
  let scratch = Scratch::new().await;
  let data = tempdir().unwrap();
  let receiver = unrun(scratch.db(), DataRoot::new(data.path()), vec![], deployment()).receiver;
  let batch = [said(1, 2, "person-a", &fixture("addressing-korean.txt"))];
  let found = addressees(&receiver, ROOM, OWNER, &batch, &[], Some(OWN)).await.unwrap();
  let languages = held_to(&receiver, &found);
  let people = clauses(&found);
  let reading = Reading {
    languages: &languages,
    people: &people,
    language: true,
    own_lines: &[],
    max_chars: 4_000,
  };

  assert_eq!(people, "person-a: ko");
  assert_eq!(failing(&fixture("answer-korean.txt"), &reading), vec![]);
  assert_eq!(
    failing("The meeting starts at three in the afternoon, in the main hall.", &reading),
    vec![Check::Language]
  );

  // A turn answering nobody names the room's language.
  assert!(
    line(&receiver, &[])
      .starts_with(&format!("ANSWER LANGUAGE: {} — this room's", receiver.deployment().language))
  );
}

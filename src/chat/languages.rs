//! The language each person a turn answers writes in (docs/answer-checks.md, 「The language an answer goes out
//! in」): a person's and never a turn's, read off their own newest prose in the room and remembered for the room
//! and the person, the deployment's standing in where they have written none; the line that names each, and
//! what the language check holds an answer to.

use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::fill_marks;

use crate::language::prose_language;
use crate::receiver::{Inbound, Receiver};
use crate::{Fault, one, run};

/// A person a turn answers: who they are, what the room calls them, and the language they write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Addressee {
  pub person: i64,
  pub name: String,
  pub language: String,
}

/// The people a turn answers, in the order they first spoke: the speakers of its words, the agent's own aside; a
/// turn with no words of anybody's answers the owner in the owner's own chat, and elsewhere the newest speaker on
/// the page who is not the agent.
pub fn people(
  owners_room: bool,
  owner: i64,
  batch: &[Inbound],
  recent: &[Inbound],
  own: Option<i64>,
) -> Vec<i64> {
  let mut people: Vec<i64> = vec![];

  for person in batch
    .iter()
    .filter_map(|message| message.sender_id)
    .filter(|sender| Some(*sender) != own)
  {
    if !people.contains(&person) {
      people.push(person);
    }
  }

  if people.is_empty() {
    if owners_room {
      people.push(owner);
    } else if let Some(person) = recent
      .iter()
      .rev()
      .filter_map(|message| message.sender_id)
      .find(|sender| Some(*sender) != own)
    {
      people.push(person);
    }
  }

  people
}

/// The language a person's own messages here name, newest first, and the message that names it; none where no
/// message of theirs holds prose.
pub fn written<'a>(
  person: i64,
  messages: impl Iterator<Item = &'a Inbound>,
) -> Option<(&'static str, i64)> {
  let mut theirs: Vec<&Inbound> = messages.filter(|message| message.sender_id == Some(person)).collect();

  theirs.sort_by_key(|message| (message.sent_at, message.message_id));
  theirs
    .iter()
    .rev()
    .find_map(|message| prose_language(&message.text).map(|language| (language, message.message_id)))
}

/// The language remembered for a person in a room, where one is.
pub async fn remembered(receiver: &Receiver, chat_id: i64, person: i64) -> Result<Option<String>, Fault> {
  let row = one(
    receiver.db(),
    r#"SELECT "language" FROM "people_languages" WHERE "chat_id" = $1 AND "user_id" = $2"#,
    vec![chat_id.into(), person.into()],
  )
  .await?;

  Ok(row.map(|row| row.try_get::<String>("", "language")).transpose()?)
}

/// A person's language in a room remembered, read off the message named; a message older than the one it was
/// read off before leaves it as it stands.
pub async fn remember(
  receiver: &Receiver,
  chat_id: i64,
  person: i64,
  language: &str,
  message_id: i64,
) -> Result<(), Fault> {
  run(
    receiver.db(),
    r#"INSERT INTO "people_languages" ("chat_id", "user_id", "language", "message_id", "changed_at")
       VALUES ($1, $2, $3, $4, $5)
       ON CONFLICT ("chat_id", "user_id") DO UPDATE
         SET "language" = excluded."language", "message_id" = excluded."message_id",
             "changed_at" = excluded."changed_at"
         WHERE "people_languages"."message_id" <= excluded."message_id""#,
    vec![
      chat_id.into(),
      person.into(),
      language.into(),
      message_id.into(),
      crate::now_ms().into(),
    ],
  )
  .await
  .map(drop)
}

/// The people a turn answers with the language each writes: read off their own newest prose on the turn's page
/// or in its words and remembered, else the language remembered for them, else the deployment's.
pub async fn addressees(
  receiver: &Receiver,
  chat_id: i64,
  owner: i64,
  batch: &[Inbound],
  recent: &[Inbound],
  own: Option<i64>,
) -> Result<Vec<Addressee>, Fault> {
  let mut found = vec![];

  for person in people(chat_id == receiver.owner_room(), owner, batch, recent, own) {
    let language = match written(person, batch.iter().chain(recent)) {
      Some((language, message_id)) => {
        remember(receiver, chat_id, person, language, message_id).await?;
        language.to_owned()
      }
      None => match remembered(receiver, chat_id, person).await? {
        Some(language) => language,
        None => receiver.deployment().language.clone(),
      },
    };
    let name = batch
      .iter()
      .chain(recent)
      .filter(|message| message.sender_id == Some(person) && !message.sender_name.trim().is_empty())
      .max_by_key(|message| (message.sent_at, message.message_id))
      .map_or_else(|| CHAT_ROUND.closing.owner.clone(), |message| message.sender_name.trim().to_owned());

    found.push(Addressee {
      person,
      name,
      language,
    });
  }

  Ok(found)
}

/// Each person and their language, one clause a person, as the line and the language check's errand name them.
pub fn clauses(addressees: &[Addressee]) -> String {
  let closing = &CHAT_ROUND.closing;

  addressees
    .iter()
    .map(|one| fill_marks(&closing.language_person, &[("name", &one.name), ("language", &one.language)]))
    .collect::<Vec<_>>()
    .join(&closing.language_joint)
}

/// The turn's answer language line: each person it answers with their language, or the room's where it answers
/// nobody.
pub fn line(receiver: &Receiver, addressees: &[Addressee]) -> String {
  let closing = &CHAT_ROUND.closing;

  if addressees.is_empty() {
    fill_marks(&closing.language_room, &[("language", &receiver.deployment().language)])
  } else {
    fill_marks(&closing.language, &[("people", &clauses(addressees))])
  }
}

/// The languages an answer is held to: those of the people it answers, each once, or the deployment's where it
/// answers nobody.
pub fn held_to(receiver: &Receiver, addressees: &[Addressee]) -> Vec<String> {
  let mut languages: Vec<String> = vec![];

  for one in addressees {
    if !languages.contains(&one.language) {
      languages.push(one.language.clone());
    }
  }

  if languages.is_empty() {
    languages.push(receiver.deployment().language.clone());
  }

  languages
}

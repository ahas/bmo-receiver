//! A room's chat providers (docs/receiver.md, 「A room's chat providers」): which of the roads a room is on, as one
//! setting of its registration, and every message kept in the room carried to each of them it does not already
//! stand on. The room's record is a provider like any other and holds every kept message by being the archive;
//! each chat service on the list is handed the message through its own `POST /sends`, tagged, and the number it
//! gave its copy is kept, so its echo is known for the message already kept. Nothing here names a provider.

use std::path::Path;

use bmo_chat_client::{AlbumItem, ChatSend, ChatSendRequest, ChatVoice, ClientFailure, FileItem};
use sea_orm::{DatabaseConnection, TransactionTrait};

use super::{Road, Roads};
use crate::{Fault, all, now_ms, one, run};

/// A provider a room may be on: its key and what it is called.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Provider {
  pub key: String,
  pub label: String,
}

/// Every provider a room may be on: the room's record, then each chat service the configuration lists.
pub async fn every(roads: &Roads) -> Vec<Provider> {
  let mut providers = vec![];

  for road in
    std::iter::once(Road::Record).chain(roads.chats().names().map(|key| Road::Service(key.to_owned())))
  {
    let facts = roads.facts(&road).await;

    providers.push(Provider {
      key: road.key().to_owned(),
      label: facts.label,
    });
  }

  providers
}

/// Whether a key names a provider a room may be on.
pub fn stands(roads: &Roads, key: &str) -> bool {
  key == Road::Record.key() || roads.chats().road(key).is_some()
}

/// A room's providers in order, and whether the list was set rather than read off what the room holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
  pub providers: Vec<String>,
  pub chosen: bool,
}

/// The providers a room is on: the list set for it, or, where none was, every provider that handed it a message
/// or kept one in it, in the order each first did, and the chat service that says the room is its own.
pub async fn listed(db: &DatabaseConnection, roads: &Roads, chat_id: i64) -> Result<Listed, Fault> {
  let chosen: Vec<String> = all(
    db,
    r#"SELECT "provider" FROM "room_providers" WHERE "chat_id" = $1 ORDER BY "place""#,
    vec![chat_id.into()],
  )
  .await?
  .iter()
  .map(|row| row.try_get::<String>("", "provider"))
  .collect::<Result<_, _>>()?;

  if !chosen.is_empty() {
    return Ok(Listed {
      providers: chosen,
      chosen: true,
    });
  }

  let held: Vec<String> = all(
    db,
    r#"SELECT "via" FROM "messages" WHERE "chat_id" = $1 AND "via" IS NOT NULL
       GROUP BY "via" ORDER BY MIN("rowid")"#,
    vec![chat_id.into()],
  )
  .await?
  .iter()
  .map(|row| row.try_get::<String>("", "via"))
  .collect::<Result<_, _>>()?;

  let mut providers: Vec<String> = held.into_iter().filter(|key| stands(roads, key)).collect();

  if let Some(own) = roads.room_road(chat_id).await
    && !providers.contains(&own)
  {
    providers.push(own);
  }

  Ok(Listed {
    providers,
    chosen: false,
  })
}

/// A room's list of providers set whole, in the order given.
pub async fn choose(db: &DatabaseConnection, chat_id: i64, providers: &[String]) -> Result<(), Fault> {
  let transaction = db.begin().await.map_err(Fault::from)?;

  run(
    &transaction,
    r#"DELETE FROM "room_providers" WHERE "chat_id" = $1"#,
    vec![chat_id.into()],
  )
  .await?;

  for (place, provider) in providers.iter().enumerate() {
    run(
      &transaction,
      r#"INSERT INTO "room_providers" ("chat_id", "provider", "place") VALUES ($1, $2, $3)"#,
      vec![chat_id.into(), provider.clone().into(), i64::try_from(place).unwrap_or(i64::MAX).into()],
    )
    .await?;
  }

  transaction.commit().await.map_err(Fault::from)
}

/// What a kept message carries to another provider: one send as it stands, or the files it carried with the words
/// typed beside them.
#[derive(Clone, Debug)]
pub enum Carried {
  Send(ChatSend),
  Files {
    files: Vec<FileItem>,
    caption: String,
  },
}

impl Carried {
  /// Words, as a person or an agent said them, folded where the provider's dialect can.
  pub fn words(text: &str) -> Self {
    Self::Send(ChatSend::Text {
      text: text.to_owned(),
      silent: false,
      fold: true,
    })
  }
}

/// One kept message on its way to the room's providers: the room, the number it is kept under, the message it
/// answers, whether the owner said it, the providers it already stands on, and what it carries.
#[derive(Clone, Debug)]
pub struct Delivery {
  pub chat_id: i64,
  pub message_id: i64,
  pub reply_to: Option<i64>,
  pub owner: bool,
  pub standing: Vec<String>,
  pub carried: Carried,
}

/// What a file is to a road.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
  Picture,
  Video,
  Sound,
  Document,
}

/// A file's kind: by its type where one was given, and by the extension of its name where it was not.
pub fn kind_of(name: &str, mime: &str) -> Kind {
  let given = mime.trim().to_lowercase();
  let named = Path::new(name)
    .extension()
    .map(|held| held.to_string_lossy().into_owned())
    .unwrap_or_default()
    .to_lowercase();
  let guessed = if given.is_empty() { named } else { given };

  match guessed.as_str() {
    "mp4" | "mov" | "webm" | "avi" | "mkv" => Kind::Video,
    "mp3" | "wav" | "ogg" | "m4a" | "flac" | "aac" => Kind::Sound,
    "jpg" | "jpeg" | "png" | "gif" | "webp" | "heic" | "heif" | "tif" | "tiff" | "bmp" => Kind::Picture,
    other if other.starts_with("video/") => Kind::Video,
    other if other.starts_with("audio/") => Kind::Sound,
    other if other.starts_with("image/") => Kind::Picture,
    _ => Kind::Document,
  }
}

// Which files stand together in one album: pictures and videos, sounds; a document stands as a send of its own.
fn album_of(kind: Kind) -> Option<u8> {
  match kind {
    Kind::Picture | Kind::Video => Some(0),
    Kind::Sound => Some(1),
    Kind::Document => None,
  }
}

/// The sends files go as, the way a phone sends several at once: the pictures and videos as one album, the sounds
/// as another and each document on its own, in the order the first of each was given. The caption stands on the
/// first send where that send takes one and the caption fits under `limit`; otherwise it goes ahead as words.
pub fn file_sends(files: &[FileItem], caption: &str, limit: Option<u64>) -> Vec<ChatSend> {
  let mut groups: Vec<(Option<u8>, Vec<FileItem>)> = vec![];

  for file in files {
    let album = album_of(kind_of(&file.name, &file.mime));

    match groups.iter().position(|(held, _)| album.is_some() && *held == album) {
      Some(at) => groups[at].1.push(file.clone()),
      None => groups.push((album, vec![file.clone()])),
    }
  }

  let fits =
    limit.is_none_or(|limit| u64::try_from(caption.encode_utf16().count()).unwrap_or(u64::MAX) <= limit);
  let first_takes = groups.first().is_some_and(|(album, _)| album.is_some());
  let ahead = !caption.trim().is_empty() && !(fits && first_takes);
  let mut sends: Vec<ChatSend> = vec![];

  if ahead {
    sends.push(ChatSend::Text {
      text: caption.to_owned(),
      silent: false,
      fold: false,
    });
  }

  for (at, (album, held)) in groups.into_iter().enumerate() {
    let words = (at == 0 && !ahead && !caption.trim().is_empty()).then(|| caption.to_owned());

    match album {
      Some(_) => sends.push(ChatSend::Album {
        items: held.into_iter().map(AlbumItem::File).collect(),
        caption: words,
      }),
      None => sends.extend(held.into_iter().map(|file| ChatSend::Document {
        bytes: file.bytes,
        file_name: file.name,
      })),
    }
  }

  sends
}

// The number a message stands under on a provider, where it stands there: its own where it came in on that
// provider, and the one the provider gave the copy carried to it otherwise.
async fn number_on(db: &DatabaseConnection, chat_id: i64, message_id: i64, key: &str) -> Option<i64> {
  let via = one(
    db,
    r#"SELECT "via" FROM "messages" WHERE "chat_id" = $1 AND "message_id" = $2"#,
    vec![chat_id.into(), message_id.into()],
  )
  .await
  .ok()
  .flatten()
  .and_then(|row| row.try_get::<Option<String>>("", "via").ok().flatten());

  if via.as_deref() == Some(key) {
    return Some(message_id);
  }

  one(
    db,
    r#"SELECT "provider_message_id" FROM "room_deliveries" WHERE "chat_id" = $1 AND "message_id" = $2
       AND "provider" = $3 AND "provider_message_id" IS NOT NULL ORDER BY "id" LIMIT 1"#,
    vec![chat_id.into(), message_id.into(), key.into()],
  )
  .await
  .ok()
  .flatten()
  .and_then(|row| row.try_get::<i64>("", "provider_message_id").ok())
}

/// A copy a provider already holds of a kept message, kept beside it: the send that carried the message there
/// went out some other way, under the number the provider gave it where it gave one.
pub async fn stood(
  db: &DatabaseConnection,
  chat_id: i64,
  message_id: i64,
  key: &str,
  copy: Option<i64>,
) -> Result<i64, Fault> {
  let row = one(
    db,
    r#"INSERT INTO "room_deliveries" ("chat_id", "message_id", "provider", "provider_message_id", "delivered_at")
       VALUES ($1, $2, $3, $4, $5) RETURNING "id""#,
    vec![chat_id.into(), message_id.into(), key.into(), copy.into(), now_ms().into()],
  )
  .await?;

  Ok(row.map(|row| row.try_get::<i64>("", "id")).transpose()?.unwrap_or_default())
}

/// Whether a message a provider hands over is its copy of a message the room already keeps: one whose tag names a
/// send carried to that provider in that room, or one under the number that provider gave such a send.
pub async fn echo(
  db: &DatabaseConnection,
  chat_id: i64,
  key: &str,
  message_id: i64,
  tag: Option<&str>,
) -> Result<bool, Fault> {
  if let Some(tag) = tag.and_then(|tag| tag.trim().parse::<i64>().ok())
    && one(
      db,
      r#"SELECT 1 AS "one" FROM "room_deliveries" WHERE "id" = $1 AND "chat_id" = $2 AND "provider" = $3"#,
      vec![tag.into(), chat_id.into(), key.into()],
    )
    .await?
    .is_some()
  {
    return Ok(true);
  }

  Ok(
    one(
      db,
      r#"SELECT 1 AS "one" FROM "room_deliveries" WHERE "chat_id" = $1 AND "provider" = $2
         AND "provider_message_id" = $3 LIMIT 1"#,
      vec![chat_id.into(), key.into(), message_id.into()],
    )
    .await?
    .is_some(),
  )
}

/// A kept message carried to each provider of its room it does not already stand on. The record holds it by being
/// the archive; each chat service is handed it through its own sends, in the owner's own voice where the owner
/// said it, each send tagged and the number its copy went out as kept. A provider that refuses or does not answer
/// is said in the journal and leaves the message as it was kept.
pub async fn deliver(db: &DatabaseConnection, roads: &Roads, delivery: &Delivery) {
  let chat_id = delivery.chat_id;
  let listed = match listed(db, roads, chat_id).await {
    Ok(listed) => listed,
    Err(why) => {
      eprintln!("chat {chat_id}: the room's providers could not be read: {}", why.detail);

      return;
    }
  };

  for key in listed.providers {
    if key == Road::Record.key() || delivery.standing.contains(&key) {
      continue;
    }

    let Some(service) = roads.chats().road(&key) else { continue };
    let reply = match delivery.reply_to {
      Some(target) => number_on(db, chat_id, target, &key).await,
      None => None,
    };
    let sends = match &delivery.carried {
      Carried::Send(send) => vec![send.clone()],
      Carried::Files { files, caption } => {
        let limit = roads.declaration(&key).await.and_then(|declared| declared.dialect.limits.caption);

        file_sends(files, caption, limit)
      }
    };

    for (at, send) in sends.into_iter().enumerate() {
      let tag = match stood(db, chat_id, delivery.message_id, &key, None).await {
        Ok(tag) => tag,
        Err(why) => {
          eprintln!(
            "chat {chat_id}: message {} was not carried to {key}: {}",
            delivery.message_id, why.detail
          );

          break;
        }
      };
      let mut request = ChatSendRequest::new(chat_id, send);

      request.mode = delivery.owner.then_some(ChatVoice::HumanReader);
      request.tag = Some(tag.to_string());
      request.reply_to_message_id = if at == 0 { reply } else { None };

      match service.send(&request).await {
        Ok(sent) => {
          let kept = run(
            db,
            r#"UPDATE "room_deliveries" SET "provider_message_id" = $1 WHERE "id" = $2"#,
            vec![sent.message_id.into(), tag.into()],
          )
          .await;

          if let Err(why) = kept {
            eprintln!("chat {chat_id}: the number {key} gave its copy was not kept: {}", why.detail);
          }
        }
        Err(ClientFailure::Answered {
          answer: Some(answer),
          ..
        }) => eprintln!("chat {chat_id}: {key} refused message {}: {}", delivery.message_id, answer.error),
        Err(why) => eprintln!("chat {chat_id}: {key} did not take message {}: {why}", delivery.message_id),
      }
    }
  }
}

//! A room's chat providers (docs/receiver.md, 「A room's chat providers」): which of the roads a room is on, as one
//! setting of its registration, and every message kept in the room carried to each of them it does not already
//! stand on. The room's record is a provider like any other and holds every kept message by being the archive;
//! each chat service on the list is handed the message through its own `POST /sends`, tagged, and the number it
//! gave its copy is kept, so its echo is known for the message already kept. Nothing here names a provider.

use std::path::Path;

use bmo_chat_client::{
  AlbumItem, ChatCapability, ChatDeclaration, ChatModeStanding, ChatSend, ChatSendRequest, ChatVoice,
  ClientFailure, ErrorCode, FileBody, FileItem,
};
use receiver_codes::OwnerSendRefusal;
use receiver_policy::fill_marks;
use receiver_policy::roads::ROADS;
use receiver_ports::chat::Chat;
use sea_orm::{ConnectionTrait, DatabaseConnection, TransactionTrait};

use super::added::{self, RoomMode};
use super::{Facts, Road, Roads};
use crate::{Fault, all, now_ms, one, run};

/// A provider a room may be on: its key, what it is called, the verbs it performs, whether it lists the rooms and the
/// contacts of its account and starts a one-to-one or a group, the modes a room added on it may take (docs/receiver.md,
/// 「Adding a room」), and the largest file it takes, where it declares one (docs/api.md, 「Sends」).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Provider {
  pub key: String,
  pub label: String,
  pub can: Vec<String>,
  pub lists_rooms: bool,
  pub lists_contacts: bool,
  pub starts_chats: bool,
  pub starts_groups: bool,
  pub room_modes: Vec<RoomMode>,
  pub file_bytes: Option<u64>,
  /// The app registration a person's own account needs on it, as it declares one.
  pub registration: Option<bmo_chat_client::ChatRegistration>,
}

/// Every provider a room may be on: the room's record, which lists no rooms and takes no mode, then each chat
/// service the configuration lists, as it declares itself now.
pub async fn every(roads: &Roads) -> Vec<Provider> {
  let record = roads.facts(&Road::Record).await;
  let mut providers = vec![Provider {
    key: Road::Record.key().to_owned(),
    label: record.label,
    can: record.can,
    lists_rooms: false,
    lists_contacts: false,
    starts_chats: false,
    starts_groups: false,
    room_modes: vec![],
    file_bytes: record.file_bytes,
    registration: None,
  }];

  for key in roads.chats().names() {
    let declared = roads.declaration_now(key).await;

    providers.push(Provider {
      key: key.to_owned(),
      label: declared.as_ref().map_or_else(|| key.to_owned(), |declared| declared.label.clone()),
      can: declared.as_ref().map(|declared| Facts::of(declared).can).unwrap_or_default(),
      lists_rooms: declared.as_ref().is_some_and(added::lists_rooms),
      lists_contacts: declared
        .as_ref()
        .is_some_and(|declared| added::can(declared, ChatCapability::ContactList)),
      starts_chats: declared
        .as_ref()
        .is_some_and(|declared| added::can(declared, ChatCapability::StartChat)),
      starts_groups: declared
        .as_ref()
        .is_some_and(|declared| added::can(declared, ChatCapability::StartGroup)),
      room_modes: declared.as_ref().map(added::offered).unwrap_or_default(),
      file_bytes: declared.as_ref().and_then(|declared| declared.dialect.limits.file_bytes),
      registration: declared.as_ref().and_then(|declared| declared.registration.clone()),
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

/// The largest file a room takes: the chat service on its list declaring the smallest limit, and that limit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileLimit {
  pub provider: String,
  pub bytes: u64,
}

impl FileLimit {
  /// Why a file of `size` bytes is turned away before anything is kept or goes: past the limit.
  pub fn turns_away(&self, name: &str, size: u64) -> Option<OwnerSendRefusal> {
    (size > self.bytes).then(|| OwnerSendRefusal::TooLarge {
      name: name.to_owned(),
      size_bytes: size,
      largest_bytes: self.bytes,
      provider: self.provider.clone(),
    })
  }
}

/// The largest file a room takes (docs/api.md, 「Sends」): the smallest any chat service on its list of providers
/// declares, as its kept declaration says; none where none of them declares one, so the room takes any size.
pub async fn file_limit(
  db: &DatabaseConnection,
  roads: &Roads,
  chat_id: i64,
) -> Result<Option<FileLimit>, Fault> {
  let listed = listed(db, roads, chat_id).await?;
  let mut limit: Option<FileLimit> = None;

  for key in listed.providers {
    let Some(bytes) = roads.facts(&Road::of(&key)).await.file_bytes else { continue };

    if limit.as_ref().is_none_or(|smallest| bytes < smallest.bytes) {
      limit = Some(FileLimit {
        provider: key,
        bytes,
      });
    }
  }

  Ok(limit)
}

/// Whether a kept message in a room goes on to any chat service: its list of providers names one the
/// configuration lists.
pub async fn carries(db: &DatabaseConnection, roads: &Roads, chat_id: i64) -> Result<bool, Fault> {
  let listed = listed(db, roads, chat_id).await?;

  Ok(
    listed
      .providers
      .iter()
      .any(|key| key != Road::Record.key() && roads.chats().road(key).is_some()),
  )
}

/// Whether a room is read-only (docs/receiver.md, 「A read-only room」): its list of providers names at least one
/// chat service, and every chat service on it reads the room in the owner's account alone. The record on the list
/// changes nothing either way.
pub async fn read_only(db: &DatabaseConnection, roads: &Roads, chat_id: i64) -> Result<bool, Fault> {
  let listed = listed(db, roads, chat_id).await?;
  let mut services = 0;

  for key in &listed.providers {
    if roads.chats().road(key).is_none() {
      continue;
    }

    if !roads.reads_only(key, chat_id).await {
      return Ok(false);
    }

    services += 1;
  }

  Ok(services > 0)
}

/// Whether the receiver asks the chat service under `key` to read the people's words of a room
/// (docs/receiver.md, 「A read-only room」): the room was added, in whatever mode, and it is still on that
/// service's list of providers.
pub async fn read_asked(
  db: &DatabaseConnection,
  roads: &Roads,
  chat_id: i64,
  key: &str,
) -> Result<bool, Fault> {
  if added::mode_of(db, chat_id).await?.is_none() {
    return Ok(false);
  }

  Ok(listed(db, roads, chat_id).await?.providers.iter().any(|listed| listed == key))
}

/// A room's list of providers set whole, in the order given.
pub async fn choose(db: &DatabaseConnection, chat_id: i64, providers: &[String]) -> Result<(), Fault> {
  let transaction = db.begin().await.map_err(Fault::from)?;

  write_list(&transaction, chat_id, providers).await?;
  transaction.commit().await.map_err(Fault::from)
}

/// A room's list of providers written whole, in the order given, on a connection the caller holds.
pub async fn write_list<C: ConnectionTrait>(
  conn: &C,
  chat_id: i64,
  providers: &[String],
) -> Result<(), Fault> {
  run(conn, r#"DELETE FROM "room_providers" WHERE "chat_id" = $1"#, vec![chat_id.into()]).await?;

  for (place, provider) in providers.iter().enumerate() {
    run(
      conn,
      r#"INSERT INTO "room_providers" ("chat_id", "provider", "place") VALUES ($1, $2, $3)"#,
      vec![chat_id.into(), provider.clone().into(), i64::try_from(place).unwrap_or(i64::MAX).into()],
    )
    .await?;
  }

  Ok(())
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
  /// One recording, kept in the room's record, that the room's other providers are handed as a voice message.
  Voice {
    audio: FileBody,
    caption: String,
  },
}

impl Carried {
  /// Words, as a person or an agent said them, unfolded: a fold is asked by the send that carries one
  /// (docs/wake.md, 「Folds」).
  pub fn words(text: &str) -> Self {
    Self::Send(ChatSend::Text {
      text: text.to_owned(),
      silent: false,
      fold: false,
    })
  }

  /// The same words with the speaker's shown name before them, as the policy writes a relay in a provider's own
  /// voice (docs/receiver.md, 「Console rooms and accounts」): the words of a send, or the caption of files and of
  /// a voice; a send carrying neither goes as it stands.
  pub fn named(&self, name: &str) -> Self {
    let named = |text: &str| fill_marks(&ROADS.relay.named, &[("name", name), ("text", text)]);

    match self {
      Self::Send(ChatSend::Text { text, silent, fold }) => Self::Send(ChatSend::Text {
        text: named(text),
        silent: *silent,
        fold: *fold,
      }),
      Self::Send(send) => Self::Send(send.clone()),
      Self::Files { files, caption } => Self::Files {
        files: files.clone(),
        caption: named(caption),
      },
      Self::Voice { audio, caption } => Self::Voice {
        audio: audio.clone(),
        caption: named(caption),
      },
    }
  }
}

/// Who said a kept message, as its carriage to a chat service reads it (docs/receiver.md, 「A room's chat
/// providers」, 「Console rooms and accounts」).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SaidBy {
  /// The owner, whose words go in his own voice.
  Owner,
  /// An account other than the owner's, in the console: its key and the name it is shown by.
  Account { account: i64, name: String },
  /// Anyone else: a person a chat service handed over, an agent, a line of the receiver's own.
  Room,
}

impl SaidBy {
  /// The owner where `owner` says the owner said it, and the room's voice otherwise.
  pub fn owner_or_room(owner: bool) -> Self {
    if owner { Self::Owner } else { Self::Room }
  }
}

/// One kept message on its way to the room's providers: the room, the number it is kept under, the message it
/// answers, who said it, the providers it already stands on, and what it carries.
#[derive(Clone, Debug)]
pub struct Delivery {
  pub chat_id: i64,
  pub message_id: i64,
  pub reply_to: Option<i64>,
  pub said_by: SaidBy,
  pub standing: Vec<String>,
  pub carried: Carried,
}

/// How a kept message goes to one chat service: the voice, the speaker and the account its sends name, and what
/// they carry.
struct Going {
  voice: Option<ChatVoice>,
  speaker: Option<String>,
  account: Option<i64>,
  carried: Carried,
}

/// The voice a provider speaks in as itself: a `bot-account` or `own-account` mode it declares active, never a
/// person's account; none where it declares neither.
fn own_voice(declared: &ChatDeclaration) -> Option<ChatVoice> {
  declared
    .modes
    .iter()
    .filter(|mode| mode.standing == ChatModeStanding::Active)
    .map(|mode| mode.voice)
    .find(|voice| matches!(voice, ChatVoice::BotAccount | ChatVoice::OwnAccount))
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

/// The kept message a provider's number names in a room, where it is the number that provider gave its copy of one:
/// a reply the provider hands over to its copy answers the kept message.
pub async fn copied(
  db: &DatabaseConnection,
  chat_id: i64,
  key: &str,
  number: i64,
) -> Result<Option<i64>, Fault> {
  let row = one(
    db,
    r#"SELECT "message_id" FROM "room_deliveries" WHERE "chat_id" = $1 AND "provider" = $2
       AND "provider_message_id" = $3 ORDER BY "id" LIMIT 1"#,
    vec![chat_id.into(), key.into(), number.into()],
  )
  .await?;

  Ok(row.map(|row| row.try_get::<i64>("", "message_id")).transpose()?)
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

/// How one of a room's providers took an act carried to it (docs/messages.md, 「What an agent can send back」).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Carriage {
  /// The act went there.
  Carried,
  /// The provider holds no number for the message, so it has no copy of the message to act on.
  NoCopy,
  /// The room's list names the provider, but no chat service stands under that key.
  NoService,
  /// The provider turned the act down, in the voice named and in the room's own where that was tried: `why` is
  /// the provider's own sentence.
  Refused(String),
}

impl Carriage {
  /// Why the act did not go, in the receiver's own words; nothing where it went.
  fn why_not_carried(&self) -> Option<String> {
    match self {
      Self::Carried => None,
      Self::NoCopy => Some("the service holds no copy of that message".to_owned()),
      Self::NoService => Some("no chat service stands under that provider".to_owned()),
      Self::Refused(why) => Some(why.clone()),
    }
  }
}

/// One of a room's providers and how it took an act carried to it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActCarriage {
  pub provider: String,
  pub took: Carriage,
}

/// An act on a kept message carried to each of the room's other providers where the message stands under a
/// number of the provider's own (docs/messages.md, 「What an agent can send back」): the act sent with that number,
/// in the voice named, the room's own where none is. A provider that refuses the act in the voice named is handed
/// it again in the voice it answers the room by, where that is another voice and the provider does not read the
/// room in the owner's account alone. Every provider the act was taken to, and every one that did not take it, is
/// answered, each provider that did not take it said in the log as not carried.
// The agent whose words a kept message is, none for a person's or a message the record does not hold.
async fn message_agent(db: &DatabaseConnection, chat_id: i64, message_id: i64) -> Option<String> {
  let rows = crate::all(
    db,
    r#"SELECT "agent_name" FROM "messages" WHERE "chat_id" = $1 AND "message_id" = $2 AND "agent_name" IS NOT NULL"#,
    vec![chat_id.into(), message_id.into()],
  )
  .await
  .ok()?;

  rows.first()?.try_get::<String>("", "agent_name").ok()
}

pub async fn carry_act(
  db: &DatabaseConnection,
  roads: &Roads,
  chat_id: i64,
  message_id: i64,
  act: &ChatSend,
  voice: Option<ChatVoice>,
  default_answerer: &str,
) -> Vec<ActCarriage> {
  let Ok(listed) = listed(db, roads, chat_id).await else {
    eprintln!(
      "chat {chat_id}: an act on message {message_id} was not carried to any provider: the room's providers could \
       not be read"
    );

    return vec![];
  };
  // The agent whose words the message is, none for a person's; and the default answerer, whose words go in the
  // service's own voice.
  let sender_agent = message_agent(db, chat_id, message_id).await;
  let mut taken: Vec<ActCarriage> = vec![];
  let mut name = |provider: &str, carriage: Carriage| {
    if let Some(why) = carriage.why_not_carried() {
      eprintln!("chat {chat_id}: an act on message {message_id} was not carried to {provider}: {why}");
    }

    taken.push(ActCarriage {
      provider: provider.to_owned(),
      took: carriage,
    });
  };

  for key in listed.providers {
    if key == Road::Record.key() {
      continue;
    }

    let Some(service) = roads.chats().road(&key) else {
      name(&key, Carriage::NoService);

      continue;
    };
    let Some(number) = number_on(db, chat_id, message_id, &key).await else {
      name(&key, Carriage::NoCopy);

      continue;
    };
    let send = match act.clone() {
      ChatSend::Reaction { emoji, .. } => ChatSend::Reaction {
        message_id: number,
        emoji,
      },
      ChatSend::Pin { on, silent, .. } => ChatSend::Pin {
        message_id: number,
        on,
        silent,
      },
      ChatSend::Edit { text, .. } => ChatSend::Edit {
        message_id: number,
        text,
      },
      ChatSend::Delete { .. } => ChatSend::Delete { message_id: number },
      _ => continue,
    };

    let mut request = ChatSendRequest::new(chat_id, send);

    request.mode = voice;

    // A message an agent sent is changed or taken down by the account that sent it, which alone may: the agent's own
    // account where it holds one, else the service's (docs/receiver.md, 「Acts on a message」).
    if let Some(agent) = &sender_agent
      && matches!(request.send, ChatSend::Edit { .. } | ChatSend::Delete { .. })
    {
      request.mode = None;
      request.speaker = (agent != default_answerer).then(|| agent.clone());
    }

    let refused = match roads.send(service.as_ref(), &request).await {
      Ok(_) => {
        name(&key, Carriage::Carried);

        continue;
      }
      Err(why @ ClientFailure::Answered { .. }) => why,
      Err(why) => {
        name(&key, Carriage::Refused(why.to_string()));

        continue;
      }
    };
    let own = added::voice_in(db, roads, &key, chat_id).await;

    if voice.is_none() || own == voice || roads.reads_only(&key, chat_id).await {
      name(&key, Carriage::Refused(refused.to_string()));

      continue;
    }

    println!(
      "chat {chat_id}: {key} refused an act on message {message_id} in the voice named, so it goes in the room's own: \
       {refused}"
    );
    request.mode = own;

    let went = roads.send(service.as_ref(), &request).await;

    name(
      &key,
      match went {
        Ok(_) => Carriage::Carried,
        Err(why) => Carriage::Refused(why.to_string()),
      },
    );
  }

  taken
}

/// A kept message carried to each provider of its room it does not already stand on. The record holds it by being
/// the archive; each chat service is handed it through its own sends, in the owner's own voice where the owner
/// said it, as another account's words go where an account said it in the console, and in the voice that service
/// answers the room by otherwise, each send tagged and the number its copy went out as kept. A service that reads
/// the room in the owner's account alone is handed the owner's words alone. A provider that refuses or does not
/// answer is said in the journal and leaves the message as it was kept.
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

    if delivery.said_by != SaidBy::Owner && roads.reads_only(&key, chat_id).await {
      println!(
        "chat {chat_id}: message {} is not carried to {key}, which reads the room in the owner's account alone",
        delivery.message_id
      );

      continue;
    }

    let Some(going) = going(db, roads, &key, delivery).await else { continue };
    let refused = carry(db, roads, service.as_ref(), &key, delivery, &going).await;

    // A provider that refuses the account as a speaker is handed the words again in its own voice with the
    // account's name before them.
    if refused
      && let SaidBy::Account { name, .. } = &delivery.said_by
      && going.speaker.is_some()
    {
      let named = Going {
        speaker: None,
        carried: delivery.carried.named(name),
        ..going
      };

      carry(db, roads, service.as_ref(), &key, delivery, &named).await;
    }
  }
}

// The voice, the speaker and the account one chat service is handed what someone said in: the owner's own voice
// for the owner; for another account, the provider's own voice and that account, never the room's leader's, naming
// the account as the speaker where the provider takes speakers, and nothing where it has no voice of its own; and
// the voice the service answers the room by for everyone else.
struct Voiced {
  voice: Option<ChatVoice>,
  speaker: Option<String>,
  account: Option<i64>,
}

async fn voiced(
  db: &DatabaseConnection,
  roads: &Roads,
  key: &str,
  chat_id: i64,
  said_by: &SaidBy,
) -> Option<Voiced> {
  match said_by {
    SaidBy::Owner => Some(Voiced {
      voice: Some(ChatVoice::HumanReader),
      speaker: None,
      account: None,
    }),
    SaidBy::Room => Some(Voiced {
      voice: added::voice_in(db, roads, key, chat_id).await,
      speaker: None,
      account: None,
    }),
    SaidBy::Account { account, .. } => {
      let declared = roads.declaration(key).await?;
      let voice = own_voice(&declared)?;

      Some(Voiced {
        voice: Some(voice),
        speaker: declared.speakers.then(|| crate::accounts::identity_key(*account)),
        account: Some(*account),
      })
    }
  }
}

// How a kept message goes to one chat service: in the voice `voiced` gives, another account's words with the
// account's name before them where the provider takes no speaker, and not at all where it has no voice of its own.
async fn going(db: &DatabaseConnection, roads: &Roads, key: &str, delivery: &Delivery) -> Option<Going> {
  let Some(voiced) = voiced(db, roads, key, delivery.chat_id, &delivery.said_by).await else {
    println!(
      "chat {}: message {} is not carried to {key}, which has no voice of its own to say another account's words in",
      delivery.chat_id, delivery.message_id
    );

    return None;
  };
  let carried = match &delivery.said_by {
    SaidBy::Account { name, .. } if voiced.speaker.is_none() => delivery.carried.named(name),
    _ => delivery.carried.clone(),
  };

  Some(Going {
    voice: voiced.voice,
    speaker: voiced.speaker,
    account: voiced.account,
    carried,
  })
}

/// A typing mark of someone writing in the console handed to each chat service of the room that shows typing, in
/// the voice their words would go in (docs/receiver.md, 「Typing and presence」): the owner's own, or another
/// account's in the service's own voice naming that account; a service with no voice of its own, or one reading the
/// room in the owner's account alone, is handed no other account's mark. A refusal is let go.
pub async fn typing(db: &DatabaseConnection, roads: &Roads, chat_id: i64, said_by: &SaidBy, on: bool) {
  let Ok(listed) = listed(db, roads, chat_id).await else { return };

  for key in listed.providers {
    if key == Road::Record.key() {
      continue;
    }

    let Some(service) = roads.chats().road(&key) else { continue };
    let shows = roads.facts(&Road::Service(key.clone())).await.can.iter().any(|verb| verb == "typing");

    if !shows || (*said_by != SaidBy::Owner && roads.reads_only(&key, chat_id).await) {
      continue;
    }

    let Some(voiced) = voiced(db, roads, &key, chat_id, said_by).await else { continue };
    let mut request = ChatSendRequest::new(chat_id, ChatSend::Typing { on });

    request.mode = voiced.voice;
    request.speaker = voiced.speaker;
    request.account_id = voiced.account;

    let _ = roads.send(service.as_ref(), &request).await;
  }
}

// The sends one kept message makes on one chat service, each tagged and the number its copy went out as kept:
// whether the service refused the speaker the first send named, so nothing of it went.
async fn carry(
  db: &DatabaseConnection,
  roads: &Roads,
  service: &dyn Chat,
  key: &str,
  delivery: &Delivery,
  going: &Going,
) -> bool {
  let chat_id = delivery.chat_id;
  let reply = match delivery.reply_to {
    Some(target) => number_on(db, chat_id, target, key).await,
    None => None,
  };
  let sends = match &going.carried {
    Carried::Send(send) => vec![send.clone()],
    Carried::Files { files, caption } => {
      let limit = roads.declaration(key).await.and_then(|declared| declared.dialect.limits.caption);

      file_sends(files, caption, limit)
    }
    Carried::Voice { audio, caption } => vec![ChatSend::Voice {
      audio: audio.clone(),
      said: caption.clone(),
      caption: (!caption.trim().is_empty()).then(|| caption.clone()),
      transcript: None,
    }],
  };

  for (at, send) in sends.into_iter().enumerate() {
    let tag = match stood(db, chat_id, delivery.message_id, key, None).await {
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

    request.mode = going.voice;
    request.speaker = going.speaker.clone();
    request.account_id = going.account;
    request.tag = Some(tag.to_string());
    request.reply_to_message_id = if at == 0 { reply } else { None };

    match roads.send(service, &request).await {
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
      Err(failure)
        if at == 0
          && going.speaker.is_some()
          && matches!(
            failure.code(),
            Some(ErrorCode::SpeakerNotAllowed | ErrorCode::SpeakerUnnumbered)
          ) =>
      {
        println!(
          "chat {chat_id}: {key} took no speaker for message {}, so it goes named in its own voice",
          delivery.message_id
        );

        return true;
      }
      Err(ClientFailure::Answered {
        answer: Some(answer),
        ..
      }) => eprintln!("chat {chat_id}: {key} refused message {}: {}", delivery.message_id, answer.error),
      Err(why) => eprintln!("chat {chat_id}: {key} did not take message {}: {why}", delivery.message_id),
    }
  }

  false
}

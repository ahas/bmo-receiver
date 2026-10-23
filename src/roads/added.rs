//! A room added on a chat service in a mode (docs/receiver.md, 「Adding a room」): the modes a service offers, read
//! off what it declares and never off its name, and the room kept with its mode, its title and its list of
//! providers, the service it was chosen on and the room's record.

use std::collections::BTreeMap;

use bmo_chat_client::{ChatCapability, ChatDeclaration, ChatModeStanding, ChatVoice, ProviderRoom};
use sea_orm::{ConnectionTrait, DatabaseConnection, TransactionTrait};
use serde::{Deserialize, Serialize};

use super::{Road, providers};
use crate::receiver::rows;
use crate::{Fault, all, now_ms, one, run};

/// What a room added is to the agents: the user alone, or answered as the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RoomMode {
  /// The room's designated agents answer it, every send the receiver makes there in the account of the person the
  /// service reads.
  Avatar,
  /// The user alone: the room is kept and carried, and no agent is woken in it.
  RecordOnly,
}

impl RoomMode {
  /// The mode as the store and the wire name it.
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Avatar => "avatar",
      Self::RecordOnly => "record-only",
    }
  }

  fn read(text: &str) -> Option<Self> {
    [Self::Avatar, Self::RecordOnly].into_iter().find(|mode| mode.as_str() == text)
  }
}

/// Whether a service lists the rooms its account is in, as it declares.
pub fn lists_rooms(declared: &ChatDeclaration) -> bool {
  can(declared, ChatCapability::RoomList)
}

/// Whether a service declares a capability: listing its contacts, or starting a conversation of a shape.
pub fn can(declared: &ChatDeclaration, capability: ChatCapability) -> bool {
  declared.can.contains(&capability)
}

/// The modes a room added on a service may take: the user alone on every service, and answered as the user where
/// the service's `human-avatar` mode stands active.
pub fn offered(declared: &ChatDeclaration) -> Vec<RoomMode> {
  let answers_as_the_user = declared
    .modes
    .iter()
    .any(|mode| mode.voice == ChatVoice::HumanAvatar && mode.standing == ChatModeStanding::Active);

  if answers_as_the_user {
    vec![RoomMode::Avatar, RoomMode::RecordOnly]
  } else {
    vec![RoomMode::RecordOnly]
  }
}

/// Whether a room takes a mode, as the service's answer for it states its modes: the user alone in every room, and
/// answered as the user only where the room's modes, when stated, hold `human-avatar`. A read-only room is never
/// answered in.
pub fn takes(room: &ProviderRoom, mode: RoomMode) -> bool {
  match (mode, &room.modes) {
    (RoomMode::Avatar, Some(modes)) => modes.contains(&ChatVoice::HumanAvatar),
    _ => true,
  }
}

/// The mode a room was added in; none for a room never added.
pub async fn mode_of<C: ConnectionTrait>(db: &C, chat_id: i64) -> Result<Option<RoomMode>, Fault> {
  let row =
    one(db, r#"SELECT "mode" FROM "room_modes" WHERE "chat_id" = $1"#, vec![chat_id.into()]).await?;

  Ok(
    row
      .map(|row| row.try_get::<String>("", "mode"))
      .transpose()?
      .as_deref()
      .and_then(RoomMode::read),
  )
}

/// Every room added, with its mode.
pub async fn modes(db: &DatabaseConnection) -> Result<BTreeMap<i64, RoomMode>, Fault> {
  let rows = all(db, r#"SELECT "chat_id", "mode" FROM "room_modes""#, vec![]).await?;
  let mut modes = BTreeMap::new();

  for row in rows {
    let mode = row.try_get::<String>("", "mode")?;

    if let Some(mode) = RoomMode::read(&mode) {
      modes.insert(row.try_get::<i64>("", "chat_id")?, mode);
    }
  }

  Ok(modes)
}

/// The voice a send the receiver makes in a room goes in where it names none of its own: the account of the
/// person the service reads in a room added `avatar`, and the voice the service answers the room by everywhere
/// else. A room whose mode cannot be read is said in the journal and answered as any room.
pub async fn voice_in<C: ConnectionTrait>(db: &C, chat_id: i64) -> Option<ChatVoice> {
  match mode_of(db, chat_id).await {
    Ok(Some(RoomMode::Avatar)) => Some(ChatVoice::HumanAvatar),
    Ok(_) => None,
    Err(why) => {
      eprintln!("chat {chat_id}: the room's mode could not be read: {}", why.detail);

      None
    }
  }
}

/// A room added on the service under `key` in `mode`: its mode, its title where one is given, and its list of
/// providers set to that service and the record, in one write. Whether it was added; a room added before is not,
/// and nothing is written.
pub async fn add(
  db: &DatabaseConnection,
  chat_id: i64,
  key: &str,
  mode: RoomMode,
  title: Option<&str>,
) -> Result<bool, Fault> {
  let transaction = db.begin().await.map_err(Fault::from)?;
  let added = run(
    &transaction,
    r#"INSERT INTO "room_modes" ("chat_id", "mode", "added_at") VALUES ($1, $2, $3) ON CONFLICT DO NOTHING"#,
    vec![chat_id.into(), mode.as_str().into(), now_ms().into()],
  )
  .await?;

  if added == 0 {
    transaction.rollback().await.map_err(Fault::from)?;

    return Ok(false);
  }

  if let Some(title) = title {
    rows::set_chat_title(&transaction, chat_id, title).await?;
  }

  providers::write_list(&transaction, chat_id, &[key.to_owned(), Road::Record.key().to_owned()]).await?;
  transaction.commit().await.map_err(Fault::from)?;

  Ok(true)
}

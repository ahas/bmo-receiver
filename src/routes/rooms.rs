//! A room as the receiver holds it (`docs/api.md`, 「Rooms」): the deployment's owner, one room, the agents
//! answering it, one message, the chat providers it is on, and a room added on a chat service in a mode. Any chat
//! reads as a room; one never registered reads as one whose registration is gone, and the work filed from it
//! reaches the owner's private chat instead. What a provider offers a room added on it, the rooms and the contacts
//! of its account and the conversations it starts, is read off its declaration, never off its name.

use std::collections::BTreeMap;

use axum::{
  body::Bytes,
  extract::{Path, RawQuery, State},
  http::StatusCode,
};
use bmo_chat_client::{
  AccountRoomsQuery, ChatCapability, ChatContactKind, ChatDeclaration, ChatStart, ClientFailure,
  ContactsQuery, ErrorCode, ProviderRoom, RoomKind,
};
use receiver_codes::{RoomAdditionRefusal, RoomModeRefusal};
use receiver_ports::chat::Chat;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Accepted, Answered, Refusal, Served, answered, code, query, read, refused, store};
use crate::Fault;
use crate::receiver::wake::agents;
use crate::receiver::{NotReceived, Receiver, reads, rows};
use crate::roads::added::{self, RoomMode};
use crate::roads::{Road, providers};
use crate::routes::say::known;

// A read of the room that could not be made because a unit it reaches did not answer.
fn reached(why: NotReceived) -> Refusal {
  refused(StatusCode::BAD_GATEWAY, "upstream-failed", &why.to_string())
}

/// The deployment's owner: the owner as a sender, and the owner's private chat.
pub async fn owner(State(served): State<Served>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();

  answered(json!({ "senderId": receiver.owner_id(), "room": receiver.owner_room() }))
}

/// What a room is to the deployment, in the shape the room list answers.
pub async fn room(State(served): State<Served>, Path(room): Path<i64>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();

  room_read(&receiver, room).await.and_then(answered)
}

// A room as `GET /rooms/{roomId}` answers it.
async fn room_read(receiver: &Receiver, room: i64) -> Result<Value, Refusal> {
  let db = receiver.db();
  let owner_room = receiver.roads().is_owner_room(room).await;
  let registered = rows::is_registered(db, room).await.map_err(store)?;
  let work = rows::KeptWorkRoom::of(room, receiver.owner_room(), registered);
  let shown = reads::shown(db).await.map_err(store)?;
  let line = shown.iter().find(|line| line.chat_id == room);
  let pinned = crate::receiver::pinned::pinned(db).await.map_err(store)?;
  let supervisors = crate::receiver::figures::answerers(db, room).await.map_err(store)?;
  let senders = reads::senders(db, room).await.map_err(store)?;
  let mode = added::mode_of(db, room).await.map_err(store)?;

  Ok(json!({
    "id": room,
    "title": line.and_then(|line| line.title.clone()),
    "messageCount": line.map_or(0, |line| line.message_count),
    "lastMessageAt": line.and_then(|line| line.last_message_at),
    "pinned": pinned.contains(&room),
    "senders": senders,
    "supervisors": supervisors,
    "ownerRoom": owner_room,
    "registered": registered,
    "workRoom": work.chat_id,
    "mode": mode.map(RoomMode::as_str),
  }))
}

/// The agents answering a room: the default answerer, the voice of the room, and every agent named to answer
/// there that is not resting.
pub async fn answerers(State(served): State<Served>, Path(room): Path<i64>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();

  answered(answerers_json(&receiver, room).await?)
}

// The answerers of a room, as `GET /rooms/{roomId}/answerers` and a room's registration answer them.
async fn answerers_json(receiver: &Receiver, room: i64) -> Result<Value, Refusal> {
  let default = agents::default_designated(receiver).await.map_err(reached)?;
  let voice = agents::voice_of(receiver, room).await.map_err(reached)?;
  let named = agents::named_designated(receiver, room).await.map_err(reached)?;
  let listed: Vec<Value> = named
    .iter()
    .map(|agent| json!({ "agentName": agent.name, "defaultAnswerer": agent.default_answerer }))
    .collect();

  Ok(json!({
    "default": default.name,
    "voice": voice,
    "items": listed,
    "more": false,
  }))
}

/// A message said in a room; nothing found where the receiver does not keep it.
pub async fn message(State(served): State<Served>, Path((room, message)): Path<(i64, i64)>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let Some(shaped) = message_shape(&receiver, room, message).await.map_err(store)? else {
    return Err(refused(
      StatusCode::NOT_FOUND,
      "not-found",
      &format!("The receiver keeps no message {message} in room {room}."),
    ));
  };

  answered(shaped)
}

/// One message in the one message shape, its files and reactions beside it; none where the room keeps no such
/// message.
pub async fn message_shape(receiver: &Receiver, room: i64, message: i64) -> Result<Option<Value>, Fault> {
  let Some(kept) = reads::message_of(receiver.db(), room, message).await? else {
    return Ok(None);
  };
  let files: Vec<Value> = reads::files(receiver.db(), room, message)
    .await?
    .iter()
    .map(|file| {
      json!({
        "kind": file.kind,
        "source": file.source,
        "storedPath": file.stored_path,
        "summary": file.summary,
        "collectedAt": file.collected_at,
      })
    })
    .collect();

  Ok(Some(json!({
    "id": kept.message_id,
    "room": kept.chat_id,
    "senderId": kept.sender_id,
    "senderName": kept.sender_name,
    "sentAt": kept.sent_at,
    "text": kept.text,
    "imagePath": kept.image_path,
    "replyToMessageId": kept.reply_to_message_id,
    "answerSeconds": kept.answer_seconds,
    "round": kept.round,
    "album": kept.album,
    "via": kept.via,
    "agentName": kept.agent_name,
    "voice": kept.voice,
    "kind": kept.kind,
    "editedAt": kept.edited_at,
    "deletedAt": kept.deleted_at,
    "pinnedAt": kept.pinned_at,
    "reactions": reads::reactions(receiver.db(), room, message).await?,
    "forwardedFrom": super::console::forwarded_from(&kept.forwarded),
    "situation": super::console::situation_json(kept.situation.as_ref()),
    "files": files,
  })))
}

/// Every chat provider a room may be on, each by its key and what it is called (docs/receiver.md, 「A room's chat
/// providers」).
pub async fn providers(State(served): State<Served>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();
  let items: Vec<Value> = providers::every(receiver.roads())
    .await
    .into_iter()
    .map(|provider| {
      let modes: Vec<&str> = provider.room_modes.iter().map(|mode| mode.as_str()).collect();

      json!({
        "key": provider.key,
        "label": provider.label,
        "listsRooms": provider.lists_rooms,
        "listsContacts": provider.lists_contacts,
        "startsChats": provider.starts_chats,
        "startsGroups": provider.starts_groups,
        "roomModes": modes,
      })
    })
    .collect();

  answered(json!({ "items": items, "more": false }))
}

// A call on a chat service that did not answer.
fn unanswered(key: &str, why: impl std::fmt::Display) -> Refusal {
  refused(
    StatusCode::BAD_GATEWAY,
    "upstream-failed",
    &format!("The {key} road did not answer: {why}"),
  )
}

// What a chat service declares of itself now; a service that does not answer is no service to add a room on.
async fn declared_now(receiver: &Receiver, key: &str) -> Result<ChatDeclaration, Refusal> {
  receiver
    .roads()
    .declaration_now(key)
    .await
    .ok_or_else(|| unanswered(key, "its declaration"))
}

// Whether the receiver holds a room: its own, one it keeps a title or a message of, or one added. The receiver's
// own record answers it, never the provider.
async fn holds(receiver: &Receiver, room: i64, modes: &BTreeMap<i64, RoomMode>) -> Result<bool, Refusal> {
  Ok(modes.contains_key(&room) || known(receiver, room).await?)
}

/// The rooms a provider's account is in, relayed from its own list page by page, each with whether the receiver
/// holds it and the mode the receiver added it in. Refused where no provider stands under the key, and where the
/// provider does not declare that it lists them.
pub async fn account_rooms(
  State(served): State<Served>,
  Path(key): Path<String>,
  raw: RawQuery,
) -> Answered {
  let asked = query::<AccountRoomsQuery>(raw)?;
  let receiver: Receiver = (*served.receiver).clone();

  if !providers::stands(receiver.roads(), &key) {
    return Err(refused(StatusCode::NOT_FOUND, "not-found", &format!("No provider stands under {key}.")));
  }

  let Some(service) = receiver.roads().chats().road(&key) else {
    let label = receiver.roads().facts(&Road::Record).await.label;

    return Err(code(&RoomAdditionRefusal::RoomsUnlisted { label }));
  };
  let declared = declared_now(&receiver, &key).await?;

  if !added::lists_rooms(&declared) {
    return Err(code(&RoomAdditionRefusal::RoomsUnlisted {
      label: declared.label,
    }));
  }

  let page = match service.account_rooms(&asked).await {
    Ok(page) => page,
    Err(failure) if failure.code() == Some(ErrorCode::UnsupportedCapability) => {
      return Err(code(&RoomAdditionRefusal::RoomsUnlisted {
        label: declared.label,
      }));
    }
    Err(why) => return Err(unanswered(&key, why)),
  };
  let modes = added::modes(receiver.db()).await.map_err(store)?;
  let mut items: Vec<Value> = Vec::with_capacity(page.items.len());

  for room in &page.items {
    items.push(json!({
      "id": room.id,
      "title": room.title,
      "kind": match room.kind {
        RoomKind::Owner => "owner",
        RoomKind::Group => "group",
      },
      "privateTo": room.private_to,
      "held": holds(&receiver, room.id, &modes).await?,
      "mode": modes.get(&room.id).map(|mode| mode.as_str()),
    }));
  }

  answered(json!({ "items": items, "more": page.more }))
}

/// The contacts of a provider's account, relayed from its own list page by page, each with the one-to-one the
/// account has with it, whether the receiver holds that room and the mode it was added in. Refused where no
/// provider stands under the key, and where the provider does not declare that it lists them.
pub async fn contacts(State(served): State<Served>, Path(key): Path<String>, raw: RawQuery) -> Answered {
  let asked = query::<ContactsQuery>(raw)?;
  let receiver: Receiver = (*served.receiver).clone();

  if !providers::stands(receiver.roads(), &key) {
    return Err(refused(StatusCode::NOT_FOUND, "not-found", &format!("No provider stands under {key}.")));
  }

  let Some(service) = receiver.roads().chats().road(&key) else {
    let label = receiver.roads().facts(&Road::Record).await.label;

    return Err(code(&RoomAdditionRefusal::ContactsUnlisted { label }));
  };
  let declared = declared_now(&receiver, &key).await?;

  if !added::can(&declared, ChatCapability::ContactList) {
    return Err(code(&RoomAdditionRefusal::ContactsUnlisted {
      label: declared.label,
    }));
  }

  let page = match service.contacts(&asked).await {
    Ok(page) => page,
    Err(failure) if failure.code() == Some(ErrorCode::UnsupportedCapability) => {
      return Err(code(&RoomAdditionRefusal::ContactsUnlisted {
        label: declared.label,
      }));
    }
    Err(why) => return Err(unanswered(&key, why)),
  };
  let modes = added::modes(receiver.db()).await.map_err(store)?;
  let mut items: Vec<Value> = Vec::with_capacity(page.items.len());

  for contact in &page.items {
    let held = match contact.room_id {
      Some(room) => holds(&receiver, room, &modes).await?,
      None => false,
    };

    items.push(json!({
      "id": contact.id,
      "name": contact.name,
      "kind": match contact.kind {
        ChatContactKind::Person => "person",
        ChatContactKind::Bot => "bot",
      },
      "roomId": contact.room_id,
      "held": held,
      "mode": contact.room_id.and_then(|room| modes.get(&room)).map(|mode| mode.as_str()),
    }));
  }

  answered(json!({ "items": items, "more": page.more }))
}

/// A room to add: the chat service it is chosen on, its mode, and one of the room as that service numbers it, a
/// contact to start a one-to-one with, or the contacts to start a group with; and its title where the caller names
/// one, which a group needs.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Adding {
  provider: String,
  mode: RoomMode,
  #[serde(default)]
  room_id: Option<i64>,
  #[serde(default)]
  contact_id: Option<i64>,
  #[serde(default)]
  contact_ids: Option<Vec<i64>>,
  #[serde(default)]
  title: Option<String>,
}

/// What a room to add is chosen as: a room of the account, or a conversation to start.
enum Chosen {
  Room(i64),
  Start(ChatStart),
}

impl Adding {
  // The one choice the body makes; a body naming none, or more than one, is no addition.
  fn chosen(&self) -> Result<Chosen, Refusal> {
    let bad = |why: &str| refused(StatusCode::BAD_REQUEST, "bad-request", why);

    match (self.room_id, self.contact_id, &self.contact_ids) {
      (Some(room), None, None) => Ok(Chosen::Room(room)),
      (None, Some(contact_id), None) => Ok(Chosen::Start(ChatStart::OneToOne { contact_id })),
      (None, None, Some(contact_ids)) => match self.title.as_deref().map(str::trim) {
        Some(title) if !title.is_empty() => Ok(Chosen::Start(ChatStart::Group {
          title: title.to_owned(),
          contact_ids: contact_ids.clone(),
        })),
        _ => Err(bad("A group to start needs a title.")),
      },
      _ => Err(bad("A room to add names one of roomId, contactId or contactIds.")),
    }
  }
}

// The room a conversation started on a service came to; refused where the service does not start one of that
// shape, does not know a contact named, or will not start it.
async fn started(
  service: &dyn Chat,
  declared: &ChatDeclaration,
  key: &str,
  start: &ChatStart,
) -> Result<ProviderRoom, Refusal> {
  let label = || declared.label.clone();
  let capability = match start {
    ChatStart::OneToOne { .. } => ChatCapability::StartChat,
    ChatStart::Group { .. } => ChatCapability::StartGroup,
  };

  if !added::can(declared, capability) {
    return Err(code(&RoomAdditionRefusal::ChatsUnstarted { label: label() }));
  }

  let failure = match service.start_chat(start).await {
    Ok(room) => return Ok(room),
    Err(failure) => failure,
  };
  let said = match &failure {
    ClientFailure::Answered {
      answer: Some(answer),
      ..
    } => answer.error.clone(),
    other => other.to_string(),
  };

  Err(match failure.code() {
    Some(ErrorCode::UnsupportedCapability) => code(&RoomAdditionRefusal::ChatsUnstarted { label: label() }),
    Some(ErrorCode::NotFound) => code(&RoomAdditionRefusal::ContactUnknown { label: label() }),
    Some(ErrorCode::BadRequest) => refused(StatusCode::BAD_REQUEST, "bad-request", &said),
    Some(ErrorCode::Platform | ErrorCode::Gone) => code(&RoomAdditionRefusal::StartRefused {
      label: label(),
      detail: said,
    }),
    _ => unanswered(key, failure),
  })
}

/// A room added on a chat service in a mode: one of its account's rooms, or the room a conversation the service
/// starts comes to; kept with its title and its list of providers set to that service and the record, and
/// answered with the room as it now stands. A mode the service does not offer is refused before anything is
/// started; a room added before, one the service does not say is its own and a mode the room does not take are
/// refused, and nothing is written.
pub async fn add_room(State(served): State<Served>, body: Bytes) -> Accepted {
  let asked = read::<Adding>(&body)?;
  let chosen = asked.chosen()?;
  let receiver: Receiver = (*served.receiver).clone();
  let key = asked.provider.as_str();
  let Some(service) = receiver.roads().chats().road(key) else {
    return Err(refused(
      StatusCode::BAD_REQUEST,
      "bad-request",
      &format!("No chat service stands under {key}."),
    ));
  };

  if let Chosen::Room(room) = chosen
    && added::mode_of(receiver.db(), room).await.map_err(store)?.is_some()
  {
    return Err(code(&RoomAdditionRefusal::RoomAdded { room }));
  }

  let declared = declared_now(&receiver, key).await?;
  let avatar_unsupported = || {
    code(&RoomAdditionRefusal::AvatarUnsupported {
      label: declared.label.clone(),
    })
  };

  if !added::offered(&declared).contains(&asked.mode) {
    return Err(avatar_unsupported());
  }

  let held = match &chosen {
    Chosen::Room(room) => match service.room(*room).await {
      Ok(held) => held,
      Err(ClientFailure::Answered { status: 404, .. }) => {
        return Err(code(&RoomAdditionRefusal::RoomUnknown {
          label: declared.label.clone(),
          room: *room,
        }));
      }
      Err(why) => return Err(unanswered(key, why)),
    },
    Chosen::Start(start) => started(service.as_ref(), &declared, key, start).await?,
  };
  let room = held.id;

  if !held.owns {
    return Err(code(&RoomAdditionRefusal::RoomUnknown {
      label: declared.label.clone(),
      room,
    }));
  }

  if !added::takes(&held, asked.mode) {
    return Err(avatar_unsupported());
  }

  let title = asked.title.filter(|title| !title.trim().is_empty()).or(held.title);
  let made = added::add(receiver.db(), room, key, asked.mode, title.as_deref())
    .await
    .map_err(store)?;

  if !made {
    return Err(code(&RoomAdditionRefusal::RoomAdded { room }));
  }

  // The service has just said what it holds; what was kept of the room before is read again.
  receiver.roads().forget_room(room);

  Ok((StatusCode::CREATED, axum::Json(room_read(&receiver, room).await?)))
}

/// The administrator a registration read or a mode change is asked by, as the console names him.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ByAdministrator {
  #[serde(default)]
  by: String,
}

/// The mode a room's registration is to stand in.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ChangingMode {
  mode: RoomMode,
}

// A registration read or a mode change is the administrator's alone.
async fn administrator(receiver: &Receiver, asked: &ByAdministrator) -> Result<(), Refusal> {
  if super::is_administrator(receiver, asked.by.trim()).await {
    return Ok(());
  }

  Err(refused(
    StatusCode::FORBIDDEN,
    "not-administrator",
    "only the administrator reads or changes a room's registration",
  ))
}

// A room the receiver holds has a registration to read or change: one added, or one its record keeps.
async fn held_registration(receiver: &Receiver, room: i64) -> Result<(), Refusal> {
  if added::mode_of(receiver.db(), room).await.map_err(store)?.is_some() || known(receiver, room).await? {
    return Ok(());
  }

  Err(refused(
    StatusCode::NOT_FOUND,
    "not-found",
    &format!("The receiver holds no room {room}."),
  ))
}

// A room's registration as the read and the change answer it: its mode, its providers, its answerers, and
// whether the chat services on its list read it in the owner's account alone.
async fn registration(receiver: &Receiver, room: i64) -> Result<Value, Refusal> {
  let db = receiver.db();
  let mode = added::mode_of(db, room).await.map_err(store)?;
  let listed = providers::listed(db, receiver.roads(), room).await.map_err(store)?;
  let answerers = answerers_json(receiver, room).await?;
  let mut read_only_by: Vec<String> = vec![];
  let mut services = 0;

  for key in &listed.providers {
    if receiver.roads().chats().road(key).is_none() {
      continue;
    }

    services += 1;

    if receiver.roads().reads_only(key, room).await {
      read_only_by.push(key.clone());
    }
  }

  Ok(json!({
    "room": room,
    "mode": mode.map(RoomMode::as_str),
    "providers": listed.providers,
    "chosen": listed.chosen,
    "answerers": answerers,
    "readOnly": services > 0 && read_only_by.len() == services,
    "readOnlyBy": read_only_by,
  }))
}

/// A room's registration: its mode, the chat providers it is on, the agents answering it, and whether it is
/// read-only and by which chat services.
pub async fn room_registration(
  State(served): State<Served>,
  Path(room): Path<i64>,
  raw: RawQuery,
) -> Answered {
  let asked = query::<ByAdministrator>(raw)?;
  let receiver: Receiver = (*served.receiver).clone();

  administrator(&receiver, &asked).await?;
  held_registration(&receiver, room).await?;
  answered(registration(&receiver, room).await?)
}

// Whether every chat service on the room's list offers the avatar mode and takes it for the room, refused naming
// the service and the reason where one does not. A room on no chat service has no account to answer as the person.
async fn offer_avatar(receiver: &Receiver, room: i64) -> Result<(), Refusal> {
  let listed = providers::listed(receiver.db(), receiver.roads(), room).await.map_err(store)?;

  // What a service says of the room is asked afresh: the account the road sends through joining it, or an
  // account leaving, moves it.
  receiver.roads().forget_room(room);

  let mut services = 0;

  for key in &listed.providers {
    if receiver.roads().chats().road(key).is_none() {
      continue;
    }

    services += 1;

    let Some(declared) = receiver.roads().declaration_now(key).await else {
      return Err(unanswered(key, "its declaration"));
    };

    if !added::offered(&declared).contains(&RoomMode::Avatar) {
      return Err(code(&RoomModeRefusal::AvatarUnoffered {
        label: declared.label,
      }));
    }

    match receiver.roads().takes_avatar(key, room).await {
      Some(true) => {}
      Some(false) => {
        return Err(code(&RoomModeRefusal::RoomTakesNoAvatar {
          label: declared.label,
          room,
        }));
      }
      None => {
        return Err(code(&RoomAdditionRefusal::RoomUnknown {
          label: declared.label,
          room,
        }));
      }
    }
  }

  if services == 0 {
    return Err(code(&RoomModeRefusal::NoChatService));
  }

  Ok(())
}

/// A room's mode changed between `record-only` and `avatar`, its record, title, providers and messages untouched;
/// answered with the registration as it now stands. A mode the services on its list do not offer, or a room they
/// never answer in, is refused and nothing is written.
pub async fn change_room_mode(
  State(served): State<Served>,
  Path(room): Path<i64>,
  raw: RawQuery,
  body: Bytes,
) -> Answered {
  let asked = query::<ByAdministrator>(raw)?;
  let changing = read::<ChangingMode>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();

  administrator(&receiver, &asked).await?;
  held_registration(&receiver, room).await?;

  let db = receiver.db();
  let Some(current) = added::mode_of(db, room).await.map_err(store)? else {
    return Err(code(&RoomModeRefusal::RoomNotAdded { room }));
  };

  if changing.mode != current {
    if changing.mode == RoomMode::Avatar {
      offer_avatar(&receiver, room).await?;
    }

    added::set_mode(db, room, changing.mode).await.map_err(store)?;
  }

  answered(registration(&receiver, room).await?)
}

/// The list of providers a room is to be on, set whole.
#[derive(Deserialize)]
struct Choosing {
  providers: Vec<String>,
}

// A room the receiver does not hold is no room whose providers can be read or set.
async fn held(receiver: &Receiver, room: i64) -> Result<(), Refusal> {
  if known(receiver, room).await? {
    return Ok(());
  }

  Err(refused(
    StatusCode::NOT_FOUND,
    "not-found",
    &format!("The receiver holds no room {room}."),
  ))
}

// The room's providers as they stand.
async fn standing(receiver: &Receiver, room: i64) -> Answered {
  let listed = providers::listed(receiver.db(), receiver.roads(), room).await.map_err(store)?;

  answered(json!({ "providers": listed.providers, "chosen": listed.chosen }))
}

/// The providers a room is on, and whether its list was set.
pub async fn room_providers(State(served): State<Served>, Path(room): Path<i64>) -> Answered {
  let receiver: Receiver = (*served.receiver).clone();

  held(&receiver, room).await?;
  standing(&receiver, room).await
}

/// A room's providers set whole: a list of providers that stand, each once, and at least one.
pub async fn choose_providers(
  State(served): State<Served>,
  Path(room): Path<i64>,
  body: Bytes,
) -> Answered {
  let asked = read::<Choosing>(&body)?;
  let receiver: Receiver = (*served.receiver).clone();

  held(&receiver, room).await?;

  if asked.providers.is_empty() {
    return Err(refused(StatusCode::BAD_REQUEST, "bad-request", "A room is on one provider at least."));
  }

  for (at, key) in asked.providers.iter().enumerate() {
    if !providers::stands(receiver.roads(), key) {
      return Err(refused(
        StatusCode::BAD_REQUEST,
        "bad-request",
        &format!("No provider stands under {key}."),
      ));
    }

    if asked.providers[..at].contains(key) {
      return Err(refused(StatusCode::BAD_REQUEST, "bad-request", &format!("{key} is named twice.")));
    }
  }

  providers::choose(receiver.db(), room, &asked.providers).await.map_err(store)?;
  standing(&receiver, room).await
}

//! The roads a room may be answered on (docs/receiver.md, 「The one hard rule」 and 「Identity, and reaching the
//! owner」): each chat service the configuration lists, reached through bmo-chat-client, and the room's own record.
//! Which service a room is on, whether it is the owner's private chat, who the agent is there and whose one-to-one
//! it is are each that service's answer; what differs between services is read from what each declares. Nothing
//! here compares a road's key to a literal.

pub mod added;
pub mod archived;
pub mod declared;
pub mod providers;
pub mod record;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use bmo_chat_client::{
  ChatDeclaration, ChatModeStanding, ChatVoice, ClientFailure, ProviderRoom, RoomKind,
};
use receiver_policy::kept::KEPT;
use receiver_policy::roads::ROADS;
use receiver_ports::chat::{Chat, ChatServices};

pub use declared::Facts;

use crate::kept::Kept;
use crate::receiver::{Receiver, Room};
use crate::room_record::forget::{Forgetting, RoomForget};
use crate::room_record::{Facing, RoomHolding, Says};
use crate::{all, one};
use receiver_codes::{RoomRemovalRefusal, RoomRemovalWarning};

// The administrator's own private chat on any road, where reports and questions reach the administrator.
fn owner_refusal<'a>(facing: &'a Facing<'a>) -> Says<'a, RoomRemovalRefusal> {
  Box::pin(async move {
    let owner = facing.receiver.roads().is_owner_room(facing.room).await;

    Ok(owner.then_some(RoomRemovalRefusal::OwnersChat))
  })
}

crate::room_holding!(RoomHolding::Refuses {
  what: "the administrator's own private chat",
  refuses: owner_refusal,
});

// A room a chat service still holds, where a later message would register it anew.
fn reachable<'a>(facing: &'a Facing<'a>) -> Says<'a, RoomRemovalWarning> {
  Box::pin(async move {
    let roads = facing.receiver.roads();
    let Some(key) = roads.room_road(facing.room).await else { return Ok(None) };
    let road = roads.facts(&Road::Service(key)).await.label;

    Ok(Some(RoomRemovalWarning::Road { road }))
  })
}

crate::room_holding!(RoomHolding::Warns {
  what: "a road that can still reach the room",
  warns: reachable,
});

// What each road said of a room let go as the room's registration is removed.
fn forget_facts(receiver: &Receiver, room: i64) -> Forgetting<'_> {
  Box::pin(async move { receiver.roads().forget_room(room) })
}

crate::room_forget!(RoomForget {
  what: "what each road said of the room",
  forget: forget_facts,
});

/// A road a line goes out on: the room's own record, or a chat service by its key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Road {
  Record,
  Service(String),
}

impl Road {
  /// The road's key, as a message and a turn name it.
  pub fn key(&self) -> &str {
    match self {
      Self::Record => &ROADS.record.key,
      Self::Service(key) => key,
    }
  }
}

/// The chat services, what each has declared, read once and kept, and what each said of a room, kept for a
/// short span by road and room: the room as the service answered it, or none where it does not hold the room.
#[derive(Clone)]
pub struct Roads {
  chats: ChatServices,
  declared: Arc<Mutex<BTreeMap<String, ChatDeclaration>>>,
  rooms: Kept<(String, i64), Option<ProviderRoom>>,
  // The receiver's store, which says which rooms stand on the record alone; given once the receiver stands.
  store: Arc<OnceLock<sea_orm::DatabaseConnection>>,
  // The owner's private room: the configuration's `owner_room`, else the room numbered as his own number; given
  // once the receiver stands.
  owners: Arc<OnceLock<i64>>,
}

// A chat service's answer that did not come is said once and read as no answer, so one service that does not
// answer never stops a room another service holds.
fn heard<T>(road: &str, asked: &str, answer: Result<T, impl std::fmt::Display>) -> Option<T> {
  answer
    .map_err(|why| eprintln!("bmo-receiver: the {road} road did not answer {asked}: {why}"))
    .ok()
}

impl Roads {
  /// The chat services, what each says of a room kept for the span the policy sets.
  pub fn new(chats: ChatServices) -> Self {
    Self::keeping_for(chats, Duration::from_secs(KEPT.room_seconds))
  }

  /// The chat services, what each says of a room kept for `span`.
  pub fn keeping_for(chats: ChatServices, span: Duration) -> Self {
    Self {
      chats,
      declared: Arc::default(),
      rooms: Kept::new(span),
      store: Arc::default(),
      owners: Arc::default(),
    }
  }

  /// The owner's private room (docs/receiver.md, 「Identity, and reaching the owner」); the first given stands.
  pub fn with_owner_room(&self, room: i64) {
    let _ = self.owners.set(room);
  }

  /// Whether a room is the owner's private chat: the owner's private room alone, a chat service's `kind` of
  /// `owner` read for no room; where the roads were given none, what the service says, `said`.
  pub fn owners_room(&self, chat_id: i64, said: bool) -> bool {
    match self.owners.get() {
      Some(room) => *room == chat_id,
      None => said,
    }
  }

  /// The roads reading the receiver's store for the rooms on the record alone; the first given stands.
  pub fn with_store(&self, db: sea_orm::DatabaseConnection) {
    let _ = self.store.set(db);
  }

  /// Whether a room no chat service holds stands on the record alone (docs/receiver.md, 「A room's chat
  /// providers」): its list of providers names the record, or, where none was chosen, the room is one the receiver
  /// numbered on the record or keeps messages of.
  async fn on_record(&self, chat_id: i64) -> bool {
    let Some(db) = self.store.get() else { return false };
    let record = Road::Record.key();
    let chosen: Vec<String> = all(
      db,
      r#"SELECT "provider" FROM "room_providers" WHERE "chat_id" = $1"#,
      vec![chat_id.into()],
    )
    .await
    .map(|rows| rows.iter().filter_map(|row| row.try_get::<String>("", "provider").ok()).collect())
    .unwrap_or_default();

    if !chosen.is_empty() {
      return chosen.iter().any(|key| key == record);
    }

    one(
      db,
      r#"SELECT 1 AS "known" WHERE EXISTS (SELECT 1 FROM "provider_rooms" WHERE "provider" = $1 AND "chat_id" = $2)
           OR EXISTS (SELECT 1 FROM "messages" WHERE "chat_id" = $2)"#,
      vec![record.into(), chat_id.into()],
    )
    .await
    .ok()
    .flatten()
    .is_some()
  }

  /// Whether the record holds a room alone by its own rows (docs/receiver.md, 「A room's chat providers」): its
  /// chosen list names no chat service the configuration lists, or, where none was chosen, the record gave its
  /// number. No chat service is asked of such a room: what it is, the record says.
  pub async fn record_alone(&self, chat_id: i64) -> bool {
    let Some(db) = self.store.get() else { return false };

    if let Some(chosen) = self.chosen(chat_id).await {
      return !chosen.iter().any(|key| self.service(key).is_some());
    }

    one(
      db,
      r#"SELECT 1 AS "numbered" FROM "provider_rooms" WHERE "provider" = $1 AND "chat_id" = $2"#,
      vec![Road::Record.key().into(), chat_id.into()],
    )
    .await
    .ok()
    .flatten()
    .is_some()
  }

  /// The providers a room's list names, in order, where its list was chosen; none where it was never set
  /// (docs/receiver.md, 「A room's chat providers」).
  pub async fn chosen(&self, chat_id: i64) -> Option<Vec<String>> {
    let db = self.store.get()?;
    let chosen: Vec<String> = all(
      db,
      r#"SELECT "provider" FROM "room_providers" WHERE "chat_id" = $1 ORDER BY "place""#,
      vec![chat_id.into()],
    )
    .await
    .map(|rows| rows.iter().filter_map(|row| row.try_get::<String>("", "provider").ok()).collect())
    .unwrap_or_default();

    (!chosen.is_empty()).then_some(chosen)
  }

  /// Whether a room takes and is sent what a road carries: every road where its list was never chosen, and the
  /// roads its chosen list names where it was.
  pub async fn admits(&self, chat_id: i64, key: &str) -> bool {
    self.chosen(chat_id).await.is_none_or(|chosen| chosen.iter().any(|named| named == key))
  }

  // A provider's key as the road it is.
  fn road_of(key: &str) -> Road {
    if key == ROADS.record.key { Road::Record } else { Road::Service(key.to_owned()) }
  }

  /// The room's own road: the first provider its chosen list names; where none was chosen, the chat service that
  /// holds it, else the record where the room stands on it alone.
  pub async fn own(&self, chat_id: i64) -> Option<Road> {
    if let Some(chosen) = self.chosen(chat_id).await {
      return chosen.first().map(|key| Self::road_of(key));
    }

    if let Some(key) = self.room_road(chat_id).await {
      return Some(Road::Service(key));
    }

    self.on_record(chat_id).await.then_some(Road::Record)
  }

  pub fn chats(&self) -> &ChatServices {
    &self.chats
  }

  fn service(&self, key: &str) -> Option<&Arc<dyn Chat>> {
    self.chats.road(key)
  }

  /// What a chat service declares of itself, asked once and kept; none where it is not listed or does not answer.
  pub async fn declaration(&self, key: &str) -> Option<ChatDeclaration> {
    if let Some(declared) = self.declared.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).get(key) {
      return Some(declared.clone());
    }

    let declared = heard(key, "its declaration", self.service(key)?.declaration().await)?;

    self
      .declared
      .lock()
      .unwrap_or_else(|poisoned| poisoned.into_inner())
      .insert(key.to_owned(), declared.clone());

    Some(declared)
  }

  /// What a chat service declares of itself now, asked afresh and kept in place of what was kept; none where it is
  /// not listed or does not answer. A mode's standing moves while the service runs, so a choice that turns on it
  /// asks here.
  pub async fn declaration_now(&self, key: &str) -> Option<ChatDeclaration> {
    let declared = heard(key, "its declaration", self.service(key)?.declaration().await)?;

    self
      .declared
      .lock()
      .unwrap_or_else(|poisoned| poisoned.into_inner())
      .insert(key.to_owned(), declared.clone());

    Some(declared)
  }

  /// What a road declares; a chat service that has declared nothing is named by its key alone, performs nothing and
  /// folds nothing.
  pub async fn facts(&self, road: &Road) -> Facts {
    match road {
      Road::Record => Facts::record(),
      Road::Service(key) => match self.declaration(key).await {
        Some(declared) => Facts::of(&declared),
        None => Facts {
          key: key.clone(),
          label: key.clone(),
          can: vec![],
          unpinned: vec![],
          writing: String::new(),
          foldable: false,
          shows: None,
          speakers: false,
        },
      },
    }
  }

  /// What a chat service says of a room, kept for the span: the room as it answers it, none where it does not
  /// hold the room, and nothing where it is not listed or did not answer, which is kept not at all. A room the
  /// record holds alone is held by no service, and no service is asked of it.
  async fn room_of(&self, key: &str, chat_id: i64) -> Option<Option<ProviderRoom>> {
    if self.record_alone(chat_id).await {
      return Some(None);
    }

    let held = (key.to_owned(), chat_id);

    if let Some(kept) = self.rooms.get(&held) {
      return Some(kept);
    }

    let said = match self.service(key)?.room(chat_id).await {
      Ok(room) => Some(room).filter(|room| room.owns),
      Err(ClientFailure::Answered { status: 404, .. }) => None,
      Err(why) => {
        eprintln!("bmo-receiver: the {key} road did not answer what it says of the room: {why}");

        return None;
      }
    };

    self.rooms.keep(held, said.clone());

    Some(said)
  }

  /// Whether a chat service holds a room, as it says.
  async fn holds(&self, key: &str, chat_id: i64) -> bool {
    self.room_of(key, chat_id).await.flatten().is_some()
  }

  /// What each road said of a room let go, so the next read asks again.
  pub fn forget_room(&self, chat_id: i64) {
    self.rooms.forget(|(_, room)| *room == chat_id);
  }

  /// The chat service a room is on: the first that says the room is its own.
  pub async fn room_road(&self, chat_id: i64) -> Option<String> {
    for key in self.chats.names() {
      if self.holds(key, chat_id).await {
        return Some(key.to_owned());
      }
    }

    None
  }

  /// What the road a room is on says of it: whether it is the owner's private chat, who the agent is there, and
  /// whose one-to-one with the agent it is; nothing where no road holds the room. Each road's answer is kept for
  /// the span, and the road that holds the room answers all three.
  pub async fn room_facts(&self, chat_id: i64) -> Room {
    for key in self.chats.names() {
      let Some(Some(room)) = self.room_of(key, chat_id).await else { continue };

      return Room {
        owner_room: self.owners_room(chat_id, room.kind == RoomKind::Owner),
        identity: room.identity,
        private_to: room.private_to,
      };
    }

    Room {
      owner_room: self.owners_room(chat_id, false),
      ..Room::default()
    }
  }

  /// Whether a chat service reads a room in the owner's account alone (docs/receiver.md, 「A read-only room」): its
  /// answer for the room states the room's modes and they hold `human-reader` alone. Where the answer states none,
  /// the service declares the `human-reader` mode, and either that is the one voice it has active or it holds the
  /// room under no identity of its own. A service that declares nothing, or does not answer for the room, speaks
  /// there as it always has; a room the record holds alone no service reads.
  pub async fn reads_only(&self, key: &str, chat_id: i64) -> bool {
    if self.record_alone(chat_id).await {
      return false;
    }

    let room = self.room_of(key, chat_id).await;

    if let Some(Some(ProviderRoom {
      modes: Some(modes), ..
    })) = &room
    {
      return !modes.is_empty() && modes.iter().all(|voice| *voice == ChatVoice::HumanReader);
    }

    let Some(declared) = self.declaration(key).await else { return false };

    if !declared.modes.iter().any(|mode| mode.voice == ChatVoice::HumanReader) {
      return false;
    }

    let active: Vec<ChatVoice> = declared
      .modes
      .iter()
      .filter(|mode| mode.standing == ChatModeStanding::Active)
      .map(|mode| mode.voice)
      .collect();

    if !active.is_empty() && active.iter().all(|voice| *voice == ChatVoice::HumanReader) {
      return true;
    }

    matches!(room, Some(Some(room)) if room.identity.is_none())
  }

  /// Whether a chat service's answer for a room takes the avatar mode: none where it does not hold the room or did
  /// not answer, else whether the room's modes, where stated, hold `human-avatar`. A room that states modes
  /// without it is never answered in.
  pub async fn takes_avatar(&self, key: &str, chat_id: i64) -> Option<bool> {
    self
      .room_of(key, chat_id)
      .await
      .flatten()
      .map(|room| room.modes.as_ref().is_none_or(|modes| modes.contains(&ChatVoice::HumanAvatar)))
  }

  /// Whether a room is the owner's private chat, as the road it is on says.
  pub async fn is_owner_room(&self, chat_id: i64) -> bool {
    self.room_facts(chat_id).await.owner_room
  }

  /// Who the agent is in a room: the number its words there are filed under, as the room's road says.
  pub async fn identity_in(&self, chat_id: i64) -> Option<i64> {
    self.room_facts(chat_id).await.identity
  }

  // Whether a chat service reaches a room: one of its own, or the owner's room it reaches beyond its own rooms.
  async fn reaches(&self, key: &str, chat_id: i64) -> bool {
    let Some(service) = self.service(key) else { return false };

    if self.record_alone(chat_id).await {
      return false;
    }

    self.holds(key, chat_id).await
      || heard(key, "the owner's room", service.owner_room().await).flatten() == Some(chat_id)
  }

  /// Whether the record is where a line nobody asked for goes in a room: a road reaching the room shows its whole
  /// line, so whatever the record keeps is read there. Where none does, the room's own road carries it; no road
  /// reaches a room the record holds alone.
  pub async fn shown(&self, chat_id: i64) -> bool {
    if self.record_alone(chat_id).await {
      return false;
    }

    for key in self.chats.names() {
      let shows = self.declaration(key).await.is_some_and(|declared| declared.shows.is_some());

      if shows && self.reaches(key, chat_id).await {
        return true;
      }
    }

    false
  }

  /// Where a line nobody asked for goes in a room — a turn opened by the system's own lines alone, a line the service says
  /// itself, a send that names no road: the room's record where a road reaching the room shows it, the room's own
  /// road everywhere else, and none where no road holds the room.
  pub async fn unasked(&self, chat_id: i64) -> Option<Road> {
    // A chosen list alone decides: its first provider is where such a line goes.
    if self.chosen(chat_id).await.is_some() {
      return self.own(chat_id).await;
    }

    if self.shown(chat_id).await {
      return Some(Road::Record);
    }

    self.own(chat_id).await
  }

  /// The road a turn in a room answers on, given the road it was asked on: that road where it is the room's own or
  /// reaches the room, and the room's own road everywhere else.
  pub async fn answering(&self, chat_id: i64, via: Option<&str>) -> Option<Road> {
    let own = self.own(chat_id).await;
    let Some(via) = via else { return own };

    // A chosen list alone decides: the road asked on where the list names it, the room's own road otherwise.
    if let Some(chosen) = self.chosen(chat_id).await {
      return if chosen.iter().any(|named| named == via) { Some(Self::road_of(via)) } else { own };
    }

    if via == ROADS.record.key {
      return if self.shown(chat_id).await { Some(Road::Record) } else { own };
    }

    if own.as_ref().is_some_and(|own| own.key() == via) || self.reaches(via, chat_id).await {
      return Some(Road::Service(via.to_owned()));
    }

    own
  }

  /// The road a send goes out on in a room, given the road its caller named: the one `answering` chooses for a road
  /// named, and the unasked road where none is.
  pub async fn sending(&self, chat_id: i64, via: Option<&str>) -> Option<Road> {
    match via {
      Some(via) => self.answering(chat_id, Some(via)).await,
      None => self.unasked(chat_id).await,
    }
  }

  /// The road a turn answers on, given the roads its messages came in on and those its instructions were asked on:
  /// the one every word came in on, the room's own where they came in on more than one; for a turn of instructions
  /// alone, the road they were asked on where they agree, and otherwise — a turn opened by the system's own lines alone —
  /// the unasked road, whichever road the owner last spoke on.
  pub async fn turn_road(&self, chat_id: i64, came: &[&str], asked: &[&str]) -> Option<Road> {
    let (came, asked) = (distinct(came), distinct(asked));

    match (came.as_slice(), asked.as_slice()) {
      ([_, _, ..], _) => self.own(chat_id).await,
      ([one], _) | ([], [one]) => self.answering(chat_id, Some(one)).await,
      _ => self.unasked(chat_id).await,
    }
  }

  /// Whether an answer can be pinned to a message that came in on a road by one of its read modes. A road that
  /// declares nothing, or a message that names no mode, can be answered where it stands.
  pub async fn pinnable(&self, via: &str, mode: Option<&str>) -> bool {
    let Some(mode) = mode else { return true };

    !self
      .facts(&Road::Service(via.to_owned()))
      .await
      .unpinned
      .iter()
      .any(|unpinned| unpinned == mode)
  }
}

// Each road named once.
fn distinct<'a>(roads: &[&'a str]) -> Vec<&'a str> {
  let mut roads = roads.to_vec();

  roads.sort_unstable();
  roads.dedup();
  roads
}

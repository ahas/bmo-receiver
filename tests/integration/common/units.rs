//! In-memory stand-ins for the units a receiver reaches, each answering its port as a test sets it and keeping
//! what it was asked: a chat service, the gate, the web unit, where the receiver's news is heard, and the wake.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bmo_chat_client::{
  AccountRoomsQuery, ChatCapability, ChatContact, ChatContactKind, ChatDeclaration, ChatDialectDeclaration,
  ChatDialectLimits, ChatHealth, ChatMediaRequest, ChatMember, ChatMembers, ChatModeStanding,
  ChatNumberRanges, ChatNumberSpace, ChatNumbering, ChatReach, ChatReadMode, ChatReceiver, ChatSendRequest,
  ChatSent, ChatStart, ChatStartRequest, ChatVerb, ChatVoice, ClientFailure, ContactsQuery, DialectChecked,
  DialectReading, MessageFile, Page, ProviderRoom, RoomKind, RoomQuery,
};

use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::{
  Deployment, How, Inbound, News, NotReceived, Receiver, Room, Runner, Units, Wake,
};
use bmo_receiver::roads::Roads;
use futures_util::StreamExt;
use futures_util::future::BoxFuture;
use receiver_ports::agency::{
  Agency, Agent, Agents, Delivery, DoubleEnd, DoubleSeat, DoubleStart, DoubleTurn, Event, Events,
  FieldRefusal, Forgotten, KeptEvents, Load, NewSupervisor, OpenRun, Passed, Removal, Repository,
  ResetPoint, Rest, RestKind, Resting, Role, Run, RunLine, RunningJob, Said, Scope, Session, SessionState,
  Turn, TurnRead, Written,
};
use receiver_ports::brain::BrainUnit;
use receiver_ports::chat::{Chat, ChatAnswer, ChatServices};
use receiver_ports::gate::{Entry, Envelope, Gate, IndexGroup, Judgement};
use receiver_ports::hearing::{Heard as HearingHeard, Hearing};
use receiver_ports::http::{Answer, PortFailure, WireError};
use receiver_ports::job::{Chosen, ItemRow, Job, Moved, OpenQuestion, RoomHolding, Talked};
use receiver_ports::voice::{Briefing, NewBriefing, VoiceEngine};
use receiver_ports::warmer::{Target, Used, Warmer};
use receiver_ports::web::{
  Checked, Grant, GrantRow, MessageCheck, ObjectCheck, Render, RoomCheck, User, UserNode, Web,
};
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};

use crate::integration::common::session::StandInSession;

/// The owner every receiver of the tests answers for.
pub const OWNER: i64 = 42;
/// The default answerer's registration name.
pub const AGENT: &str = "agent-a";
/// What the default answerer's own words are filed under.
pub const SENDER: &str = "Agent A";
/// The name a room calls the default answerer by.
pub const ADDRESS: &str = "agent-a";

/// A chat service: the rooms it holds and what it says of each, what it declares, and every send it was handed.
#[derive(Default)]
pub struct StandInChat {
  pub key: String,
  pub shows: Option<String>,
  pub owns: BTreeSet<i64>,
  pub owner_rooms: BTreeSet<i64>,
  pub identities: BTreeMap<i64, i64>,
  pub private: BTreeMap<i64, i64>,
  pub reaches_owner: Option<i64>,
  pub titles: BTreeMap<i64, String>,
  /// The voices each room says it is read by and may be sent in; a room not here says none.
  pub room_modes: BTreeMap<i64, Vec<ChatVoice>>,
  /// How often it was asked what it says of a room.
  pub rooms_asked: AtomicUsize,
  /// Whether nothing listens at its address, and how often it was offered a request while nothing did.
  pub down: bool,
  pub refused: AtomicUsize,
  pub cans: Mutex<BTreeSet<ChatCapability>>,
  /// The read modes it declares, each by its voice and where it stands.
  pub modes: Mutex<Vec<(ChatVoice, ChatModeStanding)>>,
  pub sends: Mutex<Vec<ChatSendRequest>>,
  /// The kinds of send each voice refuses, as a platform refuses what an account cannot do; a refused send is
  /// not among the sends carried through.
  pub voice_refusals: Vec<(ChatVoice, ChatVerb)>,
  /// How often a send was turned down for the voice it was asked in.
  pub turn_downs: AtomicUsize,
  /// The contacts of its account, every listing of them it was asked for, and every conversation it was asked
  /// to start.
  pub contacts: Vec<ChatContact>,
  /// The members of each room it has read, by the room's number; a room not here holds none it has read.
  pub members: BTreeMap<i64, Vec<ChatMember>>,
  /// Whether it was built before the protocol carried the members route, so no route answers there.
  pub routeless_members: bool,
  pub contacts_asked: Mutex<Vec<ContactsQuery>>,
  pub starts: Mutex<Vec<ChatStartRequest>>,
  /// What each send is answered with; where none is set, each send is numbered anew, the first 900, as a
  /// platform numbers the messages of a room.
  pub answer: Mutex<Option<ChatSent>>,
  /// What each message's files are answered with when asked again, by the message's number.
  pub files: BTreeMap<i64, FilesAnswer>,
  /// Every message whose files were asked again, in the order asked, with the sent time the ask named.
  pub files_asked: Mutex<Vec<(i64, Option<i64>)>>,
  numbered: AtomicI64,
  // The next send, held until the test lets it go, and whether one stands held now.
  held_send: Mutex<Option<Arc<tokio::sync::Notify>>>,
  holding_send: Mutex<bool>,
  // The receiving side it asks about a room before it answers what it says of that room, where one is given.
  asking_back: Mutex<Option<Arc<dyn ChatReceiver>>>,
}

/// How many questions about a room a service asking back asks the receiver at most, so a receiver that asked it
/// again each time ends rather than asking for ever.
const ASKED_BACK_AT_MOST: usize = 8;

impl StandInChat {
  pub fn new(key: &str) -> Self {
    Self {
      key: key.to_owned(),
      ..Self::default()
    }
  }

  /// Asks `receiver` about a room each time it is asked what it says of that room, before it answers, as a
  /// service that reads a room in the owner's account asks whether the receiver reads it.
  pub fn asks_back(&self, receiver: Arc<dyn ChatReceiver>) {
    *self.asking_back.lock().unwrap() = Some(receiver);
  }

  /// A service nothing listens for: every question of its declaration, a room or the owner's room is refused as
  /// a connection is, and counted.
  pub fn down(mut self) -> Self {
    self.down = true;
    self
  }

  pub fn holding(mut self, rooms: &[i64]) -> Self {
    self.owns.extend(rooms);
    self
  }

  /// The service's own room that is the owner's private chat.
  pub fn owners(mut self, room: i64) -> Self {
    self.owns.insert(room);
    self.owner_rooms.insert(room);
    self.reaches_owner = Some(room);
    self
  }

  /// A service whose screen shows the whole of the rooms it reaches, the owner's among them.
  pub fn showing(mut self, reaches_owner: i64) -> Self {
    self.shows = Some("its screen shows the whole room".to_owned());
    self.reaches_owner = Some(reaches_owner);
    self
  }

  pub fn agent_in(mut self, room: i64, identity: i64) -> Self {
    self.identities.insert(room, identity);
    self
  }

  /// The voices one of its rooms says it is read by and may be sent in.
  pub fn room_moding(mut self, room: i64, voices: &[ChatVoice]) -> Self {
    self.room_modes.insert(room, voices.to_vec());
    self
  }

  /// The send verbs the service declares beside the three every road carries.
  pub fn canning(mut self, verbs: &[ChatCapability]) -> Self {
    self.cans.get_mut().unwrap().extend(verbs.iter().copied());
    self
  }

  /// The kinds of send a voice refuses with the platform's own reason.
  pub fn refusing_in(mut self, voice: ChatVoice, verbs: &[ChatVerb]) -> Self {
    self.voice_refusals.extend(verbs.iter().map(|verb| (voice, *verb)));
    self
  }

  /// A service built before the protocol carried the members route: asking it answers that no route stands there.
  pub fn routeless_members(mut self) -> Self {
    self.routeless_members = true;
    self
  }

  /// The members it has read of a room.
  pub fn membering(mut self, room: i64, members: Vec<ChatMember>) -> Self {
    self.members.insert(room, members);
    self
  }

  /// A contact of its account, a person, with the one-to-one the account has with it where it has one.
  pub fn contacting(mut self, id: i64, name: &str, room_id: Option<i64>) -> Self {
    self.contacts.push(ChatContact {
      id,
      name: name.to_owned(),
      kind: ChatContactKind::Person,
      room_id,
    });
    self
  }

  /// Every conversation it was asked to start, in order.
  pub fn started(&self) -> Vec<ChatStartRequest> {
    self.starts.lock().unwrap().clone()
  }

  /// What the service calls one of its rooms.
  pub fn titling(mut self, room: i64, title: &str) -> Self {
    self.titles.insert(room, title.to_owned());
    self
  }

  /// A read mode the service declares, by its voice and where it stands; it may be set again while it runs.
  pub fn moding(&self, voice: ChatVoice, standing: ChatModeStanding) {
    let mut modes = self.modes.lock().unwrap();

    modes.retain(|(held, _)| *held != voice);
    modes.push((voice, standing));
  }

  /// A room of the service's own that is one person's private chat with the default answerer.
  pub fn privating(mut self, room: i64, person: i64) -> Self {
    self.owns.insert(room);
    self.private.insert(room, person);
    self
  }

  /// Every send carried through so far, in order.
  /// The service answers a message's files as given when they are asked again, and declares `fetchMedia`.
  pub fn filing(mut self, message: i64, answer: FilesAnswer) -> Self {
    self.cans.get_mut().unwrap().insert(ChatCapability::FetchMedia);
    self.files.insert(message, answer);
    self
  }

  /// The messages whose files were asked again, in the order asked.
  pub fn asked_files(&self) -> Vec<i64> {
    self.files_asked.lock().unwrap().iter().map(|(message, _)| *message).collect()
  }

  pub fn sent(&self) -> Vec<ChatSendRequest> {
    self.sends.lock().unwrap().clone()
  }

  /// How often a send was turned down for the voice it was asked in, each voice refusal counted once.
  pub fn turned_down(&self) -> usize {
    self.turn_downs.load(Ordering::SeqCst)
  }

  /// The next send stands held, unsent, until what this answers is notified.
  pub fn holding_the_next_send(&self) -> Arc<tokio::sync::Notify> {
    let release = Arc::new(tokio::sync::Notify::new());

    *self.held_send.lock().unwrap() = Some(release.clone());
    release
  }

  /// Whether a send stands held now.
  pub fn send_held(&self) -> bool {
    *self.holding_send.lock().unwrap()
  }
}

// A refusal of the stand-in in the error shape, as bmo-chat-client reads one.
fn answered_with(status: u16, code: bmo_chat_client::ErrorCode, error: &str) -> ClientFailure {
  ClientFailure::Answered {
    url: "the stand-in chat service".to_owned(),
    status,
    answer: Some(bmo_chat_client::WireError::new(code, error)),
  }
}

/// What a stand-in service answers when a message's files are asked again.
#[derive(Clone, Debug)]
pub enum FilesAnswer {
  Files(Vec<MessageFile>),
  Refused(u16, bmo_chat_client::ErrorCode, &'static str),
  Unreachable,
}

fn not_asked<T>(what: &str) -> BoxFuture<'_, ChatAnswer<T>> {
  panic!("a stand-in chat service was asked {what}, which no test sets")
}

impl StandInChat {
  // What a service nothing listens for answers, where it is down; nothing where it stands.
  fn refusing<T: Send + 'static>(&self, path: &str) -> Option<BoxFuture<'static, ChatAnswer<T>>> {
    if !self.down {
      return None;
    }

    self.refused.fetch_add(1, Ordering::SeqCst);

    let url = format!("http://{}.invalid/{path}", self.key);

    Some(Box::pin(async move {
      Err(ClientFailure::Unreachable {
        url,
        waited: Duration::ZERO,
        why: "connection refused".to_owned(),
      })
    }))
  }
}

impl Chat for StandInChat {
  fn declaration(&self) -> BoxFuture<'_, ChatAnswer<ChatDeclaration>> {
    if let Some(refused) = self.refusing("declaration") {
      return refused;
    }

    let mut can = BTreeSet::from([ChatCapability::Text, ChatCapability::Delete, ChatCapability::Pin]);

    can.extend(self.cans.lock().unwrap().iter().copied());

    let modes = self
      .modes
      .lock()
      .unwrap()
      .iter()
      .map(|(voice, standing)| ChatReadMode {
        voice: *voice,
        reach: ChatReach::AllMessages,
        standing: *standing,
        reason: None,
        can_reply_to: true,
      })
      .collect();
    let declared = ChatDeclaration {
      key: self.key.clone(),
      label: format!("the {} road", self.key),
      configured: None,
      modes,
      can,
      dialect: ChatDialectDeclaration {
        name: "plain".to_owned(),
        limits: ChatDialectLimits {
          text: None,
          caption: None,
          album_photos: None,
          file_bytes: None,
        },
        writing: "plain text".to_owned(),
        foldable: false,
      },
      shows: self.shows.clone(),
      numbering: ChatNumbering {
        rooms: ChatNumberSpace::Native,
        people: ChatNumberSpace::Native,
        messages: ChatNumberSpace::Native,
        ranges: ChatNumberRanges::default(),
      },
      speakers: false,
    };

    Box::pin(async move { Ok(declared) })
  }

  fn health(&self) -> BoxFuture<'_, ChatAnswer<ChatHealth>> {
    Box::pin(async { Ok(ChatHealth::ready("0.1.0")) })
  }

  fn send<'a>(&'a self, request: &'a ChatSendRequest) -> BoxFuture<'a, ChatAnswer<ChatSent>> {
    let voice = request.mode.unwrap_or(ChatVoice::BotAccount);

    if self.voice_refusals.contains(&(voice, request.send.kind())) {
      self.turn_downs.fetch_add(1, Ordering::SeqCst);

      return Box::pin(async {
        Err(answered_with(
          422,
          bmo_chat_client::ErrorCode::Platform,
          "The stand-in's account does not send that kind in this voice.",
        ))
      });
    }

    let held = self.held_send.lock().unwrap().take();
    let answer = self.answer.lock().unwrap().clone().unwrap_or_else(|| ChatSent {
      message_id: Some(900 + self.numbered.fetch_add(1, Ordering::SeqCst)),
      missed: vec![],
    });

    Box::pin(async move {
      if let Some(release) = held {
        *self.holding_send.lock().unwrap() = true;
        release.notified().await;
        *self.holding_send.lock().unwrap() = false;
      }

      self.sends.lock().unwrap().push(request.clone());

      Ok(answer)
    })
  }

  fn message_files<'a>(
    &'a self,
    request: &'a ChatMediaRequest,
  ) -> BoxFuture<'a, ChatAnswer<Page<MessageFile>>> {
    let Some(answer) = self.files.get(&request.message_id).cloned() else {
      return not_asked("for the files of a message no test sets");
    };

    self.files_asked.lock().unwrap().push((request.message_id, request.sent_at));

    Box::pin(async move {
      match answer {
        FilesAnswer::Files(items) => Ok(Page { items, more: false }),
        FilesAnswer::Refused(status, code, error) => Err(answered_with(status, code, error)),
        FilesAnswer::Unreachable => Err(ClientFailure::Unreachable {
          url: "http://the-stand-in.invalid/files".to_owned(),
          waited: Duration::ZERO,
          why: "connection refused".to_owned(),
        }),
      }
    })
  }

  fn dialect_read<'a>(&'a self, _text: &'a str) -> BoxFuture<'a, ChatAnswer<DialectReading>> {
    not_asked("to read its dialect")
  }

  fn dialect_check<'a>(&'a self, _markup: &'a str) -> BoxFuture<'a, ChatAnswer<DialectChecked>> {
    not_asked("to check its dialect")
  }

  fn room(&self, chat_id: i64) -> BoxFuture<'_, ChatAnswer<ProviderRoom>> {
    self.rooms_asked.fetch_add(1, Ordering::SeqCst);

    if let Some(refused) = self.refusing(&format!("rooms/{chat_id}")) {
      return refused;
    }

    let owns = self.owns.contains(&chat_id);
    let identity = self.identities.get(&chat_id).copied();
    let kind = if self.owner_rooms.contains(&chat_id) { RoomKind::Owner } else { RoomKind::Group };
    let private_to = self.private.get(&chat_id).copied();
    let title = self.titles.get(&chat_id).cloned();
    let modes = self.room_modes.get(&chat_id).cloned();
    let asking_back = (self.rooms_asked.load(Ordering::SeqCst) <= ASKED_BACK_AT_MOST)
      .then(|| self.asking_back.lock().unwrap().clone())
      .flatten();
    let via = self.key.clone();

    Box::pin(async move {
      if let Some(receiver) = asking_back {
        let _ = receiver.room(chat_id, RoomQuery { via }).await;
      }

      if !owns {
        return Err(ClientFailure::Answered {
          url: "the stand-in chat service".to_owned(),
          status: 404,
          answer: None,
        });
      }

      Ok(ProviderRoom {
        id: chat_id,
        owns,
        identity,
        kind,
        private_to,
        title,
        modes,
      })
    })
  }

  fn account_rooms<'a>(
    &'a self,
    query: &'a AccountRoomsQuery,
  ) -> BoxFuture<'a, ChatAnswer<Page<ProviderRoom>>> {
    Box::pin(async move {
      if !self.cans.lock().unwrap().contains(&ChatCapability::RoomList) {
        return Err(ClientFailure::Answered {
          url: "the stand-in chat service".to_owned(),
          status: 422,
          answer: Some(bmo_chat_client::WireError::new(
            bmo_chat_client::ErrorCode::UnsupportedCapability,
            "The stand-in cannot list the rooms of its account.",
          )),
        });
      }

      let mut listed = vec![];

      for room in self.owns.iter().filter(|room| query.after.is_none_or(|after| **room > after)) {
        listed.push(self.room(*room).await?);
      }

      let limit = query.limit.map_or(usize::MAX, |limit| usize::try_from(limit).unwrap_or(usize::MAX));
      let more = listed.len() > limit;

      listed.truncate(limit);

      Ok(Page {
        items: listed,
        more,
      })
    })
  }

  fn members(&self, chat_id: i64) -> BoxFuture<'_, ChatAnswer<ChatMembers>> {
    if let Some(refused) = self.refusing(&format!("rooms/{chat_id}/members")) {
      return refused;
    }

    Box::pin(async move {
      if self.routeless_members {
        return Err(answered_with(
          404,
          bmo_chat_client::ErrorCode::NoRoute,
          &format!("There is no route at GET /rooms/{chat_id}/members."),
        ));
      }

      if !self.cans.lock().unwrap().contains(&ChatCapability::MemberList) {
        return Err(answered_with(
          422,
          bmo_chat_client::ErrorCode::UnsupportedCapability,
          "The stand-in cannot list the members of a room.",
        ));
      }

      match self.members.get(&chat_id) {
        Some(members) => Ok(ChatMembers {
          members: members.clone(),
        }),
        None => Err(answered_with(
          404,
          bmo_chat_client::ErrorCode::NotFound,
          &format!("The stand-in has read no members of room {chat_id}."),
        )),
      }
    })
  }

  fn contacts<'a>(&'a self, query: &'a ContactsQuery) -> BoxFuture<'a, ChatAnswer<Page<ChatContact>>> {
    Box::pin(async move {
      self.contacts_asked.lock().unwrap().push(*query);

      if !self.cans.lock().unwrap().contains(&ChatCapability::ContactList) {
        return Err(answered_with(
          422,
          bmo_chat_client::ErrorCode::UnsupportedCapability,
          "The stand-in cannot list the contacts of its account.",
        ));
      }

      let mut listed: Vec<ChatContact> = self
        .contacts
        .iter()
        .filter(|contact| query.after.is_none_or(|after| contact.id > after))
        .cloned()
        .collect();

      listed.sort_by_key(|contact| contact.id);

      let limit = query.limit.map_or(usize::MAX, |limit| usize::try_from(limit).unwrap_or(usize::MAX));
      let more = listed.len() > limit;

      listed.truncate(limit);

      Ok(Page {
        items: listed,
        more,
      })
    })
  }

  /// A one-to-one comes to the contact's room, or to a room numbered by the contact, read-only; a group to room
  /// -700, read as any room of the service's; a group titled "Refused" is refused by the platform.
  fn start_chat<'a>(&'a self, request: &'a ChatStartRequest) -> BoxFuture<'a, ChatAnswer<ProviderRoom>> {
    Box::pin(async move {
      self.starts.lock().unwrap().push(request.clone());

      let contact = |id: i64| self.contacts.iter().find(|contact| contact.id == id);
      let unknown = || answered_with(404, bmo_chat_client::ErrorCode::NotFound, "No such contact.");

      match &request.start {
        ChatStart::OneToOne { contact_id } => {
          let contact = contact(*contact_id).ok_or_else(unknown)?;

          Ok(ProviderRoom {
            id: contact.room_id.unwrap_or(contact.id),
            owns: true,
            identity: None,
            kind: RoomKind::Group,
            private_to: Some(contact.id),
            title: Some(contact.name.clone()),
            modes: Some(vec![ChatVoice::HumanReader]),
          })
        }
        ChatStart::Group { title, contact_ids } => {
          if contact_ids.iter().any(|id| contact(*id).is_none()) {
            return Err(unknown());
          }

          if title == "Refused" {
            return Err(answered_with(
              422,
              bmo_chat_client::ErrorCode::Platform,
              "The stand-in: that group cannot be made.",
            ));
          }

          Ok(ProviderRoom {
            id: -700,
            owns: true,
            identity: None,
            kind: RoomKind::Group,
            private_to: None,
            title: Some(title.clone()),
            modes: None,
          })
        }
      }
    })
  }

  fn room_owns(&self, chat_id: i64) -> BoxFuture<'_, ChatAnswer<bool>> {
    let owns = self.owns.contains(&chat_id);

    Box::pin(async move { Ok(owns) })
  }

  fn room_identity(&self, chat_id: i64) -> BoxFuture<'_, ChatAnswer<Option<i64>>> {
    let identity = self.identities.get(&chat_id).copied();

    Box::pin(async move { Ok(identity) })
  }

  fn room_kind(&self, chat_id: i64) -> BoxFuture<'_, ChatAnswer<RoomKind>> {
    let kind = if self.owner_rooms.contains(&chat_id) { RoomKind::Owner } else { RoomKind::Group };

    Box::pin(async move { Ok(kind) })
  }

  fn room_private_of(&self, chat_id: i64) -> BoxFuture<'_, ChatAnswer<Option<i64>>> {
    let person = self.private.get(&chat_id).copied();

    Box::pin(async move { Ok(person) })
  }

  fn room_title(&self, _chat_id: i64) -> BoxFuture<'_, ChatAnswer<Option<String>>> {
    Box::pin(async { Ok(None) })
  }

  fn owner_room(&self) -> BoxFuture<'_, ChatAnswer<Option<i64>>> {
    if let Some(refused) = self.refusing("owner") {
      return refused;
    }

    let reaches = self.reaches_owner;

    Box::pin(async move { Ok(reaches) })
  }
}

/// One index asked of the gate: the room, and the session and agent it was asked for.
pub type IndexAsked = (i64, Option<String>, Option<String>);

/// The gate: every attach of a round's judgements asked is kept, each the room, the message and the round's start, and every copy of gate options asked is kept, each the agent it is for and
/// the agent copied from.
#[derive(Default)]
pub struct StandInGate {
  /// The entries the reference selection answers, and how many times it was asked.
  pub entries: Mutex<Vec<Entry>>,
  pub referred: Mutex<usize>,
  pub attached: Mutex<Vec<(i64, i64, i64)>>,
  pub copied: Mutex<Vec<(String, String)>>,
  /// The index the gate serves, by room, and the session and agent each was asked for; none served where empty.
  pub index: Mutex<BTreeMap<i64, Vec<IndexGroup>>>,
  pub indexed: Mutex<Vec<IndexAsked>>,
}

impl Gate for StandInGate {
  fn index<'a>(
    &'a self,
    room: i64,
    session: Option<&'a str>,
    agent: Option<&'a str>,
  ) -> BoxFuture<'a, Answer<Option<Vec<IndexGroup>>>> {
    Box::pin(async move {
      let index = self.index.lock().unwrap();

      if index.is_empty() {
        return Ok(None);
      }

      self
        .indexed
        .lock()
        .unwrap()
        .push((room, session.map(str::to_owned), agent.map(str::to_owned)));

      Ok(Some(index.get(&room).cloned().unwrap_or_default()))
    })
  }

  fn reference<'a>(&'a self, _envelope: &'a Envelope) -> BoxFuture<'a, Answer<Vec<Entry>>> {
    *self.referred.lock().unwrap() += 1;

    let entries = self.entries.lock().unwrap().clone();

    Box::pin(async move { Ok(entries) })
  }

  fn judge<'a>(&'a self, _judgement: &'a Judgement) -> BoxFuture<'a, Answer<()>> {
    Box::pin(async { Ok(()) })
  }

  fn attach<'a>(&'a self, chat: i64, message: i64, since: i64) -> BoxFuture<'a, Answer<()>> {
    self.attached.lock().unwrap().push((chat, message, since));

    Box::pin(async { Ok(()) })
  }

  fn copy_options<'a>(&'a self, agent: &'a str, from: &'a str) -> BoxFuture<'a, Answer<()>> {
    self.copied.lock().unwrap().push((agent.to_owned(), from.to_owned()));

    Box::pin(async { Ok(()) })
  }
}

/// The web unit: the account model's links and grants a test gives it, whether that model stands in force at
/// all, and a fixed line rendered as its code and the language asked; nothing else is asked.
pub struct StandInWeb {
  pub users: Mutex<BTreeMap<i64, String>>,
  pub grants: Mutex<BTreeSet<(i64, i64, String)>>,
  pub model: Mutex<bool>,
  // The names the web unit says are the administrator's.
  pub admins: Mutex<BTreeSet<String>>,
  pub watched: Mutex<Vec<i64>>,
  // The machine each user's account is bound to, and whether it is attached.
  pub nodes: Mutex<BTreeMap<String, UserNode>>,
  // Each (head, user) where the head heads the user's unit.
  pub heads: Mutex<BTreeSet<(String, String)>>,
}

impl StandInWeb {
  pub fn new() -> Self {
    StandInWeb {
      users: Mutex::default(),
      grants: Mutex::default(),
      model: Mutex::default(),
      admins: Mutex::default(),
      watched: Mutex::default(),
      nodes: Mutex::default(),
      heads: Mutex::default(),
    }
  }

  /// The rooms the console has open, as the web unit answers.
  pub fn watching(&self, rooms: &[i64]) {
    *self.watched.lock().unwrap() = rooms.to_vec();
  }

  /// The name the web unit answers for as the administrator's.
  pub fn admin(&self, name: String) {
    self.admins.lock().unwrap().insert(name);
  }

  /// The account model stands in force: what the room holds in force is read from its grants, and a user's own
  /// thread is followed.
  pub fn modelling(&self) {
    *self.model.lock().unwrap() = true;
  }

  pub fn linking(&self, sender: i64, user: String) {
    self.users.lock().unwrap().insert(sender, user.to_owned());
  }

  pub fn granting(&self, room: i64, sender: i64, relation: String) {
    self.grants.lock().unwrap().insert((room, sender, relation.to_owned()));
  }
}

fn unasked<T>() -> BoxFuture<'static, Answer<T>> {
  panic!("the stand-in web unit was asked what no test sets")
}

impl StandInWeb {
  /// A user's account bound to a machine, attached or not.
  pub fn binding(&self, user: &str, attached: bool) {
    self.nodes.lock().unwrap().insert(
      user.to_owned(),
      UserNode {
        agent: Some(format!("u{user}")),
        attached,
        left: false,
      },
    );
  }
}

impl Web for StandInWeb {
  fn model_in_force(&self) -> BoxFuture<'_, Answer<bool>> {
    let in_force = *self.model.lock().unwrap();

    Box::pin(async move { Ok(in_force) })
  }

  fn user_of_sender(&self, sender: i64) -> BoxFuture<'_, Answer<Option<String>>> {
    let user = self.users.lock().unwrap().get(&sender).cloned();

    Box::pin(async move { Ok(user) })
  }

  fn user<'a>(&'a self, user: &'a str) -> BoxFuture<'a, Answer<User>> {
    let known = self.admins.lock().unwrap().contains(user);

    Box::pin(async move {
      Ok(User {
        user: user.to_owned(),
        administrator: known,
      })
    })
  }

  fn senders_of_user<'a>(&'a self, user: &'a str) -> BoxFuture<'a, Answer<Vec<i64>>> {
    let linked = self
      .users
      .lock()
      .unwrap()
      .iter()
      .filter(|(_, one)| one.as_str() == user)
      .map(|(sender, _)| *sender)
      .collect::<Vec<i64>>();

    Box::pin(async move { Ok(linked) })
  }

  fn check_room<'a>(&'a self, check: &'a RoomCheck) -> BoxFuture<'a, Answer<Checked>> {
    let sender = check.sender.unwrap_or_default();
    let allowed = self.grants.lock().unwrap().contains(&(check.room, sender, check.relation.clone()));
    let checked = Checked {
      allowed,
      grant: check.relation.clone(),
      user: None,
      sender: check.sender,
    };

    Box::pin(async move { Ok(checked) })
  }

  fn check_message<'a>(&'a self, _check: &'a MessageCheck) -> BoxFuture<'a, Answer<Checked>> {
    unasked()
  }

  fn check_object<'a>(&'a self, _check: &'a ObjectCheck) -> BoxFuture<'a, Answer<Checked>> {
    unasked()
  }

  fn room_grants(&self, _chat_id: i64) -> BoxFuture<'_, Answer<Vec<GrantRow>>> {
    Box::pin(async { Ok(vec![]) })
  }

  fn grant<'a>(&'a self, _grant: &'a Grant) -> BoxFuture<'a, Answer<String>> {
    unasked()
  }

  fn revoke<'a>(&'a self, _grant: &'a Grant) -> BoxFuture<'a, Answer<String>> {
    unasked()
  }

  fn render<'a>(&'a self, line: &'a Render) -> BoxFuture<'a, Answer<String>> {
    Box::pin(async move { Ok(format!("{} in {}", line.code, line.language)) })
  }

  fn watched_rooms(&self) -> BoxFuture<'_, Answer<Vec<i64>>> {
    let rooms = self.watched.lock().unwrap().clone();

    Box::pin(async move { Ok(rooms) })
  }

  fn node_of<'a>(&'a self, user: &'a str) -> BoxFuture<'a, Answer<UserNode>> {
    let node = self.nodes.lock().unwrap().get(user).cloned().unwrap_or_default();

    Box::pin(async move { Ok(node) })
  }

  fn heads_unit_of<'a>(&'a self, head: &'a str, user: &'a str) -> BoxFuture<'a, Answer<bool>> {
    let heads = self.heads.lock().unwrap().contains(&(head.to_owned(), user.to_owned()));

    Box::pin(async move { Ok(heads) })
  }
}

/// Where the receiver's news is heard: each message told, in order, and every other event the stream would carry
/// of a window closed or a verdict written.
#[derive(Default)]
pub struct Heard(pub Mutex<Vec<(i64, i64)>>, pub Mutex<Vec<Value>>);

impl Heard {
  pub fn told(&self) -> Vec<(i64, i64)> {
    self.0.lock().unwrap().clone()
  }

  /// The events of one type the stream was told, in order.
  pub fn events(&self, kind: &str) -> Vec<Value> {
    self.1.lock().unwrap().iter().filter(|event| event["type"] == kind).cloned().collect()
  }
}

impl News for Heard {
  fn message(&self, chat_id: i64, message_id: i64) {
    self.0.lock().unwrap().push((chat_id, message_id));
  }

  fn window_closed(&self, chat_id: i64, agent: &str, at: i64) {
    self.1.lock().unwrap().push(bmo_receiver::events::window_closed(chat_id, agent, at));
  }

  fn publish(&self, event: &Value) {
    self.1.lock().unwrap().push(event.clone());
  }
}

/// What the wake was handed: each message by its room and number, whether the owner said it, and how it came.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Woken {
  Words {
    chat_id: i64,
    message_id: i64,
    owner: bool,
    how: How,
  },
  File {
    chat_id: i64,
    message_id: i64,
    owner: bool,
    caption: String,
  },
}

/// The wake, keeping what it was handed and waking nobody.
#[derive(Default)]
pub struct WakeStandIn(pub Mutex<Vec<Woken>>);

impl WakeStandIn {
  pub fn woken(&self) -> Vec<Woken> {
    self.0.lock().unwrap().clone()
  }
}

impl Wake for WakeStandIn {
  fn words<'a>(
    &'a self,
    _receiver: &'a Receiver,
    inbound: &'a Inbound,
    _room: Room,
    owner: bool,
    how: How,
  ) -> BoxFuture<'a, Result<(), NotReceived>> {
    self.0.lock().unwrap().push(Woken::Words {
      chat_id: inbound.chat_id,
      message_id: inbound.message_id,
      owner,
      how,
    });

    Box::pin(async { Ok(()) })
  }

  fn file<'a>(
    &'a self,
    _receiver: &'a Receiver,
    inbound: &'a Inbound,
    _room: Room,
    owner: bool,
    caption: &'a str,
  ) -> BoxFuture<'a, Result<(), NotReceived>> {
    self.0.lock().unwrap().push(Woken::File {
      chat_id: inbound.chat_id,
      message_id: inbound.message_id,
      owner,
      caption: caption.to_owned(),
    });

    Box::pin(async { Ok(()) })
  }
}

/// The deployment every receiver of the tests answers for.
pub fn deployment() -> Deployment {
  Deployment {
    owner_id: OWNER,
    agent: AGENT.to_owned(),
    sender_name: SENDER.to_owned(),
    addresses: vec![ADDRESS.to_owned()],
    infra_room: None,
    owner_room: None,
    notices_room: None,
    language: "en".to_owned(),
    attachment_bytes: Some(CAP),
  }
}

/// The newest run the stand-in agency answers every round's turns opened, the one before it beside it.
pub const ROUND_RUN: i64 = 77;

/// The attachment size cap of the tests' deployment, in bytes.
pub const CAP: u64 = 1024;

/// The length of window the stand-in conversations open when a turn's ground does, in seconds.
pub const WINDOW: i64 = 60;

/// An agent as the agency registers it.
pub fn registration(name: &str, label: &str, words: Vec<&str>, window: Option<i64>) -> Agent {
  let mut row = serde_json::Map::<String, Value>::new();

  row.insert("aliases".to_owned(), serde_json::json!(words.join(",")));

  if let Some(window) = window {
    row.insert("windowSeconds".to_owned(), serde_json::json!(window));
  }

  Agent {
    name: name.to_owned(),
    label: Some(label.to_owned()),
    awake: true,
    brain_folder: None,
    row,
  }
}

/// The agency: the agents it registers, the supervisors whose road answers through the rest,
/// the deployment's rest, every other agent removal it was asked, the field it refuses another agent's call on
/// where a test sets one, every push it was asked with what it says of it (a push failing where it says
/// nothing), and the session its turns go on where a test stands one.
pub struct StandInAgency {
  pub session: Mutex<Option<Arc<StandInSession>>>,
  /// The double's runs: the answers its turns give in their order where a test scripts them, a review's turn
  /// passing where none is left, and every start, turn and finish it was asked.
  pub double_script: Mutex<std::collections::VecDeque<String>>,
  pub double_starts: Mutex<Vec<DoubleStart>>,
  pub double_turns: Mutex<Vec<DoubleTurn>>,
  pub double_finishes: Mutex<Vec<(String, bool)>>,
  /// Every owner whose slots it was asked to give back, in order.
  pub double_releases: Mutex<Vec<String>>,
  /// Whether a double's start is refused, as an agency that lets no run of the role in refuses it.
  pub double_refused: Mutex<bool>,
  pub removals: Mutex<Vec<(String, Removal)>>,
  pub refusing: Mutex<Option<FieldRefusal>>,
  /// The code a removal is refused with, as the agency's 409, where a test sets one.
  pub removal_refused: Mutex<Option<String>>,
  /// Every reset it was asked, the supervisor and the body, in order.
  pub resets: Mutex<Vec<(String, Value)>>,
  pub pushes: Mutex<Vec<(String, i64, String)>>,
  pub saying: Mutex<Option<Said>>,
  /// Where a test holds pushes: each one, once recorded, answers only after the test lets one go here.
  pub held_pushes: Mutex<Option<Arc<tokio::sync::Notify>>>,
  pub agents: Mutex<Vec<Agent>>,
  /// How often it was asked for its agents.
  pub agents_asked: AtomicUsize,
  /// Every supervisor it was asked for by its name, in order.
  pub supervisors_read: Mutex<Vec<String>>,
  /// Every supervisor whose reset point alone it was asked for, in order.
  pub reset_points_read: Mutex<Vec<String>>,
  /// Every supervisor an add handed it, in order.
  pub added_rows: Mutex<Vec<NewSupervisor>>,
  pub roads_answering: Mutex<BTreeSet<String>>,
  pub rest: Mutex<Rest>,
  pub written: Mutex<Vec<Resting>>,
  pub passed: Mutex<Option<RestKind>>,
  // The rooms it was asked to forget, each with whether the asking was a preview.
  pub forgotten: Mutex<Vec<(i64, bool)>>,
  /// The roles it lists.
  pub roles: Mutex<Vec<Role>>,
  /// The repositories it lists.
  pub repositories: Mutex<Vec<Repository>>,
  // The runs it holds, each its id and the moment it began, in epoch seconds.
  pub runs: Mutex<BTreeMap<i64, i64>>,
  // Every round whose runs it was asked for, and every message attached to a run, as the run and the message.
  pub rounds_asked: Mutex<Vec<String>>,
  /// The sessions a fold was asked of.
  pub folds: Mutex<Vec<String>>,
  /// The runs still open, as the list of runs answers them.
  pub open: Mutex<Vec<OpenRun>>,
  /// The jobs starting or running, as the list of jobs answers them.
  pub running: Mutex<Vec<RunningJob>>,
  pub attached: Mutex<Vec<(i64, i64)>>,
  // The sessions it lists, each with its state, where a test sets them.
  pub listed: Mutex<Option<Vec<(Session, SessionState)>>>,
  // Whether it answers nothing of its agents and sessions, as an agency that is down.
  pub down: Mutex<bool>,
  // Each session's loaded documents, as the last keep left them.
  pub loads: Mutex<BTreeMap<String, Vec<Load>>>,
  // Every session whose loads it was asked to forget, in order.
  pub loads_forgotten: Mutex<Vec<String>>,
  // Every session it was asked to end, in order.
  pub ends: Mutex<Vec<String>>,
  /// The events it keeps, as its page of kept events answers them.
  pub kept: Mutex<Vec<Event>>,
  /// The streams it opens in their order where a test scripts them: each carries its events and then ends, as a
  /// stream an agency's restart cuts; one opened past the script is refused as an agency that is down.
  pub streams: Mutex<Option<std::collections::VecDeque<Vec<Event>>>>,
  /// What its progress read answers for each turn, where a test sets it; `unknown-turn` for any other.
  pub turns: Mutex<BTreeMap<i64, TurnRead>>,
  /// The agency on a real port that carries its turns, their events and their reads, where a test stands one.
  pub carrier: Mutex<Option<Arc<dyn Agency>>>,
}

impl StandInAgency {
  pub fn new() -> Self {
    StandInAgency {
      session: Mutex::default(),
      double_script: Mutex::default(),
      double_starts: Mutex::default(),
      double_turns: Mutex::default(),
      double_finishes: Mutex::default(),
      double_releases: Mutex::default(),
      double_refused: Mutex::default(),
      removals: Mutex::default(),
      refusing: Mutex::default(),
      removal_refused: Mutex::default(),
      resets: Mutex::default(),
      pushes: Mutex::default(),
      saying: Mutex::default(),
      held_pushes: Mutex::default(),
      agents: Mutex::default(),
      agents_asked: AtomicUsize::new(0),
      supervisors_read: Mutex::default(),
      reset_points_read: Mutex::default(),
      added_rows: Mutex::default(),
      roads_answering: Mutex::default(),
      rest: Mutex::new(Rest {
        resting: false,
        until: None,
        kind: None,
      }),
      written: Mutex::default(),
      passed: Mutex::default(),
      forgotten: Mutex::default(),
      roles: Mutex::default(),
      repositories: Mutex::default(),
      runs: Mutex::default(),
      rounds_asked: Mutex::default(),
      folds: Mutex::default(),
      open: Mutex::default(),
      running: Mutex::default(),
      attached: Mutex::default(),
      listed: Mutex::default(),
      down: Mutex::default(),
      loads: Mutex::default(),
      loads_forgotten: Mutex::default(),
      ends: Mutex::default(),
      kept: Mutex::default(),
      streams: Mutex::default(),
      turns: Mutex::default(),
      carrier: Mutex::default(),
    }
  }

  /// Its turns, their events and their reads carried from now on by `carrier`.
  pub fn carried_through(&self, carrier: Arc<dyn Agency>) {
    *self.carrier.lock().unwrap() = Some(carrier);
  }

  fn carrier(&self) -> Option<Arc<dyn Agency>> {
    self.carrier.lock().unwrap().clone()
  }

  /// The streams it opens from now on, each its events and then its end.
  pub fn streaming(&self, streams: Vec<Vec<Event>>) {
    *self.streams.lock().unwrap() = Some(streams.into());
  }

  /// What its progress read answers for the turn from now on.
  pub fn reading(&self, turn: i64, read: TurnRead) {
    self.turns.lock().unwrap().insert(turn, read);
  }

  /// The answers its double's turns give, in their order.
  pub fn doubling(&self, answers: &[&str]) {
    *self.double_script.lock().unwrap() = answers.iter().map(|answer| (*answer).to_owned()).collect();
  }

  /// The events it keeps from now on, oldest first.
  pub fn keeping(&self, events: Vec<Event>) {
    *self.kept.lock().unwrap() = events;
  }

  /// Every session it was asked to end, in order.
  pub fn ended(&self) -> Vec<String> {
    self.ends.lock().unwrap().clone()
  }

  /// Each session's loaded documents, as the last keep left them.
  pub fn loaded(&self) -> BTreeMap<String, Vec<Load>> {
    self.loads.lock().unwrap().clone()
  }

  /// Every session whose loads it was asked to forget, in order.
  pub fn loads_forgotten(&self) -> Vec<String> {
    self.loads_forgotten.lock().unwrap().clone()
  }

  /// The sessions it lists, each with the state it answers for it.
  pub fn listing(&self, sessions: Vec<(Session, SessionState)>) {
    *self.listed.lock().unwrap() = Some(sessions);
  }

  /// Answering nothing of its agents and sessions from now on.
  pub fn going_down(&self) {
    *self.down.lock().unwrap() = true;
  }

  fn is_down(&self) -> bool {
    *self.down.lock().unwrap()
  }

  pub fn registering(&self, agents: Vec<Agent>) {
    *self.agents.lock().unwrap() = agents.clone();
  }

  pub fn answering_on_their_roads(&self, agents: Vec<String>) {
    *self.roads_answering.lock().unwrap() =
      agents.iter().map(|agent| agent.to_owned()).collect::<BTreeSet<String>>();
  }

  pub fn at_rest(&self, rest: Rest) {
    *self.rest.lock().unwrap() = rest.clone();
  }

  pub fn asked(&self) -> Vec<Resting> {
    self.written.lock().unwrap().clone()
  }

  /// The session its turns go on.
  pub fn carrying_on(&self, session: Arc<StandInSession>) {
    *self.session.lock().unwrap() = Some(session);
  }

  fn standing_session(&self) -> Option<Arc<StandInSession>> {
    self.session.lock().unwrap().clone()
  }
}

fn not_asked_agency<T>(what: &str) -> BoxFuture<'_, Answer<T>> {
  panic!("a stand-in agency was asked {what}, which no test sets")
}

// What an agency that is down answers.
fn agency_down<T: Send + 'static>(path: &str) -> BoxFuture<'static, Answer<T>> {
  let url = format!("http://agency.invalid/{path}");

  Box::pin(async move {
    Err(PortFailure::Answered {
      url,
      status: 503,
      answer: None,
    })
  })
}

impl Agency for StandInAgency {
  fn supervisors(&self) -> BoxFuture<'_, Answer<Agents>> {
    self.agents_asked.fetch_add(1, Ordering::SeqCst);

    if self.is_down() {
      return agency_down("supervisors");
    }

    Box::pin(async move {
      Ok(Agents {
        agents: self.agents.lock().unwrap().clone(),
        more: false,
      })
    })
  }

  fn supervisor<'a>(&'a self, name: &'a str) -> BoxFuture<'a, Answer<Option<Agent>>> {
    self.supervisors_read.lock().unwrap().push(name.to_owned());

    if self.is_down() {
      return agency_down("supervisors");
    }

    let found = self.agents.lock().unwrap().iter().find(|agent| agent.name == name).cloned();

    Box::pin(async move { Ok(found) })
  }

  fn reset_point<'a>(&'a self, name: &'a str) -> BoxFuture<'a, Answer<Option<ResetPoint>>> {
    self.reset_points_read.lock().unwrap().push(name.to_owned());

    if self.is_down() {
      return agency_down("supervisors");
    }

    let found =
      self
        .agents
        .lock()
        .unwrap()
        .iter()
        .find(|agent| agent.name == name)
        .map(|agent| ResetPoint {
          supervisor: agent.name.clone(),
          reset_point: agent.row.get("resetPoint").and_then(Value::as_i64),
        });

    Box::pin(async move { Ok(found) })
  }

  fn roles(&self) -> BoxFuture<'_, Answer<Vec<Role>>> {
    let roles = self.roles.lock().unwrap().clone();

    Box::pin(async move { Ok(roles) })
  }

  fn repositories(&self) -> BoxFuture<'_, Answer<Vec<Repository>>> {
    let repositories = self.repositories.lock().unwrap().clone();

    Box::pin(async move { Ok(repositories) })
  }

  fn road_answers<'a>(&'a self, supervisor: &'a str) -> BoxFuture<'a, Answer<bool>> {
    let answers = self.roads_answering.lock().unwrap().contains(supervisor);

    Box::pin(async move { Ok(answers) })
  }

  fn rest(&self) -> BoxFuture<'_, Answer<Rest>> {
    let rest = self.rest.lock().unwrap().clone();

    Box::pin(async move { Ok(rest) })
  }

  fn write_rest<'a>(&'a self, resting: &'a Resting) -> BoxFuture<'a, Answer<Written>> {
    self.written.lock().unwrap().push(resting.clone());

    Box::pin(async move { Ok(Written { silenced: true }) })
  }

  fn passed_rest(&self) -> BoxFuture<'_, Answer<Passed>> {
    let kind = *self.passed.lock().unwrap();

    Box::pin(async move { Ok(Passed { kind }) })
  }

  fn start<'a>(&'a self, agent: &'a str, scope: Scope) -> BoxFuture<'a, Answer<Session>> {
    let Some(session) = self.standing_session() else {
      return not_asked_agency("a session");
    };

    Box::pin(async move { session.start(agent, scope).await })
  }

  fn turn<'a>(&'a self, turn: &'a Turn) -> BoxFuture<'a, Answer<Delivery>> {
    if let Some(carrier) = self.carrier() {
      return Box::pin(async move { carrier.turn(turn).await });
    }

    let Some(session) = self.standing_session() else { return not_asked_agency("a turn") };

    Box::pin(async move { session.turn(turn).await })
  }

  fn double_start<'a>(&'a self, start: &'a DoubleStart) -> BoxFuture<'a, Answer<DoubleSeat>> {
    if *self.double_refused.lock().unwrap() {
      return Box::pin(async {
        Err(PortFailure::Answered {
          url: "http://agency.invalid/slot-requests".to_owned(),
          status: 409,
          answer: None,
        })
      });
    }

    let mut starts = self.double_starts.lock().unwrap();

    starts.push(start.clone());

    let seat = DoubleSeat {
      token: format!("slot-{}", starts.len()),
      session: format!("double@run:{}", starts.len()),
      run: start.run.clone(),
    };

    Box::pin(async move { Ok(seat) })
  }

  fn double_turn<'a>(
    &'a self,
    seat: &'a DoubleSeat,
    turn: &'a DoubleTurn,
  ) -> BoxFuture<'a, Answer<DoubleEnd>> {
    self.double_turns.lock().unwrap().push(turn.clone());

    if let Some(carrier) = self.carrier() {
      return Box::pin(async move { carrier.double_turn(seat, turn).await });
    }

    // A review's turn passes, as the session's own stand-in passes one it hides.
    let answer = if turn.prompt.contains(&receiver_policy::drafting::DRAFTING.review.marker) {
      receiver_policy::drafting::DRAFTING.review.pass.clone()
    } else {
      self
        .double_script
        .lock()
        .unwrap()
        .pop_front()
        .unwrap_or_else(|| "unscripted".to_owned())
    };

    Box::pin(async move {
      Ok(DoubleEnd {
        answer,
        outcome: "answered".to_owned(),
        reason: String::new(),
      })
    })
  }

  fn double_finish<'a>(
    &'a self,
    _seat: &'a DoubleSeat,
    outcome: &'a str,
    cancel: bool,
  ) -> BoxFuture<'a, Answer<()>> {
    self.double_finishes.lock().unwrap().push((outcome.to_owned(), cancel));

    Box::pin(async { Ok(()) })
  }

  fn state<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<SessionState>> {
    if self.is_down() {
      return agency_down("session/state");
    }

    let listed = self.listed.lock().unwrap().clone().unwrap_or_default();

    if let Some((_, state)) = listed.into_iter().find(|(listed, _)| listed.id == session) {
      return Box::pin(async move { Ok(state) });
    }

    let Some(session) = self.standing_session() else {
      return not_asked_agency("a session's state");
    };

    Box::pin(async move { session.state().await })
  }

  fn double_release<'a>(&'a self, owner: &'a str) -> BoxFuture<'a, Answer<()>> {
    self.double_releases.lock().unwrap().push(owner.to_owned());

    Box::pin(async { Ok(()) })
  }

  fn interrupt<'a>(&'a self, _session: &'a str, _turn: i64) -> BoxFuture<'a, Answer<()>> {
    not_asked_agency("an interrupt")
  }

  fn fold<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<()>> {
    self.folds.lock().unwrap().push(session.to_owned());
    Box::pin(async { Ok(()) })
  }

  // A session is ended only where a test stands one, as a replaced session is.
  fn end<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<()>> {
    if self.standing_session().is_none() {
      return not_asked_agency("an end");
    }

    self.ends.lock().unwrap().push(session.to_owned());

    Box::pin(async { Ok(()) })
  }

  fn sessions(&self) -> BoxFuture<'_, Answer<Vec<Session>>> {
    if self.is_down() {
      return agency_down("session/list");
    }

    let listed = self.listed.lock().unwrap().clone().unwrap_or_default();

    Box::pin(async move { Ok(listed.into_iter().map(|(session, _)| session).collect()) })
  }

  fn forget_room(&self, chat_id: i64, dry_run: bool) -> BoxFuture<'_, Answer<Forgotten>> {
    self.forgotten.lock().unwrap().push((chat_id, dry_run));

    Box::pin(async {
      Ok(Forgotten {
        gone: json!({ "sessions": 1 }).as_object().unwrap().clone(),
        stayed: vec![],
      })
    })
  }

  fn register_hook<'a>(
    &'a self,
    _point: &'a str,
    _url: &'a str,
    _token: &'a str,
  ) -> BoxFuture<'a, Answer<()>> {
    Box::pin(async { Ok(()) })
  }

  fn run_log<'a>(&'a self, _run: &'a str, _after: Option<i64>) -> BoxFuture<'a, Answer<Vec<RunLine>>> {
    let Some(session) = self.standing_session() else {
      return not_asked_agency("a run's log");
    };

    Box::pin(async move { session.run_log().await })
  }

  fn run(&self, id: i64) -> BoxFuture<'_, Answer<Option<Run>>> {
    let run = self.runs.lock().unwrap().get(&id).map(|started_at| Run {
      started_at: *started_at,
    });

    Box::pin(async move { Ok(run) })
  }

  fn round_runs<'a>(&'a self, round: &'a str) -> BoxFuture<'a, Answer<Vec<i64>>> {
    self.rounds_asked.lock().unwrap().push(round.to_owned());

    // Newest first, as the agency answers.
    Box::pin(async move { Ok(vec![ROUND_RUN, ROUND_RUN - 1]) })
  }

  fn open_runs(&self) -> BoxFuture<'_, Answer<Vec<OpenRun>>> {
    if self.is_down() {
      return agency_down("runs");
    }

    let open = self.open.lock().unwrap().clone();

    Box::pin(async move { Ok(open) })
  }

  fn running_jobs(&self) -> BoxFuture<'_, Answer<Vec<RunningJob>>> {
    if self.is_down() {
      return agency_down("jobs");
    }

    let running = self.running.lock().unwrap().clone();

    Box::pin(async move { Ok(running) })
  }

  fn attach_message(&self, run: i64, message: i64) -> BoxFuture<'_, Answer<()>> {
    self.attached.lock().unwrap().push((run, message));

    Box::pin(async move { Ok(()) })
  }

  fn loads<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<Vec<Load>>> {
    let kept = self.loads.lock().unwrap().get(session).cloned().unwrap_or_default();

    Box::pin(async move { Ok(kept) })
  }

  fn keep_loads<'a>(&'a self, session: &'a str, loads: &'a [Load]) -> BoxFuture<'a, Answer<()>> {
    self.loads.lock().unwrap().insert(session.to_owned(), loads.to_vec());

    Box::pin(async { Ok(()) })
  }

  fn forget_loads<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<()>> {
    self.loads_forgotten.lock().unwrap().push(session.to_owned());
    self.loads.lock().unwrap().remove(session);

    Box::pin(async { Ok(()) })
  }

  fn turn_read<'a>(&'a self, session: &'a str, turn: i64) -> BoxFuture<'a, Answer<TurnRead>> {
    if let Some(carrier) = self.carrier() {
      return Box::pin(async move { carrier.turn_read(session, turn).await });
    }

    let read = self.turns.lock().unwrap().get(&turn).cloned();

    Box::pin(async move {
      read.ok_or_else(|| PortFailure::Answered {
        url: format!("http://agency.invalid/sessions/turns/{turn}"),
        status: 404,
        answer: Some(receiver_ports::WireError {
          code: "unknown-turn".to_owned(),
          error: "The session holds no such turn.".to_owned(),
        }),
      })
    })
  }

  fn kept_events(&self, after: Option<i64>) -> BoxFuture<'_, Answer<KeptEvents>> {
    if let Some(carrier) = self.carrier() {
      return Box::pin(async move { carrier.kept_events(after).await });
    }

    let items = self
      .kept
      .lock()
      .unwrap()
      .iter()
      .filter(|event| after.is_none_or(|after| event.id.is_some_and(|id| id > after)))
      .cloned()
      .collect();

    Box::pin(async move { Ok(KeptEvents { items, more: false }) })
  }

  fn events<'a>(&'a self, kinds: &'a [&'a str]) -> BoxFuture<'a, Answer<Events>> {
    if let Some(carrier) = self.carrier() {
      return Box::pin(async move { carrier.events(kinds).await });
    }

    if let Some(streams) = self.streams.lock().unwrap().as_mut() {
      let Some(events) = streams.pop_front() else {
        return agency_down("events");
      };

      return Box::pin(async move {
        let stream: Events = futures_util::stream::iter(events.into_iter().map(Ok)).boxed();

        Ok(stream)
      });
    }

    let Some(session) = self.standing_session() else {
      return not_asked_agency("the events");
    };

    Box::pin(async move { session.events().await })
  }

  fn push<'a>(&'a self, session: &'a str, turn: i64, words: &'a str) -> BoxFuture<'a, Answer<Said>> {
    self.pushes.lock().unwrap().push((session.to_owned(), turn, words.to_owned()));

    let said = *self.saying.lock().unwrap();
    let held = self.held_pushes.lock().unwrap().clone();

    Box::pin(async move {
      if let Some(held) = held {
        held.notified().await;
      }

      said.ok_or(PortFailure::Transport {
        url: "agency/session/push".to_owned(),
        why: "the stand-in says nothing".to_owned(),
      })
    })
  }

  fn add_supervisor<'a>(
    &'a self,
    added: &'a NewSupervisor,
  ) -> BoxFuture<'a, Answer<Result<Value, FieldRefusal>>> {
    self.added_rows.lock().unwrap().push(added.clone());

    if let Some(refusal) = self.refusing.lock().unwrap().clone() {
      return Box::pin(async move { Ok(Err(refusal)) });
    }

    let mut row = serde_json::Map::<String, Value>::new();

    row.insert("turnShape".to_owned(), serde_json::json!("pieces"));
    row.insert("seeded".to_owned(), serde_json::json!(false));
    row.insert("windowSeconds".to_owned(), serde_json::json!(180));

    self.agents.lock().unwrap().push(Agent {
      name: added.name.clone(),
      label: Some(added.label.clone()),
      awake: true,
      brain_folder: None,
      row,
    });

    let answer = serde_json::json!({ "supervisor": { "name": added.name }, "brain": "made" });

    Box::pin(async move { Ok(Ok(answer)) })
  }

  fn remove_supervisor<'a>(&'a self, name: &'a str, removal: &'a Removal) -> BoxFuture<'a, Answer<()>> {
    if let Some(code) = self.removal_refused.lock().unwrap().clone() {
      return Box::pin(async move {
        Err(PortFailure::Answered {
          url: "agency/supervisors".to_owned(),
          status: 409,
          answer: Some(WireError {
            code,
            error: "refused".to_owned(),
          }),
        })
      });
    }

    self.removals.lock().unwrap().push((name.to_owned(), removal.clone()));

    let mut held = self.agents.lock().unwrap();

    if let Some(at) = held.iter().position(|one| one.name == name) {
      held.remove(at);
    }

    Box::pin(async { Ok(()) })
  }

  fn reset_supervisor<'a>(&'a self, name: &'a str, reset: &'a Value) -> BoxFuture<'a, Answer<Value>> {
    self.resets.lock().unwrap().push((name.to_owned(), reset.clone()));

    let answer = serde_json::json!({ "agent": name, "rows": [] });

    Box::pin(async move { Ok(answer) })
  }
}

/// bmo-job: the questions of its jobs that wait in a room, the items it holds open and every answer it was asked
/// to settle.
pub struct StandInJob {
  pub open: Mutex<BTreeMap<i64, Vec<OpenQuestion>>>,
  pub settled: Mutex<Vec<Chosen>>,
  pub talked: Mutex<Vec<Talked>>,
  pub talk_refusal: Mutex<Option<String>>,
  pub moved: Mutex<Vec<Moved>>,
  pub items: Mutex<Vec<ItemRow>>,
}

impl StandInJob {
  pub fn new() -> Self {
    StandInJob {
      open: Mutex::default(),
      settled: Mutex::default(),
      talked: Mutex::default(),
      talk_refusal: Mutex::default(),
      moved: Mutex::default(),
      items: Mutex::default(),
    }
  }

  /// The open items bmo-job holds, as it keeps them.
  pub fn filing(&self, items: Vec<ItemRow>) {
    *self.items.lock().unwrap() = items.clone();
  }

  pub fn asking(&self, chat_id: i64, question: OpenQuestion) {
    self.open.lock().unwrap().insert(chat_id, [question].to_vec());
  }

  pub fn settled(&self) -> Vec<Chosen> {
    self.settled.lock().unwrap().clone()
  }

  /// Every later talk refused with the code given, as bmo-job refuses one; none written.
  pub fn refusing_talk(&self, code: &str) {
    *self.talk_refusal.lock().unwrap() = Some(code.to_owned());
  }

  pub fn talked(&self) -> Vec<Talked> {
    self.talked.lock().unwrap().clone()
  }

  pub fn moved(&self) -> Vec<Moved> {
    self.moved.lock().unwrap().clone()
  }
}

fn not_asked_job<T>(what: &str) -> BoxFuture<'_, Answer<T>> {
  panic!("a stand-in bmo-job was asked {what}, which no test sets")
}

impl Job for StandInJob {
  fn holdings(&self, _chat_id: i64) -> BoxFuture<'_, Answer<Vec<RoomHolding>>> {
    not_asked_job("a room's holdings")
  }

  fn items(&self) -> BoxFuture<'_, Answer<Vec<ItemRow>>> {
    Box::pin(async { Ok(self.items.lock().unwrap().clone()) })
  }

  fn open_questions(&self, chat_id: i64) -> BoxFuture<'_, Answer<Vec<OpenQuestion>>> {
    let open = self.open.lock().unwrap().get(&chat_id).cloned().unwrap_or_default();

    Box::pin(async move { Ok(open) })
  }

  fn settle<'a>(&'a self, chosen: &'a Chosen) -> BoxFuture<'a, Answer<()>> {
    self.settled.lock().unwrap().push(chosen.clone());

    Box::pin(async move { Ok(()) })
  }

  fn talk<'a>(&'a self, talked: &'a Talked) -> BoxFuture<'a, Answer<()>> {
    if let Some(code) = self.talk_refusal.lock().unwrap().clone() {
      return Box::pin(async move {
        Err(PortFailure::Answered {
          url: "http://job.test/question-talk".into(),
          status: 409,
          answer: Some(WireError {
            code,
            error: "The question refused the talk.".into(),
          }),
        })
      });
    }

    self.talked.lock().unwrap().push(talked.clone());

    Box::pin(async move { Ok(()) })
  }

  fn queue_moved<'a>(&'a self, moved: Option<&'a str>) -> BoxFuture<'a, Answer<()>> {
    self.moved.lock().unwrap().push(Moved {
      detail: moved.map(str::to_owned),
    });

    Box::pin(async move { Ok(()) })
  }
}

/// What the conversations were handed: each agent, its room and message, the files ahead of it, whether the
/// ground opens the room's window and whether the message was read through somebody else's account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Handed {
  pub agent: String,
  pub chat_id: i64,
  pub message_id: i64,
  pub files: Vec<i64>,
  pub opens_window: bool,
  pub read_through: Option<bool>,
}

/// The conversations, keeping every turn they were handed and opening the room's window where asked, as the
/// conversations of a room do.
/// Every agent's round stands as working in every room, so a send asked as an agent's turn is carried as its
/// own, unless the test lets the rounds stand idle.
pub struct Taken {
  db: DatabaseConnection,
  handed: Mutex<Vec<Handed>>,
  idle: std::sync::atomic::AtomicBool,
}

impl Taken {
  pub fn new(db: DatabaseConnection) -> Self {
    Taken {
      db,
      handed: Mutex::default(),
      idle: std::sync::atomic::AtomicBool::new(false),
    }
  }

  pub fn handed(&self) -> Vec<Handed> {
    self.handed.lock().unwrap().clone()
  }

  /// No agent's round works anywhere from now on.
  pub fn standing_idle(&self) {
    self.idle.store(true, std::sync::atomic::Ordering::SeqCst);
  }
}

impl Runner for Taken {
  fn hand<'a>(
    &'a self,
    agent: &'a str,
    files: &'a [Inbound],
    message: &'a Inbound,
    _room: Room,
    opens_window: bool,
    _window_only: bool,
  ) -> BoxFuture<'a, bool> {
    self.handed.lock().unwrap().push(Handed {
      agent: agent.to_owned(),
      chat_id: message.chat_id,
      message_id: message.message_id,
      files: files.iter().map(|file| file.message_id).collect(),
      opens_window,
      read_through: message.via_reader,
    });

    Box::pin(async move {
      if opens_window {
        let until = bmo_receiver::now_ms() + WINDOW * 1_000;

        bmo_receiver::receiver::wake::agents::open_window(&self.db, message.chat_id, agent, until)
          .await
          .ok();
      }

      true
    })
  }

  fn forget(&self, _chat_id: i64) -> BoxFuture<'_, ()> {
    Box::pin(async {})
  }

  fn in_round(&self, _agent: &str, _chat_id: i64) -> bool {
    !self.idle.load(std::sync::atomic::Ordering::SeqCst)
  }
}

/// The voice engine, keeping every briefing it was handed and answering a line's briefings from them, each in the
/// state the test gives it, else in the state the test sets for all; a stop ends every briefing not yet ended, once
/// the stops the test makes fail have failed. A briefing is taken only once the test lets go of `taking`.
pub struct StandInVoice {
  pub briefed: Mutex<Vec<NewBriefing>>,
  pub state: Mutex<String>,
  /// The state of each briefing the test or a stop gave one of its own, by its id.
  pub states: Mutex<BTreeMap<i64, String>>,
  /// Held by a test, every briefing handed over waits for it to be let go before it is taken.
  pub taking: tokio::sync::RwLock<()>,
  /// How many briefings were handed over, taken or still waiting on `taking`.
  pub handed: AtomicUsize,
  /// How many stops still fail before one is taken.
  pub failing_stops: Mutex<usize>,
  /// Every stop asked, in order, each with whether it was taken.
  pub stops: Mutex<Vec<bool>>,
}

impl StandInVoice {
  pub fn new() -> Self {
    Self {
      briefed: Mutex::default(),
      state: Mutex::new("waiting".to_owned()),
      states: Mutex::default(),
      taking: tokio::sync::RwLock::default(),
      handed: AtomicUsize::default(),
      failing_stops: Mutex::default(),
      stops: Mutex::default(),
    }
  }

  /// Every briefing taken, in order.
  pub fn briefed(&self) -> Vec<NewBriefing> {
    self.briefed.lock().unwrap().clone()
  }

  /// Where the briefing `id` stands: its own state where it has one, else the state set for all.
  pub fn state_of(&self, id: i64) -> String {
    let own = self.states.lock().unwrap().get(&id).cloned();

    own.unwrap_or_else(|| self.state.lock().unwrap().clone())
  }

  /// Every stop asked, in order, each with whether it was taken.
  pub fn stops(&self) -> Vec<bool> {
    self.stops.lock().unwrap().clone()
  }
}

/// The brain stand-in's number for every agent's one brain.
pub const BRAIN: i64 = 9;

/// The brain as a test sets it: every agent's brain is `BRAIN`, whether a consolidation is due is what the test
/// says, and every fold reported is kept, by the brain and the session key.
pub struct StandInBrain {
  pub due: Mutex<bool>,
  pub folds: Mutex<Vec<(i64, String)>>,
}

impl StandInBrain {
  pub fn new() -> Self {
    Self {
      due: Mutex::new(false),
      folds: Mutex::default(),
    }
  }

  /// Every fold reported, in order.
  pub fn folds(&self) -> Vec<(i64, String)> {
    self.folds.lock().unwrap().clone()
  }
}

impl BrainUnit for StandInBrain {
  fn due<'a>(&'a self, _agent: &'a str) -> BoxFuture<'a, Answer<Option<bool>>> {
    let due = *self.due.lock().unwrap();

    Box::pin(async move { Ok(Some(due)) })
  }

  fn fold<'a>(&'a self, _agent: &'a str, session_key: &'a str) -> BoxFuture<'a, Answer<Option<i64>>> {
    self.folds.lock().unwrap().push((BRAIN, session_key.to_owned()));

    Box::pin(async { Ok(Some(0)) })
  }
}

/// The listening tool as a test sets it: what it hears, or the reason it does not, and every path it was handed.
pub struct StandInHearing {
  answer: Mutex<Option<Result<HearingHeard, String>>>,
  asked: Mutex<Vec<PathBuf>>,
}

impl StandInHearing {
  pub fn new() -> Self {
    Self {
      answer: Mutex::new(Some(Ok(HearingHeard {
        transcript: "a voice note".to_owned(),
        language: "en".to_owned(),
        seconds: Some(1.0),
      }))),
      asked: Mutex::default(),
    }
  }

  /// What the next recording is heard as.
  pub fn saying(&self, transcript: &str, seconds: Option<f64>) {
    *self.answer.lock().unwrap() = Some(Ok(HearingHeard {
      transcript: transcript.to_owned(),
      language: "en".to_owned(),
      seconds,
    }));
  }

  /// The next recording it cannot hear, and why.
  pub fn failing(&self, why: &str) {
    *self.answer.lock().unwrap() = Some(Err(why.to_owned()));
  }

  /// Every recording path it was handed, in order.
  pub fn asked(&self) -> Vec<PathBuf> {
    self.asked.lock().unwrap().clone()
  }
}

impl Hearing for StandInHearing {
  fn transcribe<'a>(&'a self, path: &'a Path) -> BoxFuture<'a, Answer<HearingHeard>> {
    self.asked.lock().unwrap().push(path.to_path_buf());

    let answer = self.answer.lock().unwrap().clone();

    Box::pin(async move {
      match answer {
        Some(Ok(heard)) => Ok(heard),
        Some(Err(why)) => Err(PortFailure::Transport {
          url: "http://tool-host/tool-calls".to_owned(),
          why,
        }),
        None => Err(PortFailure::Transport {
          url: "http://tool-host/tool-calls".to_owned(),
          why: "the stand-in hearing was given no answer".to_owned(),
        }),
      }
    })
  }
}

impl VoiceEngine for StandInVoice {
  fn brief<'a>(&'a self, briefing: &'a NewBriefing) -> BoxFuture<'a, Answer<Briefing>> {
    Box::pin(async move {
      self.handed.fetch_add(1, Ordering::SeqCst);

      let _taking = self.taking.read().await;
      let mut briefed = self.briefed.lock().unwrap();

      briefed.push(briefing.clone());

      Ok(Briefing {
        id: briefed.len() as i64,
        kind: briefing.kind.clone(),
        state: "waiting".to_owned(),
      })
    })
  }

  fn of_line<'a>(
    &'a self,
    chat_id: i64,
    message_id: i64,
    kind: &'a str,
  ) -> BoxFuture<'a, Answer<Vec<Briefing>>> {
    Box::pin(async move {
      let briefed = self.briefed().into_iter();

      Ok(
        briefed
          .enumerate()
          .rev()
          .filter(|(_, one)| {
            one.chat_id == chat_id && one.message_id == Some(message_id) && one.kind == kind
          })
          .map(|(at, one)| Briefing {
            id: at as i64 + 1,
            kind: one.kind,
            state: self.state_of(at as i64 + 1),
          })
          .collect(),
      )
    })
  }

  fn stop_all(&self) -> BoxFuture<'_, Answer<Vec<Briefing>>> {
    Box::pin(async move {
      let mut failing = self.failing_stops.lock().unwrap();

      if *failing > 0 {
        *failing -= 1;
        self.stops.lock().unwrap().push(false);

        return Err(PortFailure::Transport {
          url: "http://voice/briefings/stop".to_owned(),
          why: "the stand-in engine fails this stop".to_owned(),
        });
      }

      drop(failing);
      self.stops.lock().unwrap().push(true);

      let stopped: Vec<Briefing> = self
        .briefed()
        .into_iter()
        .enumerate()
        .map(|(at, one)| Briefing {
          id: at as i64 + 1,
          kind: one.kind,
          state: self.state_of(at as i64 + 1),
        })
        .filter(|one| ["waiting", "speaking"].contains(&one.state.as_str()))
        .map(|one| Briefing {
          state: "stopped".to_owned(),
          ..one
        })
        .collect();
      let mut states = self.states.lock().unwrap();

      for one in &stopped {
        states.insert(one.id, one.state.clone());
      }

      Ok(stopped)
    })
  }
}

/// The cache warmer, keeping what the receiver registered and reported.
pub struct StandInWarmer {
  pub registered: Mutex<Vec<(String, String)>>,
  pub targets: Mutex<Vec<(String, Target)>>,
  pub uses: Mutex<Vec<(String, String, Used)>>,
}

impl StandInWarmer {
  pub fn new() -> Self {
    Self {
      registered: Mutex::default(),
      targets: Mutex::default(),
      uses: Mutex::default(),
    }
  }

  /// The consumer registrations, each by its name and hook URL.
  pub fn hooks(&self) -> Vec<(String, String)> {
    self.registered.lock().unwrap().clone()
  }

  /// The targets registered, each by its name and declaration.
  pub fn targets(&self) -> Vec<(String, Target)> {
    self.targets.lock().unwrap().clone()
  }

  /// Each use reported, in order.
  pub fn used(&self) -> Vec<(String, String, Used)> {
    self.uses.lock().unwrap().clone()
  }
}

impl Warmer for StandInWarmer {
  fn register<'a>(&'a self, consumer: &'a str, hook_url: &'a str) -> BoxFuture<'a, Answer<()>> {
    Box::pin(async {
      self.registered.lock().unwrap().push((consumer.to_owned(), hook_url.to_owned()));

      Ok(())
    })
  }

  fn target<'a>(
    &'a self,
    _consumer: &'a str,
    target: &'a str,
    declared: &'a Target,
  ) -> BoxFuture<'a, Answer<()>> {
    Box::pin(async {
      self.targets.lock().unwrap().push((target.to_owned(), declared.clone()));

      Ok(())
    })
  }

  fn used<'a>(&'a self, consumer: &'a str, target: &'a str, used: Used) -> BoxFuture<'a, Answer<()>> {
    Box::pin(async move {
      self.uses.lock().unwrap().push((consumer.to_owned(), target.to_owned(), used));

      Ok(())
    })
  }
}

/// A receiver over a database and a data root, reaching the chat services given and the stand-in agency, gate,
/// web unit and bmo-job, its news, its wake and the conversations it hands kept for the test to read.
pub struct Standing {
  pub receiver: Receiver,
  pub heard: Arc<Heard>,
  pub waking: Arc<WakeStandIn>,
  pub gate: Arc<StandInGate>,
  pub agency: Arc<StandInAgency>,
  pub job: Arc<StandInJob>,
  pub taken: Arc<Taken>,
  pub web: Arc<StandInWeb>,
  pub warmer: Arc<StandInWarmer>,
  pub voice: Arc<StandInVoice>,
  pub hearing: Arc<StandInHearing>,
  pub brain: Arc<StandInBrain>,
}

/// The same standing, with the receiver's own wake in place of the recording one: the rule every message is
/// judged by, so the turns the wake hands are kept in `taken`.
pub fn woken(mut standing: Standing) -> Standing {
  standing.receiver = standing.receiver.with_wake(Arc::new(bmo_receiver::receiver::wake::Waking));

  standing
}

pub fn standing(db: &DatabaseConnection, data_root: DataRoot, chats: Vec<Arc<StandInChat>>) -> Standing {
  standing_on(db, data_root, chats, deployment())
}

/// The same standing, on a deployment of the test's own, for a test that stands the infrastructure room or
/// another owner.
pub fn standing_on(
  db: &DatabaseConnection,
  data_root: DataRoot,
  chats: Vec<Arc<StandInChat>>,
  given: Deployment,
) -> Standing {
  let standing = unrun(db, data_root, chats, given);

  standing.receiver.hold_runner(standing.taken.clone());
  standing
}

/// The same standing with no conversations held: the first runner a test holds takes the turns the wake hands.
pub fn unrun(
  db: &DatabaseConnection,
  data_root: DataRoot,
  chats: Vec<Arc<StandInChat>>,
  given: Deployment,
) -> Standing {
  let heard = Arc::new(Heard::default());
  let waking = Arc::new(WakeStandIn::default());
  let gate = Arc::new(StandInGate::default());
  let web = Arc::new(StandInWeb::new());
  let agency = Arc::new(StandInAgency::new());
  let job = Arc::new(StandInJob::new());
  let taken = Arc::new(Taken::new(db.clone()));
  let warmer = Arc::new(StandInWarmer::new());
  let voice = Arc::new(StandInVoice::new());
  let hearing = Arc::new(StandInHearing::new());
  let brain = Arc::new(StandInBrain::new());
  let services = ChatServices::of(chats.into_iter().map(|chat| (chat.key.clone(), chat as Arc<dyn Chat>)));
  let units = Units {
    roads: Roads::new(services),
    agency: agency.clone(),
    gate: gate.clone(),
    job: job.clone(),
    web: web.clone(),
    warmer: warmer.clone(),
    voice: Some(voice.clone()),
    hearing: Some(hearing.clone()),
    brain: Some(brain.clone()),
  };
  // The stand-in agency's registrations move as a test sets them, so the receiver asks it on every read; a test of
  // the kept list keeps it itself.
  let receiver = Receiver::new(db.clone(), data_root, given, units)
    .keeping_agents_for(Duration::ZERO)
    .with_news(heard.clone())
    .with_wake(waking.clone())
    .with_situation_updates(false)
    .with_review_gate(false)
    .stopping_voice_every(Duration::from_millis(2));

  Standing {
    receiver,
    heard,
    waking,
    gate,
    agency,
    job,
    taken,
    web,
    warmer,
    voice,
    hearing,
    brain,
  }
}

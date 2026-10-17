//! In-memory stand-ins for the units a receiver reaches, each answering its port as a test sets it and keeping
//! what it was asked: a chat service, the gate, the web unit, where the receiver's news is heard, and the wake.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use bmo_chat_client::{
  ChatCapability, ChatDeclaration, ChatDialectDeclaration, ChatDialectLimits, ChatHealth, ChatMediaRequest,
  ChatMediaResult, ChatNumberSpace, ChatNumbering, ChatSendRequest, ChatSendResult, DialectChecked,
  DialectReading, RoomKind,
};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::{Deployment, How, Inbound, News, NotReceived, Receiver, Room, Units, Wake};
use bmo_receiver::roads::Roads;
use futures_util::future::BoxFuture;
use receiver_ports::chat::{Chat, ChatAnswer, ChatServices};
use receiver_ports::gate::{Entry, Envelope, Gate, Judgement, Sanitise, Sanitised};
use receiver_ports::http::Answer;
use receiver_ports::web::{Checked, Grant, MessageCheck, ObjectCheck, Render, RoomCheck, User, Web};
use sea_orm::DatabaseConnection;

/// The owner every receiver of the tests answers for.
pub const OWNER: i64 = 42;
/// The chat agent's registration name.
pub const AGENT: &str = "agent-a";
/// What the chat agent's own words are filed under.
pub const SENDER: &str = "Agent A";
/// The name a room calls the chat agent by.
pub const ADDRESS: &str = "agent-a";

/// A chat service: the rooms it holds and what it says of each, what it declares, and every send it was handed.
#[derive(Default)]
pub struct StandInChat {
  pub key: String,
  pub shows: Option<String>,
  pub owns: BTreeSet<i64>,
  pub owner_rooms: BTreeSet<i64>,
  pub identities: BTreeMap<i64, i64>,
  pub reaches_owner: Option<i64>,
  pub sends: Mutex<Vec<ChatSendRequest>>,
  /// What each send is answered with; sent as message 900 where none is set.
  pub answer: Mutex<Option<ChatSendResult>>,
}

impl StandInChat {
  pub fn new(key: &str) -> Self {
    Self {
      key: key.to_owned(),
      ..Self::default()
    }
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

  /// Every send handed over so far, in order.
  pub fn sent(&self) -> Vec<ChatSendRequest> {
    self.sends.lock().unwrap().clone()
  }
}

fn not_asked<T>(what: &str) -> BoxFuture<'_, ChatAnswer<T>> {
  panic!("a stand-in chat service was asked {what}, which no test sets")
}

impl Chat for StandInChat {
  fn declaration(&self) -> BoxFuture<'_, ChatAnswer<ChatDeclaration>> {
    let declared = ChatDeclaration {
      key: self.key.clone(),
      label: format!("the {} road", self.key),
      configured: None,
      modes: vec![],
      can: BTreeSet::from([ChatCapability::Text, ChatCapability::Delete, ChatCapability::Pin]),
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
      },
      speakers: false,
    };

    Box::pin(async move { Ok(declared) })
  }

  fn health(&self) -> BoxFuture<'_, ChatAnswer<ChatHealth>> {
    Box::pin(async { Ok(ChatHealth::ready()) })
  }

  fn send<'a>(&'a self, request: &'a ChatSendRequest) -> BoxFuture<'a, ChatAnswer<ChatSendResult>> {
    self.sends.lock().unwrap().push(request.clone());

    let answer = self.answer.lock().unwrap().clone().unwrap_or(ChatSendResult::Sent {
      message_id: Some(900),
      missed: vec![],
    });

    Box::pin(async move { Ok(answer) })
  }

  fn media<'a>(&'a self, _request: &'a ChatMediaRequest) -> BoxFuture<'a, ChatAnswer<ChatMediaResult>> {
    not_asked("for media")
  }

  fn dialect_read<'a>(&'a self, _text: &'a str) -> BoxFuture<'a, ChatAnswer<DialectReading>> {
    not_asked("to read its dialect")
  }

  fn dialect_check<'a>(&'a self, _markup: &'a str) -> BoxFuture<'a, ChatAnswer<DialectChecked>> {
    not_asked("to check its dialect")
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

  fn room_private_of(&self, _chat_id: i64) -> BoxFuture<'_, ChatAnswer<Option<i64>>> {
    Box::pin(async { Ok(None) })
  }

  fn room_title(&self, _chat_id: i64) -> BoxFuture<'_, ChatAnswer<Option<String>>> {
    Box::pin(async { Ok(None) })
  }

  fn owner_room(&self) -> BoxFuture<'_, ChatAnswer<Option<i64>>> {
    let reaches = self.reaches_owner;

    Box::pin(async move { Ok(reaches) })
  }
}

/// The gate: the sanitiser marks what it read, and nothing else is asked.
#[derive(Default)]
pub struct StandInGate {
  pub read: Mutex<Vec<String>>,
}

impl Gate for StandInGate {
  fn reference<'a>(&'a self, _envelope: &'a Envelope) -> BoxFuture<'a, Answer<Vec<Entry>>> {
    Box::pin(async { Ok(vec![]) })
  }

  fn sanitise<'a>(&'a self, asked: &'a Sanitise) -> BoxFuture<'a, Answer<Sanitised>> {
    self.read.lock().unwrap().push(asked.text.clone());

    Box::pin(async move {
      Ok(Sanitised {
        text: format!("{} (read)", asked.text),
        changed: true,
      })
    })
  }

  fn judge<'a>(&'a self, _judgement: &'a Judgement) -> BoxFuture<'a, Answer<()>> {
    Box::pin(async { Ok(()) })
  }
}

/// The web unit: a fixed line rendered as its code and the language asked; nothing else is asked.
#[derive(Default)]
pub struct StandInWeb;

fn unasked<T>() -> BoxFuture<'static, Answer<T>> {
  panic!("the stand-in web unit was asked what no test sets")
}

impl Web for StandInWeb {
  fn model_in_force(&self) -> BoxFuture<'_, Answer<bool>> {
    unasked()
  }

  fn user_of_sender(&self, _sender: i64) -> BoxFuture<'_, Answer<Option<String>>> {
    unasked()
  }

  fn user<'a>(&'a self, _user: &'a str) -> BoxFuture<'a, Answer<User>> {
    unasked()
  }

  fn check_room<'a>(&'a self, _check: &'a RoomCheck) -> BoxFuture<'a, Answer<Checked>> {
    unasked()
  }

  fn check_message<'a>(&'a self, _check: &'a MessageCheck) -> BoxFuture<'a, Answer<Checked>> {
    unasked()
  }

  fn check_object<'a>(&'a self, _check: &'a ObjectCheck) -> BoxFuture<'a, Answer<Checked>> {
    unasked()
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
}

/// Where the receiver's news is heard: each message told, in order.
#[derive(Default)]
pub struct Heard(pub Mutex<Vec<(i64, i64)>>);

impl Heard {
  pub fn told(&self) -> Vec<(i64, i64)> {
    self.0.lock().unwrap().clone()
  }
}

impl News for Heard {
  fn message(&self, chat_id: i64, message_id: i64) {
    self.0.lock().unwrap().push((chat_id, message_id));
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
pub struct Waking(pub Mutex<Vec<Woken>>);

impl Waking {
  pub fn woken(&self) -> Vec<Woken> {
    self.0.lock().unwrap().clone()
  }
}

impl Wake for Waking {
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
    delegate: Some("agent-b".to_owned()),
    infra_room: None,
    language: "en".to_owned(),
    attachment_bytes: Some(CAP),
  }
}

/// The attachment size cap of the tests' deployment, in bytes.
pub const CAP: u64 = 1024;

/// A receiver over a database and a data root, reaching the chat services given and the stand-in gate and web
/// unit, its news and its wake kept for the test to read.
pub struct Standing {
  pub receiver: Receiver,
  pub heard: Arc<Heard>,
  pub waking: Arc<Waking>,
  pub gate: Arc<StandInGate>,
}

pub fn standing(db: &DatabaseConnection, data_root: DataRoot, chats: Vec<Arc<StandInChat>>) -> Standing {
  let heard = Arc::new(Heard::default());
  let waking = Arc::new(Waking::default());
  let gate = Arc::new(StandInGate::default());
  let services = ChatServices::of(chats.into_iter().map(|chat| (chat.key.clone(), chat as Arc<dyn Chat>)));
  let units = Units {
    roads: Roads::new(services),
    gate: gate.clone(),
    web: Arc::new(StandInWeb),
  };
  let receiver = Receiver::new(db.clone(), data_root, deployment(), units)
    .with_news(heard.clone())
    .with_wake(waking.clone());

  Standing {
    receiver,
    heard,
    waking,
    gate,
  }
}

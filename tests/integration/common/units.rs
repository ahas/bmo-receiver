//! In-memory stand-ins for the units a receiver reaches, each answering its port as a test sets it and keeping
//! what it was asked: a chat service, the gate, the web unit, where the receiver's news is heard, and the wake.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use bmo_chat_client::{
  ChatCapability, ChatDeclaration, ChatDialectDeclaration, ChatDialectLimits, ChatHealth, ChatMediaRequest,
  ChatNumberRanges, ChatNumberSpace, ChatNumbering, ChatSendRequest, ChatSent, DialectChecked,
  DialectReading, MessageFile, Page, RoomKind,
};
use bmo_receiver::data_root::DataRoot;
use bmo_receiver::receiver::{
  Deployment, How, Inbound, News, NotReceived, Receiver, Room, Runner, Units, Wake,
};
use bmo_receiver::roads::Roads;
use futures_util::future::BoxFuture;
use receiver_ports::agency::{
  Agency, Agent, Agents, CompanionRemoval, CompanionRow, Delivery, Events, FieldRefusal, Forgotten, Passed,
  Rest, RestKind, Resting, Run, RunLine, Said, Scope, Session, SessionState, Turn, Written,
};
use receiver_ports::brain::{
  BeforeFold, Brain, Load, Reading, Refused, Reminder, Reminders, RoomRule, Row, RuleText, Took,
};
use receiver_ports::chat::{Chat, ChatAnswer, ChatServices};
use receiver_ports::gate::{Entry, Envelope, Gate, Judgement, Sanitise, Sanitised};
use receiver_ports::http::{Answer, PortFailure, WireError};
use receiver_ports::job::{Chosen, ItemRow, Job, Moved, OpenQuestion, RoomHolding, Talked};
use receiver_ports::voice::{Briefed, Briefing, Voice};
use receiver_ports::warmer::{Target, Used, Warmer};
use receiver_ports::web::{
  Checked, Grant, GrantRow, MessageCheck, ObjectCheck, Render, RoomCheck, User, Web,
};
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};

use crate::common::session::StandInSession;

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
  pub private: BTreeMap<i64, i64>,
  pub reaches_owner: Option<i64>,
  pub sends: Mutex<Vec<ChatSendRequest>>,
  /// What each send is answered with; sent as message 900 where none is set.
  pub answer: Mutex<Option<ChatSent>>,
  // The next send, held until the test lets it go, and whether one stands held now.
  held_send: Mutex<Option<Arc<tokio::sync::Notify>>>,
  holding_send: Mutex<bool>,
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

  /// A room of the service's own that is one person's private chat with the chat agent.
  pub fn privating(mut self, room: i64, person: i64) -> Self {
    self.owns.insert(room);
    self.private.insert(room, person);
    self
  }

  /// Every send carried through so far, in order.
  pub fn sent(&self) -> Vec<ChatSendRequest> {
    self.sends.lock().unwrap().clone()
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
    let held = self.held_send.lock().unwrap().take();
    let answer = self.answer.lock().unwrap().clone().unwrap_or(ChatSent {
      message_id: Some(900),
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
    _request: &'a ChatMediaRequest,
  ) -> BoxFuture<'a, ChatAnswer<Page<MessageFile>>> {
    not_asked("for a message's files")
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

  fn room_private_of(&self, chat_id: i64) -> BoxFuture<'_, ChatAnswer<Option<i64>>> {
    let person = self.private.get(&chat_id).copied();

    Box::pin(async move { Ok(person) })
  }

  fn room_title(&self, _chat_id: i64) -> BoxFuture<'_, ChatAnswer<Option<String>>> {
    Box::pin(async { Ok(None) })
  }

  fn owner_room(&self) -> BoxFuture<'_, ChatAnswer<Option<i64>>> {
    let reaches = self.reaches_owner;

    Box::pin(async move { Ok(reaches) })
  }
}

/// The gate: the sanitiser marks what it read, every attach of a round's judgements asked is kept, each the room,
/// the message and the round's start, and every copy of gate options asked is kept, each the agent it is for and
/// the agent copied from.
#[derive(Default)]
pub struct StandInGate {
  pub read: Mutex<Vec<String>>,
  pub attached: Mutex<Vec<(i64, i64, i64)>>,
  pub copied: Mutex<Vec<(String, String)>>,
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

  fn attach<'a>(&'a self, chat: i64, message: i64, since: i64) -> BoxFuture<'a, Answer<()>> {
    self.attached.lock().unwrap().push((chat, message, since));

    Box::pin(async { Ok(()) })
  }

  fn copy_options<'a>(&'a self, agent: &'a str, from: &'a str) -> BoxFuture<'a, Answer<()>> {
    self.copied.lock().unwrap().push((agent.to_owned(), from.to_owned()));

    Box::pin(async { Ok(()) })
  }
}

/// The brain: a session's loaded documents, kept and forgotten, and nothing else asked.
pub struct StandInBrain {
  pub loads: Mutex<BTreeMap<String, Vec<Load>>>,
}

impl StandInBrain {
  pub fn new() -> StandInBrain {
    StandInBrain {
      loads: Mutex::default(),
    }
  }

  pub fn loaded(&self) -> BTreeMap<String, Vec<Load>> {
    self.loads.lock().unwrap().clone()
  }
}

fn not_asked_brain<T>(what: &str) -> BoxFuture<'_, Answer<T>> {
  panic!("a stand-in brain was asked {what}, which no test sets")
}

impl Brain for StandInBrain {
  fn room_rules(&self, _chat_id: i64) -> BoxFuture<'_, Answer<Vec<RoomRule>>> {
    Box::pin(async { Ok(vec![]) })
  }

  fn loads<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<Vec<Load>>> {
    let kept = self.loads.lock().unwrap().get(session).cloned().unwrap_or_default();

    Box::pin(async move { Ok(kept) })
  }

  fn keep_loads<'a>(&'a self, session: &'a str, loads: &'a [Load], _at: i64) -> BoxFuture<'a, Answer<()>> {
    let mut kept = self.loads.lock().unwrap();
    let held = kept.entry(session.to_owned()).or_default();

    for load in loads {
      held.retain(|one| one.path != load.path);
      held.push(load.clone());
    }

    drop(kept);

    Box::pin(async { Ok(()) })
  }

  fn forget_loads<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<()>> {
    self.loads.lock().unwrap().remove(&session.to_owned());

    Box::pin(async { Ok(()) })
  }

  fn set_room_rule<'a>(
    &'a self,
    _chat: i64,
    _message: i64,
    _id: &'a str,
    _rule: Option<&'a RuleText>,
  ) -> BoxFuture<'a, Answer<Refused<Vec<RoomRule>>>> {
    Box::pin(async { Ok(Ok(vec![])) })
  }

  fn set_reminder<'a>(&'a self, _reminder: &'a Reminder) -> BoxFuture<'a, Answer<Refused<Reading>>> {
    Box::pin(async { Ok(Ok(Reading::default())) })
  }

  fn cancel_reminder(&self, _id: i64) -> BoxFuture<'_, Answer<Refused<Reading>>> {
    Box::pin(async { Ok(Ok(Reading::default())) })
  }

  fn reminders<'a>(
    &'a self,
    _chat: i64,
    _agent: Option<&'a str>,
    _all: bool,
  ) -> BoxFuture<'a, Answer<Reminders>> {
    Box::pin(async {
      Ok(Reminders {
        reminders: vec![],
        lines: vec![],
      })
    })
  }

  fn session_summary<'a>(&'a self, _agent: &'a str) -> BoxFuture<'a, Answer<Option<String>>> {
    Box::pin(async { Ok(None) })
  }

  fn clear_session_summary<'a>(&'a self, _agent: &'a str) -> BoxFuture<'a, Answer<()>> {
    Box::pin(async { Ok(()) })
  }

  fn people_before_fold<'a>(&'a self, _chat: i64, _agent: &'a str) -> BoxFuture<'a, Answer<BeforeFold>> {
    not_asked_brain("the people before a fold")
  }

  fn memories<'a>(
    &'a self,
    _agent: Option<&'a str>,
    _chat: Option<i64>,
  ) -> BoxFuture<'a, Answer<Vec<Row>>> {
    Box::pin(async { Ok(vec![]) })
  }

  fn notes<'a>(&'a self, _agent: &'a str, _chat: Option<i64>) -> BoxFuture<'a, Answer<Vec<Row>>> {
    Box::pin(async { Ok(vec![]) })
  }

  fn watches<'a>(
    &'a self,
    _by: Option<i64>,
    _agent: Option<&'a str>,
    _all: bool,
  ) -> BoxFuture<'a, Answer<Vec<Row>>> {
    Box::pin(async { Ok(vec![]) })
  }

  fn room_forgotten(&self, _chat: i64) -> BoxFuture<'_, Answer<()>> {
    Box::pin(async { Ok(()) })
  }

  fn room_removed(&self, _chat: i64, _preview: bool) -> BoxFuture<'_, Answer<Took>> {
    Box::pin(async { Ok(Took::default()) })
  }
}

/// The voice unit: a briefing launched and kept, its line let go; nothing else asked.
pub struct StandInVoice {
  pub launched: Mutex<Vec<Briefing>>,
  pub kept: Mutex<Vec<Briefed>>,
  pub lines: Mutex<Vec<(String, i64)>>,
}

impl StandInVoice {
  pub fn new() -> StandInVoice {
    StandInVoice {
      launched: Mutex::default(),
      kept: Mutex::default(),
      lines: Mutex::default(),
    }
  }

  pub fn launched(&self) -> Vec<Briefing> {
    self.launched.lock().unwrap().clone()
  }

  /// The briefings it keeps of a room, as the readings it has launched.
  pub fn keeping(&self, kept: Vec<Briefed>) {
    *self.kept.lock().unwrap() = kept.clone();
  }
}

impl Voice for StandInVoice {
  fn brief<'a>(&'a self, briefing: &'a Briefing) -> BoxFuture<'a, Answer<String>> {
    self.launched.lock().unwrap().push(briefing.clone());

    Box::pin(async { Ok("briefing-run".to_owned()) })
  }

  fn briefings(&self, chat: i64, _awaiting: bool) -> BoxFuture<'_, Answer<Vec<Briefed>>> {
    let kept = self
      .kept
      .lock()
      .unwrap()
      .iter()
      .filter(|one| one.chat == chat)
      .cloned()
      .collect::<Vec<Briefed>>();

    Box::pin(async { Ok(kept) })
  }

  fn line<'a>(&'a self, run: &'a str, message: i64) -> BoxFuture<'a, Answer<()>> {
    self.lines.lock().unwrap().push((run.to_owned(), message));

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
}

impl StandInWeb {
  pub fn new() -> Self {
    StandInWeb {
      users: Mutex::default(),
      grants: Mutex::default(),
      model: Mutex::default(),
      admins: Mutex::default(),
    }
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
    delegate: Some("agent-b".to_owned()),
    infra_room: None,
    language: "en".to_owned(),
    attachment_bytes: Some(CAP),
    wording_document: None,
  }
}

/// The attachment size cap of the tests' deployment, in bytes.
pub const CAP: u64 = 1024;

/// The length of window the stand-in conversations open when a turn's ground does, in seconds.
pub const WINDOW: i64 = 60;

/// An agent as the agency registers it.
pub fn registration(
  name: &str,
  label: &str,
  words: Vec<&str>,
  window: Option<i64>,
  default: bool,
) -> Agent {
  let mut row = serde_json::Map::<String, Value>::new();

  row.insert("wake_words".to_owned(), serde_json::json!(words.join(",")));
  row.insert("default_answerer".to_owned(), serde_json::json!(if default { 1 } else { 0 }));

  if let Some(window) = window {
    row.insert("window_seconds".to_owned(), serde_json::json!(window));
  }

  Agent {
    name: name.to_owned(),
    label: Some(label.to_owned()),
    awake: true,
    brain_folder: None,
    row,
  }
}

/// The agency: the agents it registers, whether a stand-in answers an agent while the rest closes its road,
/// the deployment's rest, every companion removal it was asked, the field it refuses a companion's call on
/// where a test sets one, every push it was asked with what it says of it (a push failing where it says
/// nothing), and the session its turns go on where a test stands one.
pub struct StandInAgency {
  pub session: Mutex<Option<Arc<StandInSession>>>,
  pub removals: Mutex<Vec<CompanionRemoval>>,
  pub refusing: Mutex<Option<FieldRefusal>>,
  pub pushes: Mutex<Vec<(String, String)>>,
  pub saying: Mutex<Option<Said>>,
  pub agents: Mutex<Vec<Agent>>,
  pub stand_ins: Mutex<BTreeSet<String>>,
  pub rest: Mutex<Rest>,
  pub written: Mutex<Vec<Resting>>,
  pub passed: Mutex<Option<RestKind>>,
  // The rooms it was asked to forget, each with whether the asking was a preview.
  pub forgotten: Mutex<Vec<(i64, bool)>>,
  // Every agent's reset point, in epoch milliseconds, where a test moved it.
  pub point: Mutex<Option<i64>>,
  // The runs it holds, each its id and the moment it began, in epoch seconds.
  pub runs: Mutex<BTreeMap<i64, i64>>,
  // The sessions it lists, each with its state, where a test sets them.
  pub listed: Mutex<Option<Vec<(Session, SessionState)>>>,
  // Whether it answers nothing of its agents and sessions, as an agency that is down.
  pub down: Mutex<bool>,
}

impl StandInAgency {
  pub fn new() -> Self {
    StandInAgency {
      session: Mutex::default(),
      removals: Mutex::default(),
      refusing: Mutex::default(),
      pushes: Mutex::default(),
      saying: Mutex::default(),
      agents: Mutex::default(),
      stand_ins: Mutex::default(),
      rest: Mutex::new(Rest {
        resting: false,
        until: None,
        kind: None,
      }),
      written: Mutex::default(),
      passed: Mutex::default(),
      forgotten: Mutex::default(),
      point: Mutex::default(),
      runs: Mutex::default(),
      listed: Mutex::default(),
      down: Mutex::default(),
    }
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

  pub fn standing_in(&self, agents: Vec<String>) {
    *self.stand_ins.lock().unwrap() =
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
  fn agents(&self) -> BoxFuture<'_, Answer<Agents>> {
    if self.is_down() {
      return agency_down("agents");
    }

    Box::pin(async move {
      Ok(Agents {
        agents: self.agents.lock().unwrap().clone(),
        roles: vec![],
      })
    })
  }

  fn stand_in<'a>(&'a self, agent: &'a str) -> BoxFuture<'a, Answer<bool>> {
    let answers = self.stand_ins.lock().unwrap().contains(agent);

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
    let Some(session) = self.standing_session() else { return not_asked_agency("a turn") };

    Box::pin(async move { session.turn(turn).await })
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

  fn interrupt<'a>(&'a self, _session: &'a str) -> BoxFuture<'a, Answer<()>> {
    not_asked_agency("an interrupt")
  }

  fn fold<'a>(&'a self, _session: &'a str) -> BoxFuture<'a, Answer<()>> {
    not_asked_agency("a fold")
  }

  fn end<'a>(&'a self, _session: &'a str) -> BoxFuture<'a, Answer<()>> {
    not_asked_agency("an end")
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
    not_asked_agency("a hook")
  }

  fn run_log<'a>(&'a self, _run: &'a str, _after: Option<i64>) -> BoxFuture<'a, Answer<Vec<RunLine>>> {
    let Some(session) = self.standing_session() else {
      return not_asked_agency("a run's log");
    };

    Box::pin(async move { session.run_log().await })
  }

  fn reset_point<'a>(&'a self, _agent: &'a str) -> BoxFuture<'a, Answer<Option<i64>>> {
    let point = *self.point.lock().unwrap();

    Box::pin(async move { Ok(point) })
  }

  fn run(&self, id: i64) -> BoxFuture<'_, Answer<Option<Run>>> {
    let run = self.runs.lock().unwrap().get(&id).map(|started_at| Run {
      started_at: *started_at,
    });

    Box::pin(async move { Ok(run) })
  }

  fn events<'a>(&'a self, _kinds: &'a [&'a str]) -> BoxFuture<'a, Answer<Events>> {
    let Some(session) = self.standing_session() else {
      return not_asked_agency("the events");
    };

    Box::pin(async move { session.events().await })
  }

  fn push<'a>(&'a self, session: &'a str, words: &'a str) -> BoxFuture<'a, Answer<Said>> {
    self.pushes.lock().unwrap().push((session.to_owned(), words.to_owned()));

    let said = *self.saying.lock().unwrap();

    Box::pin(async move {
      said.ok_or(PortFailure::Transport {
        url: "agency/session/push".to_owned(),
        why: "the stand-in says nothing".to_owned(),
      })
    })
  }

  fn add_companion<'a>(
    &'a self,
    companion: &'a CompanionRow,
  ) -> BoxFuture<'a, Answer<Result<(), FieldRefusal>>> {
    if let Some(refusal) = self.refusing.lock().unwrap().clone() {
      return Box::pin(async move { Ok(Err(refusal)) });
    }

    let mut row = serde_json::Map::<String, Value>::new();

    row.insert("kind".to_owned(), serde_json::json!("chat"));
    row.insert("turn_shape".to_owned(), serde_json::json!("pieces"));
    row.insert("builtin".to_owned(), serde_json::json!(0));
    row.insert("window_seconds".to_owned(), serde_json::json!(180));

    self.agents.lock().unwrap().push(Agent {
      name: companion.name.clone(),
      label: Some(companion.label.clone()),
      awake: true,
      brain_folder: None,
      row,
    });

    Box::pin(async { Ok(Ok(())) })
  }

  fn remove_companion<'a>(
    &'a self,
    removal: &'a CompanionRemoval,
  ) -> BoxFuture<'a, Answer<Result<(), FieldRefusal>>> {
    if let Some(refusal) = self.refusing.lock().unwrap().clone() {
      return Box::pin(async move { Ok(Err(refusal)) });
    }

    self.removals.lock().unwrap().push(removal.clone());

    let mut held = self.agents.lock().unwrap();

    if let Some(at) = held.iter().position(|one| one.name == removal.agent) {
      held.remove(at);
    }

    Box::pin(async { Ok(Ok(())) })
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
pub struct Taken {
  db: DatabaseConnection,
  handed: Mutex<Vec<Handed>>,
}

impl Taken {
  pub fn new(db: DatabaseConnection) -> Self {
    Taken {
      db,
      handed: Mutex::default(),
    }
  }

  pub fn handed(&self) -> Vec<Handed> {
    self.handed.lock().unwrap().clone()
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
        let until = bmo_receiver::now_ms().div_euclid(1000) + WINDOW;

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
  pub brain: Arc<StandInBrain>,
  pub job: Arc<StandInJob>,
  pub taken: Arc<Taken>,
  pub voice: Arc<StandInVoice>,
  pub web: Arc<StandInWeb>,
  pub warmer: Arc<StandInWarmer>,
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
  let brain = Arc::new(StandInBrain::new());
  let voice = Arc::new(StandInVoice::new());
  let warmer = Arc::new(StandInWarmer::new());
  let services = ChatServices::of(chats.into_iter().map(|chat| (chat.key.clone(), chat as Arc<dyn Chat>)));
  let units = Units {
    roads: Roads::new(services),
    agency: agency.clone(),
    brain: brain.clone(),
    gate: gate.clone(),
    job: job.clone(),
    voice: voice.clone(),
    web: web.clone(),
    warmer: warmer.clone(),
  };
  let receiver = Receiver::new(db.clone(), data_root, given, units)
    .with_news(heard.clone())
    .with_wake(waking.clone());

  Standing {
    receiver,
    heard,
    waking,
    gate,
    agency,
    brain,
    job,
    taken,
    voice,
    web,
    warmer,
  }
}

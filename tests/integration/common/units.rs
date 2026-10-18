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
use bmo_receiver::receiver::{
  Deployment, How, Inbound, News, NotReceived, Receiver, Room, Runner, Units, Wake,
};
use bmo_receiver::roads::Roads;
use futures_util::future::BoxFuture;
use receiver_ports::agency::{
  Agency, Agent, Agents, Delivery, Events, Forgotten, Passed, Rest, RestKind, Resting, RunLine, Scope,
  Session, SessionState, Turn, Written,
};
use receiver_ports::brain::{
  BeforeFold, Brain, Load, Reading, Refused, Reminder, Reminders, RoomRule, Row, RuleText, Took,
};
use receiver_ports::chat::{Chat, ChatAnswer, ChatServices};
use receiver_ports::gate::{Entry, Envelope, Gate, Judgement, Sanitise, Sanitised};
use receiver_ports::http::Answer;
use receiver_ports::job::{Chosen, ItemRow, Job, Moved, OpenQuestion, RoomHolding, Talked};
use receiver_ports::voice::{Briefed, Briefing, Voice};
use receiver_ports::web::{
  Checked, Grant, GrantRow, MessageCheck, ObjectCheck, Render, RoomCheck, User, Web,
};
use sea_orm::DatabaseConnection;
use serde_json::Value;

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

  /// A room of the service's own that is one person's private chat with the chat agent.
  pub fn privating(mut self, room: i64, person: i64) -> Self {
    self.owns.insert(room);
    self.private.insert(room, person);
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
    self.loads.lock().unwrap().insert(session.to_owned(), loads.to_vec());

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
}

impl Voice for StandInVoice {
  fn brief<'a>(&'a self, briefing: &'a Briefing) -> BoxFuture<'a, Answer<String>> {
    self.launched.lock().unwrap().push(briefing.clone());

    Box::pin(async { Ok("briefing-run".to_owned()) })
  }

  fn briefings(&self, _chat: i64, _awaiting: bool) -> BoxFuture<'_, Answer<Vec<Briefed>>> {
    Box::pin(async { Ok(vec![]) })
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
}

impl StandInWeb {
  pub fn new() -> Self {
    StandInWeb {
      users: Mutex::default(),
      grants: Mutex::default(),
      model: Mutex::default(),
    }
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

  fn user<'a>(&'a self, _user: &'a str) -> BoxFuture<'a, Answer<User>> {
    unasked()
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
/// and the deployment's rest.
pub struct StandInAgency {
  pub agents: Mutex<Vec<Agent>>,
  pub stand_ins: Mutex<BTreeSet<String>>,
  pub rest: Mutex<Rest>,
  pub written: Mutex<Vec<Resting>>,
  pub passed: Mutex<Option<RestKind>>,
}

impl StandInAgency {
  pub fn new() -> Self {
    StandInAgency {
      agents: Mutex::default(),
      stand_ins: Mutex::default(),
      rest: Mutex::new(Rest {
        resting: false,
        until: None,
        kind: None,
      }),
      written: Mutex::default(),
      passed: Mutex::default(),
    }
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
}

fn not_asked_agency<T>(what: &str) -> BoxFuture<'_, Answer<T>> {
  panic!("a stand-in agency was asked {what}, which no test sets")
}

impl Agency for StandInAgency {
  fn agents(&self) -> BoxFuture<'_, Answer<Agents>> {
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

  fn start<'a>(&'a self, _agent: &'a str, _scope: Scope) -> BoxFuture<'a, Answer<Session>> {
    not_asked_agency("a session")
  }

  fn turn<'a>(&'a self, _turn: &'a Turn) -> BoxFuture<'a, Answer<Delivery>> {
    not_asked_agency("a turn")
  }

  fn state<'a>(&'a self, _session: &'a str) -> BoxFuture<'a, Answer<SessionState>> {
    not_asked_agency("a session's state")
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
    not_asked_agency("its sessions")
  }

  fn forget_room(&self, _chat_id: i64, _dry_run: bool) -> BoxFuture<'_, Answer<Forgotten>> {
    not_asked_agency("a room forgotten")
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
    not_asked_agency("a run's log")
  }

  fn reset_point<'a>(&'a self, _agent: &'a str) -> BoxFuture<'a, Answer<Option<i64>>> {
    Box::pin(async { Ok(None) })
  }

  fn events<'a>(&'a self, _kinds: &'a [&'a str]) -> BoxFuture<'a, Answer<Events>> {
    not_asked_agency("the events")
  }
}

/// bmo-job: the questions of its jobs that wait in a room, the items it holds open and every answer it was asked
/// to settle.
pub struct StandInJob {
  pub open: Mutex<BTreeMap<i64, Vec<OpenQuestion>>>,
  pub settled: Mutex<Vec<Chosen>>,
  pub talked: Mutex<Vec<Talked>>,
  pub moved: Mutex<Vec<Moved>>,
  pub items: Mutex<Vec<ItemRow>>,
}

impl StandInJob {
  pub fn new() -> Self {
    StandInJob {
      open: Mutex::default(),
      settled: Mutex::default(),
      talked: Mutex::default(),
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
}

/// The same standing, with the receiver's own wake in place of the recording one: the rule every message is
/// judged by, so the turns the wake hands are kept in `taken`.
pub fn woken(mut standing: Standing) -> Standing {
  standing.receiver = standing.receiver.with_wake(Arc::new(bmo_receiver::receiver::wake::Waking));

  standing
}

pub fn standing(db: &DatabaseConnection, data_root: DataRoot, chats: Vec<Arc<StandInChat>>) -> Standing {
  let heard = Arc::new(Heard::default());
  let waking = Arc::new(WakeStandIn::default());
  let gate = Arc::new(StandInGate::default());
  let web = Arc::new(StandInWeb::new());
  let agency = Arc::new(StandInAgency::new());
  let job = Arc::new(StandInJob::new());
  let taken = Arc::new(Taken::new(db.clone()));
  let brain = Arc::new(StandInBrain::new());
  let voice = Arc::new(StandInVoice::new());
  let services = ChatServices::of(chats.into_iter().map(|chat| (chat.key.clone(), chat as Arc<dyn Chat>)));
  let units = Units {
    roads: Roads::new(services),
    agency: agency.clone(),
    brain: brain.clone(),
    gate: gate.clone(),
    job: job.clone(),
    voice: voice.clone(),
    web: web.clone(),
  };
  let receiver = Receiver::new(db.clone(), data_root, deployment(), units)
    .with_news(heard.clone())
    .with_wake(waking.clone());

  receiver.hold_runner(taken.clone());

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
  }
}

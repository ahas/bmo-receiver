//! The conversations of one conversational agent, every room's in one (docs/wake.md; docs/bmo-window.md): what a
//! room says joins its queue and is handed over together once the queue's delay is up; the room's conversation
//! wakes, or takes the words into the round it is running. A round reads what is waiting, asks the session once,
//! holds the answer to its checks, and sends it — or waits for more, or passes the room by — and the room's window
//! keeps it listening without being called while it stands.

use std::collections::{HashMap, HashSet};
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use tokio::task::JoinHandle;

use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::fill_marks;
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};

use super::again::Checker;
use super::ask::Carry;
use super::context::{remember_held, sent_prompt};
use super::health::{FailureOrders, RoomHealth};
use super::mid_turn::{Paragraph, Push};
use super::prompt::{self, Round, Scope as RoundScope};
use super::reply::Sender;
use super::room::room_context;
use super::sentinels::{is_skip, is_wait};
use super::talk::{TalkFailure, TalkRequest};
use super::touch::TouchAnswer;
use super::turnover::Noter;
use super::window;
use super::window::Selection;

use crate::Fault;
use crate::companion::turn::Turn as CompanionTurn;
use crate::now_ms;
use crate::receiver::{Inbound, Receiver, Room, rows, wake};
use crate::rounds;
use crate::{one, run};
use receiver_codes::SessionReason;
use receiver_ports::agency::Said;
use receiver_ports::gate::{Judgement, TurnEvent};

/// A hook's answer, which it may take its time over.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// How long the room queue gathers a room's words, how long a wait holds them, how long the chat agent's
/// window keeps a room listening where its row says nothing else, and how often a working turn is looked at
/// for words that came for its room.
#[derive(Clone, Copy, Debug)]
pub struct Timings {
  pub queue: Duration,
  pub wait: Duration,
  pub window: Option<Duration>,
  pub look: Duration,
}

impl Timings {
  /// The round policy's, the window the chat agent's row keeps.
  pub fn of_policy() -> Self {
    let held = &CHAT_ROUND.conversation;

    Self {
      queue: Duration::from_millis(held.queue_ms),
      wait: Duration::from_millis(held.wait_ms),
      window: None,
      look: Duration::from_millis(CHAT_ROUND.pushing.look_ms),
    }
  }
}

/// What a round asks of what stands around the conversations: the rows that ride a room's next round, and a room
/// shown typing.
pub trait Hooks: Send + Sync + 'static {
  fn riders(&self, agent: &str, chat_id: i64) -> BoxFuture<'_, ()>;
  fn typing(&self, chat_id: i64, on: bool) -> BoxFuture<'_, ()>;
}

/// Why a round's turn did not answer: it was stopped, the road closed it for a limit or an outage, it failed
/// outright, or the session's engine ended it with an error, of the kind the engine named where it named one.
/// The agency carries the turn and answers with its fault; the rest of the road's reason is the agency's to tell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
  Stopped,
  Limit,
  Outage,
  Transport(String),
  Failed(String),
  Engine { reason: String, kind: Option<String> },
}

impl Failure {
  /// The failure as the turn's line says it.
  pub fn reason(&self) -> String {
    match self {
      Failure::Stopped => "stopped".to_owned(),
      Failure::Limit => "limit".to_owned(),
      Failure::Outage => "outage".to_owned(),
      Failure::Transport(why) | Failure::Failed(why) | Failure::Engine { reason: why, .. } => why.clone(),
    }
  }
}

/// A turn's answer as the round reads it: its words, the mode it answered in, the session it went out on, what
/// the session now holds against the window it holds it in, and the run whose log holds its calls.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reply {
  pub text: String,
  pub mode: Option<String>,
  pub account: Option<String>,
  pub session: Option<String>,
  pub context_tokens: Option<i64>,
  pub context_window: Option<i64>,
  pub replaced: Vec<SessionReason>,
  pub run: Option<String>,
}

impl Reply {
  /// The share of its window the session's context now fills, where the agency told both.
  pub fn context_ratio(&self) -> Option<f64> {
    match (self.context_tokens, self.context_window) {
      (Some(tokens), Some(window)) if window > 0 => Some(tokens as f64 / window as f64),
      _ => None,
    }
  }
}

/// What a conversation keeps of the session its room's turns go on: the session it resumes, why it was replaced,
/// what it holds of its context, the message its turns have read up to and whether every word it took was the
/// owner's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Held {
  pub agent: String,
  pub chat_id: i64,
  pub session: Option<String>,
  pub account: Option<String>,
  pub context_tokens: Option<i64>,
  pub all_from_owner: bool,
  pub replaced: Vec<SessionReason>,
  pub last_consumed_id: Option<i64>,
}

/// What a round hands the session: the composed prompt, the room its turn answers in, the road it answers on,
/// the messages it answers and when it took them. The carrying fills what the session's own program needs
/// beside these — its room block and persona — from the receiver's own parts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asking {
  pub chat_id: i64,
  pub session: String,
  pub prompt: String,
  pub via: Option<String>,
  pub privileged: bool,
  pub answers: Vec<i64>,
  pub began: i64,
  /// What the turn was built with, each document by its source and its path, which the carrying writes down
  /// beside its run.
  pub context: Vec<(String, String)>,
}

/// What carries a composed turn into the agent's session and answers with what the session said. The agency
/// carries the turn; its answer comes back as the turn's end. Filled by the module that owns the carrying.
pub trait Carrying: Send + Sync {
  fn carry<'a>(
    &'a self,
    asking: &'a Asking,
    held: &'a Held,
    follower: &'a Follower,
  ) -> BoxFuture<'a, Result<Reply, Failure>>;
}

/// What a round's answer is held to while the checks read it: the answer itself, the round's scope, the words
/// the round read and the words around it, whether its turn may use tools and what it still owes the owner.
pub struct Checked<'a> {
  pub reply: Reply,
  pub scope: &'a RoundScope,
  pub recent: &'a [Inbound],
  pub batch: &'a [Inbound],
  pub tools_allowed: bool,
  pub owed: usize,
  /// The room the round answers, the road its answer goes back on, whether every word it took was the owner's,
  /// and the messages waiting for the next turn: what a second asking goes out with beside its errand.
  pub room: Room,
  pub via: String,
  pub privileged: bool,
  pub pending: Vec<i64>,
  /// The round's own selection, which a second asking goes out with the same reference on.
  pub selection: &'a Selection,
}

/// What holds a round's answer to the checks the code keeps before it goes out (docs/answer-checks.md), and what
/// the round wrote down beside it; it answers with the words that go out, which a check may replace. Filled by
/// the module that owns the checks.
pub trait Checks: Send + Sync {
  fn checks<'a>(
    &'a self,
    conversations: &'a Conversations,
    receiver: &'a Receiver,
    agent: &'a str,
    held: &'a Held,
    checked: &'a Checked<'a>,
  ) -> BoxFuture<'a, Result<Reply, Failure>>;
}

/// Where a round's answer goes out: the room it goes out in, the message it answers, the road it answers on, the
/// round that wrote it and when the room said the earliest thing it answers.
pub struct ReplyRoad {
  pub chat_id: i64,
  pub reply_to: Option<i64>,
  pub via: String,
  pub round: String,
  pub asked_at: Option<i64>,
}

/// What sends a round's answer into its room, on the road the turn answers on: the number of the message it went
/// out as and its words, where anything went out. Filled by the module that owns the send.
pub trait Sending: Send + Sync {
  fn send<'a>(
    &'a self,
    receiver: &'a Receiver,
    agent: &'a str,
    road: &'a ReplyRoad,
    reply: &'a Reply,
  ) -> BoxFuture<'a, Option<Sent>>;
}

/// What writes a session's and its road's records as a turn went out, and what folds a session past its window.
/// Filled by the modules that own the notes and the fold.
pub trait Noting: Send + Sync {
  fn note<'a>(
    &'a self,
    receiver: &'a Receiver,
    agent: &'a str,
    held: &'a Held,
    reply: &'a Reply,
    ms: i64,
  ) -> BoxFuture<'a, ()>;
  fn fold<'a>(
    &'a self,
    conversations: &'a Conversations,
    receiver: &'a Receiver,
    agent: &'a str,
    chat_id: i64,
    held: &'a Held,
    room: Room,
  ) -> BoxFuture<'a, ()>;
}

/// The words a round of a pieces-shaped agent carries: the room it runs in, the messages and instructions it
/// took, the roads they came in on and the events among them.
pub struct PiecesRound {
  pub chat_id: i64,
  pub batch: Vec<Inbound>,
  pub instructions: Vec<String>,
  pub ids: Vec<i64>,
  pub events: Vec<TurnEvent>,
  pub via: String,
}

/// What a round of a pieces-shaped agent came to: no turn, where nothing was left to answer or its companion's
/// reset overtook it; a turn that answered, with the message its answer went out as where one went out and the
/// hold its reset's cut-off waits on until the room's window has moved; or a turn that failed, with why and the
/// last message it read.
pub enum PiecesEnded {
  NoTurn,
  Answered { sent: Option<Sent>, keeping: Keeping },
  Failed { reason: String, message: Option<i64> },
}

/// Whether an agent's turns are a companion's own, and what such a round does. Filled by the companion's part.
pub trait Pieces: Send + Sync {
  fn of<'a>(&'a self, receiver: &'a Receiver, agent: &'a str) -> BoxFuture<'a, bool>;

  fn round<'a>(
    &'a self,
    conversations: &'a Conversations,
    agent: &'a str,
    pieces: &'a PiecesRound,
    turn: u64,
    round: &'a str,
    record: &'a Record,
  ) -> BoxFuture<'a, Result<PiecesEnded, Failure>>;
}

/// The health of the rooms' sessions as the conversations read it: whether a room has fallen silent, what a
/// failed round is told of, a round that ended with nothing said at all, and the minute the failure orders are
/// looked over on. Filled by the module that owns the health.
pub trait Health: Send + Sync {
  fn silenced<'a>(&'a self, receiver: &'a Receiver, chat_id: i64) -> BoxFuture<'a, bool>;
  fn failed<'a>(&'a self, receiver: &'a Receiver, chat_id: i64, failure: &'a Failure) -> BoxFuture<'a, ()>;
  fn died<'a>(&'a self, receiver: &'a Receiver, chat_id: i64) -> BoxFuture<'a, ()>;
  fn orders(&self, receiver: &Receiver);
}

/// What a round's answer went out as, where it went out: the number of its message and its words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sent {
  pub message_id: i64,
  pub text: String,
}

/// What a turn of a console talk is asked with, and what it answers with (docs/unified-bmo.md, 「Console
/// conversation in the private chat」): the conversations take it as a round of the owner's private chat.
pub trait Talking: Send + Sync {
  fn talk<'a>(&'a self, request: &'a TalkRequest) -> BoxFuture<'a, Result<Option<String>, TalkFailure>>;
}

/// The touch of the quiet rooms' sessions, on the idle stretch the warmer names where it shortens it
/// (docs/bmo-window.md, 「The touch」): the conversations take it behind whatever round is running.
pub trait Touching: Send + Sync {
  fn touch<'a>(
    &'a self,
    receiver: &'a Receiver,
    agent: &'a str,
    idle_override_ms: Option<i64>,
  ) -> BoxFuture<'a, Result<TouchAnswer, Fault>>;
}

/// What the window hands a line a person typed into the agent's program (docs/bmo-window.md, 「A line typed into
/// the program」): the conversations compose it as a round of the owner's private chat.
pub trait Typing: Send + Sync {
  fn typed_line<'a>(
    &'a self,
    receiver: &'a Receiver,
    agent: &'a str,
    text: &'a str,
    session_id: &'a str,
    at_ms: i64,
  ) -> BoxFuture<'a, Result<String, Fault>>;
}

/// What the conversations' own clock looks over on the health policy's minute, for as long as the process stands
/// (docs/answer-checks.md). Filled by the module that owns the failure orders.
pub trait Orders: Send + Sync {
  fn check_failure_orders(&self, receiver: &Receiver, agent: &str);
}

// A conversation as it stands in a room.
struct Conv {
  room: Room,
  reply_target: Option<i64>,
  buffer: Vec<Inbound>,
  instructions: Vec<String>,
  instruction_ids: Vec<i64>,
  instruction_roads: Vec<String>,
  instruction_events: Vec<TurnEvent>,
  seen: HashSet<i64>,
  asked_at: Option<i64>,
  held: Held,
  first_call: bool,
  answered: bool,
  in_flight: bool,
  expired: bool,
  waiting: bool,
  held_words: Vec<Inbound>,
  held_since: HashMap<i64, i64>,
  window_until: i64,
  timer: Option<JoinHandle<()>>,
  carried: Option<Carried>,
}

// The round a conversation is running, as the words pushed into its working turn grow it: every message it
// carries, the instructions it took with their roads and rows, the paragraphs pushed and the instructions among
// them as the session read them, what it holds by path, and whether its turn still takes more.
pub struct Carried {
  round: String,
  privileged: bool,
  tools_allowed: bool,
  messages: Vec<Inbound>,
  roads: Vec<String>,
  settling: Vec<i64>,
  pushed: Vec<String>,
  pushed_instructions: Vec<String>,
  holds: HashSet<String>,
  open: bool,
  shut: bool,
  pushing: bool,
}

// The words a push took out of the conversation, held until the turn says what became of them.
struct Offered {
  messages: Vec<Inbound>,
  instructions: Vec<String>,
  ids: Vec<i64>,
  roads: Vec<String>,
  events: Vec<TurnEvent>,
  paragraph: Paragraph,
}

/// What became of words pushed into a working turn: the turn took them, its program waits on a person so it
/// takes no more, or they did not go in. The carrying reads the agency's answer to the push and names one of
/// these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AsideOutcome {
  Delivered,
  Waiting,
  Refused,
}

/// What a working turn is offered words through: the words that came for its room, composed, where it still
/// takes some, and what became of them once the agency answered.
pub trait Aside: Send + Sync {
  fn offer(&self) -> BoxFuture<'_, Option<String>>;

  fn settle(&self, outcome: AsideOutcome) -> BoxFuture<'_, ()>;
}

impl Aside for Follower {
  fn offer(&self) -> BoxFuture<'_, Option<String>> {
    Box::pin(Follower::offer(self))
  }

  fn settle(&self, outcome: AsideOutcome) -> BoxFuture<'_, ()> {
    Box::pin(Follower::settle(self, outcome))
  }
}

impl From<Said> for AsideOutcome {
  fn from(said: Said) -> Self {
    match said {
      Said::Delivered => Self::Delivered,
      Said::Waiting => Self::Waiting,
      Said::Refused => Self::Refused,
    }
  }
}

/// A quiet instruction with the row it came as and the events its delivery carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quiet {
  pub id: i64,
  pub text: String,
  pub events: Vec<TurnEvent>,
}

/// What came into a room while its round waited, taken now for the turn that waited.
#[derive(Clone, Debug, Default)]
pub struct Waiting {
  pub words: Vec<Inbound>,
  pub quiet: Vec<Quiet>,
}

impl Conv {
  // Words a push took that the turn did not, back at the front of what waits, in the order they came.
  fn give_back(
    &mut self,
    messages: Vec<Inbound>,
    instructions: Vec<String>,
    ids: Vec<i64>,
    roads: Vec<String>,
    events: Vec<TurnEvent>,
  ) {
    self.buffer.splice(0..0, messages);
    self.instructions.splice(0..0, instructions);
    self.instruction_ids.splice(0..0, ids);
    self.instruction_roads.splice(0..0, roads);
    self.instruction_events.splice(0..0, events);
  }

  fn new(_chat_id: i64, room: Room, held: Held) -> Self {
    Self {
      room,
      reply_target: None,
      buffer: vec![],
      instructions: vec![],
      instruction_ids: vec![],
      instruction_roads: vec![],
      instruction_events: vec![],
      seen: HashSet::new(),
      asked_at: None,
      held,
      first_call: true,
      answered: false,
      in_flight: false,
      expired: false,
      waiting: false,
      held_words: vec![],
      held_since: HashMap::new(),
      window_until: 0,
      timer: None,
      carried: None,
    }
  }
}

#[derive(Default)]
struct State {
  active: HashMap<i64, Conv>,
  // Each room's words the queue is gathering.
  queue: HashMap<i64, Vec<Inbound>>,
  // How many gathered batches have left the queue and not yet reached their conversation.
  delivering: usize,
  called_windows: HashMap<i64, i64>,
  // Why each room's session was last replaced, which its next turn is told.
  woken: HashMap<i64, String>,
  // Every fold running now, by room.
  folding: HashSet<i64>,
  // The speaker each room's turns are shown as, where the agent is not the room's default answerer.
  speakers: HashMap<i64, Value>,
  // The agent's own runs going now, by name with the room each runs for, which a reset or a room's removal
  // stands its cancellation over.
  going: HashMap<String, i64>,
}

/// A run of an agent's own named as going until this is dropped.
pub struct Going {
  conversations: Conversations,
  run: String,
}

impl Drop for Going {
  fn drop(&mut self) {
    self.conversations.state().going.remove(&self.run);
  }
}

/// A turn keeping what it came to, holding off its agent's reset until this is dropped.
pub struct Keeping {
  _held: tokio::sync::OwnedRwLockReadGuard<()>,
}

/// A reset moving its agent's point, holding off every turn's keeping until this is dropped.
pub struct Cutting {
  _held: tokio::sync::OwnedRwLockWriteGuard<()>,
}

/// What a reset cut off of an agent's conversations: the rows of the quiet instructions let go unread, and the
/// runs of its own going then.
#[derive(Debug, Default)]
pub struct CutOff {
  pub instructions: Vec<i64>,
  pub going: Vec<String>,
}

// What a round of the pump keeps of itself: whether its turn was written down, the rows it settles, and whether
// it held its words over.
#[derive(Default)]
pub struct Record {
  settling: Vec<i64>,
  held_over: bool,
}

struct Inner {
  /// The agent whose conversations these are.
  agent: String,
  receiver: Receiver,
  hooks: Arc<dyn Hooks>,
  timings: Timings,
  state: Mutex<State>,
  turn: tokio::sync::Mutex<()>,
  // Shared by the agent's turns while they keep what they came to, taken whole by its reset as it moves the point.
  keeping: Arc<tokio::sync::RwLock<()>>,
  carrying: Arc<dyn Carrying>,
  checks: Arc<dyn Checks>,
  sending: Arc<dyn Sending>,
  noting: Arc<dyn Noting>,
  pieces: Arc<dyn Pieces>,
  health: Arc<dyn Health>,
  orders: Arc<dyn Orders>,
}

/// One conversational agent's conversations, every room's in one: cloned freely, every clone the same.
#[derive(Clone)]
pub struct Conversations(Arc<Inner>);

impl Conversations {
  /// An agent's conversations on the receiver's own parts of a round: its turns carried into the agency's
  /// sessions, its answers held to the checks and sent on the road they answer on, its sessions noted and folded,
  /// a companion's rounds its own, and the health and failure orders of its rooms. The talk and the touch are the
  /// conversations' own, taken behind whatever round is running, and so is the typed line, composed at once.
  pub fn new(receiver: Receiver, hooks: Arc<dyn Hooks>, timings: Timings, agent: &str) -> Self {
    Self(Arc::new(Inner {
      agent: agent.to_owned(),
      carrying: Arc::new(Carry::looking_every(receiver.clone(), timings.look)),
      checks: Arc::new(Checker::new(receiver.clone())),
      sending: Arc::new(Sender::new(receiver.clone())),
      noting: Arc::new(Noter::new(receiver.clone())),
      pieces: Arc::new(CompanionTurn),
      health: Arc::new(RoomHealth::new(receiver.clone())),
      orders: Arc::new(FailureOrders::new(receiver.clone())),
      receiver,
      hooks,
      timings,
      state: Mutex::new(State::default()),
      turn: tokio::sync::Mutex::new(()),
      keeping: Arc::new(tokio::sync::RwLock::new(())),
    }))
  }

  /// The agent whose conversations these are.
  pub fn agent(&self) -> &str {
    &self.0.agent
  }

  fn state(&self) -> MutexGuard<'_, State> {
    self.0.state.lock().unwrap_or_else(PoisonError::into_inner)
  }

  pub fn receiver(&self) -> Receiver {
    self.0.receiver.clone()
  }

  pub fn owner(&self) -> i64 {
    self.0.receiver.owner_id()
  }

  // Every event of a room goes out as the agent's own: under the speaker its turns there are shown as.
  fn publish(&self, event: Value) {
    let speaker = event
      .get("chatId")
      .and_then(Value::as_i64)
      .and_then(|chat_id| self.state().speakers.get(&chat_id).cloned());

    self.0.receiver.news().publish(&told_as(event, speaker));
  }

  async fn speaker_in(&self, chat_id: i64) -> Option<Value> {
    let speaker = self.0.receiver.roads().identity_in(chat_id).await;
    let mut state = self.state();

    match &speaker {
      Some(speaker) => state.speakers.insert(chat_id, json!({ "id": speaker })),
      None => state.speakers.remove(&chat_id),
    };

    speaker.map(|id| json!({ "id": id }))
  }

  /// Waits until no round holds the session's turn: a round ends after it stops counting as in flight, with what it
  /// tells around it as it lets go.
  pub async fn turn_free(&self) {
    drop(self.0.turn.lock().await);
  }

  /// What the room queue is gathering for a room and has not yet handed to its conversation.
  pub fn gathering(&self, chat_id: i64) -> Vec<Inbound> {
    self.state().queue.get(&chat_id).cloned().unwrap_or_default()
  }

  /// A run of the agent's own in a room named as going until what this answers is dropped (the agency's
  /// `docs/companions.md`, 「What the reset cuts off」).
  pub fn going(&self, run: &str, chat_id: i64) -> Going {
    self.state().going.insert(run.to_owned(), chat_id);

    Going {
      conversations: self.clone(),
      run: run.to_owned(),
    }
  }

  /// Held by a turn of the agent's from its last reading of the reset point until what it came to is kept and sent,
  /// so its reset never moves the point between that reading and the keeping (the agency's `docs/companions.md`, 「What the reset
  /// cuts off」).
  pub async fn keeping(&self) -> Keeping {
    Keeping {
      _held: self.0.keeping.clone().read_owned().await,
    }
  }

  /// Held by the agent's reset while it moves the point: had once no turn holds `keeping`, and holding off every
  /// turn that would take it after.
  pub async fn cutting(&self) -> Cutting {
    Cutting {
      _held: self.0.keeping.clone().write_owned().await,
    }
  }

  /// Everything waiting for the agent's turns in every room let go unread, as its reset cuts it off: the words the
  /// queue gathers and those a conversation holds or waits on, and its quiet instructions, whose rows it answers
  /// with the runs of its own going now (the agency's `docs/companions.md`, 「What the reset cuts off」).
  pub fn cut_off(&self) -> CutOff {
    let mut state = self.state();
    let mut rooms: Vec<i64> = state.queue.drain().map(|(chat_id, _)| chat_id).collect();
    let mut instructions = vec![];

    for (chat_id, conv) in &mut state.active {
      rooms.push(*chat_id);
      conv.buffer.clear();
      conv.held_words.clear();
      conv.held_since.clear();
      conv.instructions.clear();
      conv.instruction_roads.clear();
      conv.instruction_events.clear();
      instructions.append(&mut conv.instruction_ids);
    }

    let going = state.going.keys().cloned().collect();

    drop(state);

    for chat_id in rooms {
      self.0.receiver.news().drop_queued(chat_id);
    }

    CutOff {
      instructions,
      going,
    }
  }

  /// Everything the agent keeps of a room forgotten, as the room's registration is removed (docs/receiver.md,
  /// 「Removing a room's registration」): its conversation and timer, the words gathering for it, its window, why
  /// its session was replaced, its fold, its speaker and the fold failure it was told of. Nothing is written to the
  /// archive, whose rows the removal takes in its own write.
  pub async fn forget_room(&self, chat_id: i64) {
    let conv = {
      let mut state = self.state();

      state.queue.remove(&chat_id);
      state.called_windows.remove(&chat_id);
      state.woken.remove(&chat_id);
      state.folding.remove(&chat_id);
      state.speakers.remove(&chat_id);
      state.active.remove(&chat_id)
    };

    if let Some(timer) = conv.and_then(|mut conv| conv.timer.take()) {
      timer.abort();
    }

    self.0.hooks.typing(chat_id, false).await;
  }

  /// The runs of the agent's own going now in a room, which its removal stands its cancellation over
  /// (docs/receiver.md, 「Removing a room's registration」).
  pub fn going_in(&self, chat_id: i64) -> Vec<String> {
    let state = self.state();

    state
      .going
      .iter()
      .filter(|(_, room)| **room == chat_id)
      .map(|(run, _)| run.clone())
      .collect()
  }

  /// Whether a conversation stands in the room.
  pub fn is_active(&self, chat_id: i64) -> bool {
    self.state().active.contains_key(&chat_id)
  }

  /// Whether nothing is waiting or running: no words gathering or on their way to the conversation they wake,
  /// no round in flight, no wait holding words for a round still to come, nothing a conversation still has to
  /// read.
  pub fn idle(&self) -> bool {
    let state = self.state();

    state.queue.is_empty()
      && state.delivering == 0
      && state.folding.is_empty()
      && state.active.values().all(|conv| {
        !conv.in_flight && !conv.waiting && conv.buffer.is_empty() && conv.instructions.is_empty()
      })
  }

  // How long the agent's window runs, read as each window opens so a change to its row reaches the next.
  async fn window_ms(&self) -> i64 {
    if let Some(window) = self.0.timings.window {
      return window.as_millis() as i64;
    }

    let listed = self.0.receiver.units().agency.agents().await.ok();
    let row = listed
      .as_ref()
      .and_then(|listed| listed.agents.iter().find(|one| one.name == self.agent()));
    let seconds = match row {
      Some(row) => row.row.get("window_seconds").and_then(Value::as_i64).unwrap_or(0),
      None => CHAT_ROUND.conversation.window_seconds,
    };

    seconds * 1000
  }

  async fn window_until(&self, chat_id: i64) -> i64 {
    wake::agents::window_until(self.0.receiver.db(), chat_id, self.agent())
      .await
      .ok()
      .unwrap_or(0)
  }

  async fn set_window(&self, chat_id: i64, until: i64) {
    let written = wake::agents::set_window(self.0.receiver.db(), chat_id, self.agent(), until).await;

    if let Err(why) = written {
      eprintln!("chat {chat_id}: the window could not be written: {why}");
    } else if until > 0 {
      self.0.receiver.news().window(chat_id);
    }
  }

  // The read mode the deployment listens in, which says whose messages wake the agents.
  async fn mode(&self) -> String {
    let mode = crate::store::settings::get(self.0.receiver.db(), "mode").await.ok().flatten();

    match mode.as_deref() {
      Some(mode @ ("owner_only" | "everyone")) => mode.to_owned(),
      _ => "owner_only".to_owned(),
    }
  }

  // The session the room's turns go on: the one the agent's row keeps, resumed where one stands.
  pub async fn held_session(&self, chat_id: i64) -> Held {
    Held {
      agent: self.agent().to_owned(),
      chat_id,
      session: None,
      account: None,
      context_tokens: None,
      all_from_owner: false,
      replaced: vec![],
      last_consumed_id: None,
    }
  }

  async fn remember_room(&self, chat_id: i64) {
    if !unified_session(&self.0.receiver, self.agent()).await {
      return;
    }

    if let Err(why) = run(
      self.0.receiver.db(),
      r#"INSERT INTO "bmo_rooms" VALUES ($1) ON CONFLICT DO NOTHING"#,
      vec![chat_id.into()],
    )
    .await
    {
      eprintln!("chat {chat_id}: the room could not be remembered: {}", why.detail);
    }
  }

  // The deadline the window or a wait stands until.
  fn set_deadline(&self, conv: &mut Conv, chat_id: i64, ms: i64, waiting: bool) {
    if let Some(timer) = conv.timer.take() {
      timer.abort();
    }

    conv.expired = false;
    conv.waiting = waiting;

    let this = self.clone();

    conv.timer = Some(tokio::spawn(async move {
      tokio::time::sleep(Duration::from_millis(ms.max(0) as u64)).await;
      Box::pin(this.on_deadline(chat_id)).await;
    }));
  }

  // A wait's deadline: a wait length after the earliest moment a held person was last waited on from.
  fn arm_wait(&self, conv: &mut Conv, chat_id: i64) {
    let wait = self.0.timings.wait.as_millis() as i64;
    let since = conv.held_since.values().min().copied();
    let ms = since.map_or(wait, |since| (since + wait - now_ms()).max(0));

    self.set_deadline(conv, chat_id, ms, true);
  }

  async fn restart_timer(&self, chat_id: i64) {
    let ms = self.window_ms().await;
    let mut state = self.state();

    if let Some(conv) = state.active.get_mut(&chat_id) {
      self.set_deadline(conv, chat_id, ms, false);
    }
  }

  async fn restart_window(&self, chat_id: i64) {
    self.restart_timer(chat_id).await;

    let until = now_ms() + self.window_ms().await;

    if let Some(conv) = self.state().active.get_mut(&chat_id) {
      conv.window_until = until;
    }

    self.set_window(chat_id, until).await;
  }

  async fn on_deadline(&self, chat_id: i64) {
    let closes = {
      let mut state = self.state();
      let Some(conv) = state.active.get_mut(&chat_id) else {
        return;
      };

      // The deadline has come: its timer is the task running this, which nothing is to abort now.
      drop(conv.timer.take());

      if conv.answered
        && conv.buffer.is_empty()
        && conv.held_words.is_empty()
        && conv.instructions.is_empty()
        && !conv.in_flight
      {
        true
      } else {
        conv.expired = true;
        conv.waiting = false;
        false
      }
    };

    if closes {
      self.close(chat_id).await;
      return;
    }

    let pumps = self.state().active.get(&chat_id).is_some_and(|conv| !conv.in_flight);

    // The round the deadline sets going is no part of the deadline's own task.
    if pumps {
      self.spawn_pump(chat_id, true);
    }
  }

  /// A room's message joins its queue rather than waking the session at once; a message that calls the agent opens
  /// the room's window now, so what the caller adds meanwhile is let in.
  pub async fn enqueue(&self, message: Inbound, room: Room, opens_window: bool) {
    let chat_id = message.chat_id;

    if opens_window {
      self.open_window(chat_id).await;
    }

    let mut state = self.state();

    // A wait is on the people whose words it holds: one of them speaking again starts theirs over.
    if let Some(conv) = state.active.get_mut(&chat_id)
      && conv.waiting
      && !conv.expired
      && conv.held_since.contains_key(&held_person(&message))
    {
      conv.held_since.insert(held_person(&message), now_ms());
      self.arm_wait(conv, chat_id);
    }

    // From here until a turn takes it the message stands queued in the picture (docs/receiver.md, 「The event
    // stream」).
    if !state.active.get(&chat_id).is_some_and(|conv| conv.seen.contains(&message.message_id)) {
      self.0.receiver.news().queue_message(chat_id, message.message_id);
    }

    match state.queue.get_mut(&chat_id) {
      Some(gathering) => gathering.push(message),
      None => {
        let this = self.clone();
        let delay = self.0.timings.queue;

        state.queue.insert(chat_id, vec![message]);
        tokio::spawn(async move {
          tokio::time::sleep(delay).await;

          // The batch counts as delivering from the moment it leaves the queue, under the same lock, so no moment
          // finds it in neither while its conversation is still being woken.
          let gathered = {
            let mut state = this.state();

            state.delivering += 1;
            state.queue.remove(&chat_id).unwrap_or_default()
          };

          Box::pin(this.deliver(chat_id, gathered, room)).await;
          this.state().delivering -= 1;
        });
      }
    }
  }

  // A call opens its room's window: a conversation already running takes it as its own; otherwise the window is
  // written down at once and the conversation the queue is about to wake takes it over.
  async fn open_window(&self, chat_id: i64) {
    if self.is_active(chat_id) {
      self.restart_window(chat_id).await;
      return;
    }

    let until = now_ms() + self.window_ms().await;

    if self.window_until(chat_id).await < until {
      self.set_window(chat_id, until).await;
    }

    self.state().called_windows.insert(chat_id, until);
  }

  async fn deliver(&self, chat_id: i64, batch: Vec<Inbound>, room: Room) {
    self.publish(json!({
      "type": "queued",
      "chatId": chat_id,
      "messageId": batch.last().map(|message| message.message_id),
      "detail": format!("{} message(s)", batch.len()),
    }));

    if self.is_active(chat_id) {
      self.state().called_windows.remove(&chat_id);

      for message in batch {
        self.forward(message).await;
      }

      return;
    }

    self.wake(batch, room).await;
  }

  /// A room's conversation woken by the words it was handed.
  pub async fn wake(&self, seed: Vec<Inbound>, room: Room) {
    let (Some(first), Some(last)) = (seed.first().cloned(), seed.last().cloned()) else {
      return;
    };
    let chat_id = first.chat_id;

    if self.is_active(chat_id) {
      for message in seed {
        self.forward(message).await;
      }

      return;
    }

    self.remember_room(chat_id).await;

    let held = self.held_session(chat_id).await;
    let window = self.window_ms().await;
    let owner = self.owner();
    let pumps = {
      let mut state = self.state();

      if state.active.contains_key(&chat_id) {
        false
      } else {
        let called = state.called_windows.remove(&chat_id).unwrap_or(0);
        let mut conv = Conv::new(chat_id, room, held);

        for message in seed {
          if conv.seen.insert(message.message_id) {
            conv.buffer.push(message);
          }
        }

        conv.reply_target = (!last.via_reader.unwrap_or(false)).then_some(last.message_id);
        conv.held.all_from_owner = conv.buffer.iter().all(|message| message.sender_id == Some(owner));
        conv.window_until = called;
        self.set_deadline(&mut conv, chat_id, window, false);
        state.active.insert(chat_id, conv);
        true
      }
    };

    if pumps {
      self.spawn_pump(chat_id, false);
    }
  }

  async fn forward(&self, message: Inbound) {
    let chat_id = message.chat_id;
    let owner = self.owner();
    let (restarts, pumps, force) = {
      let mut state = self.state();
      let Some(conv) = state.active.get_mut(&chat_id) else {
        return;
      };

      if !conv.seen.insert(message.message_id) {
        return;
      }

      if message.sender_id != Some(owner) {
        conv.held.all_from_owner = false;
      }

      conv.buffer.push(message);

      // The conversation lives on while its room is let in, but the room speaking does not open the window.
      let restarts = conv.answered;

      if restarts {
        conv.answered = false;
      }

      (restarts, !conv.in_flight, conv.expired)
    };

    if restarts {
      self.restart_timer(chat_id).await;
    }

    if pumps {
      self.spawn_pump(chat_id, force);
    }
  }

  /// A quiet instruction joins the room's own session like a message, but consumes nothing and is never seen in
  /// the room; a row of the archive's is settled once the round that takes it has ended.
  pub async fn instruct(
    &self,
    chat_id: i64,
    text: String,
    id: Option<i64>,
    via: Option<String>,
    events: Vec<TurnEvent>,
  ) {
    self.remember_room(chat_id).await;
    println!("quiet instruction for chat {chat_id}: {}", text.split('\n').next().unwrap_or_default());

    let (exists, pumps, force) = {
      let mut state = self.state();

      match state.active.get_mut(&chat_id) {
        Some(conv) => {
          conv.instructions.push(text.clone());
          conv.instruction_ids.extend(id);
          conv.instruction_roads.extend(via.clone());
          conv.instruction_events.extend(events.clone());
          (true, !conv.in_flight, conv.expired)
        }
        None => (false, false, false),
      }
    };

    if exists {
      if pumps {
        self.spawn_pump(chat_id, force);
      }

      return;
    }

    let owner_room = self.0.receiver.roads().is_owner_room(chat_id).await;
    let room = Room {
      owner_room,
      identity: None,
      private_to: None,
    };
    let held = self.held_session(chat_id).await;
    let window = self.window_ms().await;
    let pumps = {
      let mut state = self.state();

      // A conversation another word opened meanwhile takes the instruction as its own.
      match state.active.entry(chat_id) {
        std::collections::hash_map::Entry::Occupied(mut standing) => {
          let conv = standing.get_mut();

          conv.instructions.push(text);
          conv.instruction_ids.extend(id);
          conv.instruction_roads.extend(via);
          conv.instruction_events.extend(events);
          !conv.in_flight
        }
        std::collections::hash_map::Entry::Vacant(vacant) => {
          let mut conv = Conv::new(chat_id, room, held);

          conv.instructions.push(text);
          conv.instruction_ids.extend(id);
          conv.instruction_roads.extend(via);
          conv.instruction_events.extend(events);
          conv.held.all_from_owner = owner_room;
          self.set_deadline(&mut conv, chat_id, window, false);
          vacant.insert(conv);
          true
        }
      }
    };

    if pumps {
      self.spawn_pump(chat_id, false);
    }
  }

  fn spawn_pump(&self, chat_id: i64, force: bool) {
    let this = self.clone();

    tokio::spawn(async move { Box::pin(this.pump(chat_id, force)).await });
  }

  // Whatever arrived while a reply was in flight runs as soon as that reply is done.
  fn pump_pending(&self, chat_id: i64) {
    let pending = self.state().active.get(&chat_id).map(|conv| {
      (
        (!conv.instructions.is_empty() || !conv.buffer.is_empty()) && !conv.in_flight,
        conv.expired,
      )
    });

    if let Some((true, force)) = pending {
      self.spawn_pump(chat_id, force);
    }
  }

  /// Whether a room has fallen silent: the rest closes it and nothing else would answer it.
  pub async fn silenced(&self, chat_id: i64) -> bool {
    self.0.health.silenced(&self.0.receiver, chat_id).await
  }

  pub async fn close(&self, chat_id: i64) {
    let (conv, gathering) = {
      let mut state = self.state();

      (state.active.remove(&chat_id), state.queue.contains_key(&chat_id))
    };
    let Some(mut conv) = conv else { return };

    if let Some(timer) = conv.timer.take() {
      timer.abort();
    }

    // Whatever it had not read goes unread, but words the queue is still gathering wait for the next.
    if !gathering {
      self.0.receiver.news().drop_queued(chat_id);
    }

    self.persist(&conv.held).await;

    // Instructions no round took go back to the queue rather than out of memory with the talk.
    if !conv.instruction_ids.is_empty() {
      return_instructions(self.0.receiver.db(), conv.instruction_ids).await;
    }

    // A turn closes the window it opened and never one it did not.
    if self.window_until(chat_id).await <= conv.window_until {
      self.set_window(chat_id, 0).await;
    }
  }

  async fn persist(&self, held: &Held) {
    if let Some(last) = held.last_consumed_id
      && let Err(why) = rows::set_last_read(self.0.receiver.db(), held.chat_id, last).await
    {
      eprintln!("chat {}: the read mark could not be written: {}", held.chat_id, why.detail);
    }
  }

  async fn pump(&self, chat_id: i64, force: bool) {
    {
      let mut state = self.state();
      let Some(conv) = state.active.get_mut(&chat_id) else { return };

      if conv.in_flight {
        return;
      }

      conv.in_flight = true;
    }

    // A turn of any agent but the room's default answerer is shown as that agent's, from whatever it comes to.
    self.speaker_in(chat_id).await;

    if self.silenced(chat_id).await {
      if let Some(conv) = self.state().active.get_mut(&chat_id) {
        conv.in_flight = false;
      }

      let message = self.state().active.get(&chat_id).and_then(|conv| conv.held.last_consumed_id);

      self.close(chat_id).await;
      self.publish(waited(chat_id, message, None));
      return;
    }

    let turn_lock = self.0.turn.lock().await;
    let (messages, instructions, reading) = {
      let state = self.state();
      let Some(conv) = state.active.get(&chat_id) else {
        drop(state);
        drop(turn_lock);
        return;
      };

      (
        conv.buffer.len(),
        conv.instructions.len(),
        conv
          .held_words
          .iter()
          .chain(conv.buffer.iter())
          .last()
          .map(|message| message.message_id),
      )
    };
    let mut picture = json!({
      "chatId": chat_id,
      "messageId": reading,
      "messages": messages,
      "instructions": instructions,
      "since": now_ms() / 1000,
    });

    if let Some(speaker) = self.state().speakers.get(&chat_id).cloned() {
      picture["speaker"] = speaker;
    }

    let turn = self.0.receiver.news().begin_turn(&picture);

    self.publish(json!({
      "type": "run-started",
      "chatId": chat_id,
      "messageId": reading,
      "detail": if instructions > 0 {
        format!("{messages} message(s), {instructions} instruction(s)")
      } else {
        format!("{messages} message(s)")
      },
    }));

    let round = rounds::new_name(chat_id);
    let mut record = Record::default();
    // A round's future holds the whole of its asking: it stands on the heap, not on a worker's stack.
    let ended =
      rounds::within(round.clone(), Box::pin(self.round(chat_id, force, turn, &round, &mut record))).await;

    if let Err(failure) = ended {
      self.fail_round(chat_id, failure).await;
    }

    // The words a wait held go with any other ending.
    let (_held, settling) = {
      let mut state = self.state();

      match state.active.get_mut(&chat_id) {
        Some(conv) => {
          if !record.held_over {
            conv.held_words.clear();
            conv.held_since.clear();
          }

          (Some(conv.held.clone()), record.settling.clone())
        }
        None => (None, record.settling.clone()),
      }
    };

    // A round that ended, however it ended, has dealt with the rows it took.
    if !settling.is_empty() {
      settle_instructions(self.0.receiver.db(), now_ms(), &round, settling).await;
    }

    self.0.receiver.news().end_turn(turn);
    self.0.hooks.typing(chat_id, false).await;

    drop(turn_lock);
  }

  // A round that failed, wherever in it the failure came from: it ends the conversation the way a failed run
  // does, a stop with nothing of the failure road, a limit or an outage with its rest, any other with its order.
  async fn fail_round(&self, chat_id: i64, failure: Failure) {
    self.0.hooks.typing(chat_id, false).await;

    let message = {
      let mut state = self.state();
      let conv = state.active.get_mut(&chat_id);

      match conv {
        Some(conv) => {
          conv.in_flight = false;
          conv.held.last_consumed_id
        }
        None => None,
      }
    };

    self.close(chat_id).await;

    let stopped = matches!(failure, Failure::Stopped);
    let said = failure.reason();

    self.publish(json!({
      "type": if stopped { "run-stopped" } else { "run-failed" },
      "chatId": chat_id,
      "messageId": message,
      "detail": said,
    }));

    self.0.health.failed(&self.0.receiver, chat_id, &failure).await;
  }

  // The road this turn's answer goes back on, as the roads its words came in on and its instructions were asked
  // on choose it.
  async fn road_of(&self, chat_id: i64, batch: &[Inbound], roads: &[String]) -> String {
    let came: Vec<&str> = batch.iter().map(|message| message.via.as_str()).collect();
    let asked: Vec<&str> = roads.iter().map(String::as_str).collect();

    self
      .0
      .receiver
      .roads()
      .turn_road(chat_id, &came, &asked)
      .await
      .map(|road| road.key().to_owned())
      .unwrap_or_default()
  }

  // The read position moves to the newest message the round carries, and the answer is pinned to it where a road
  // can aim at it.
  async fn consume(&self, chat_id: i64, batch: &[Inbound]) {
    let Some(last) = batch.last() else { return };
    let db = self.0.receiver.db();
    let mut consumed = self.state().active.get(&chat_id).and_then(|conv| conv.held.last_consumed_id);

    for message in batch {
      let earlier = match consumed {
        None => false,
        Some(consumed) => stands_at_or_before(db, chat_id, message.message_id, consumed).await,
      };

      if !earlier {
        consumed = Some(message.message_id);
      }
    }

    let pins = self.0.receiver.roads().pinnable(&last.via, last.mode.as_deref()).await;
    let target = (!last.via_reader.unwrap_or(false) && pins).then_some(last.message_id);

    if let Some(conv) = self.state().active.get_mut(&chat_id) {
      conv.held.last_consumed_id = consumed;
      conv.reply_target = target;
    }
  }

  // One round, from what is waiting to whatever it ends in.
  async fn round(
    &self,
    chat_id: i64,
    force: bool,
    turn: u64,
    round: &str,
    record: &mut Record,
  ) -> Result<(), Failure> {
    let receiver = self.receiver();
    let owner = self.owner();

    while self.state().folding.contains(&chat_id) {
      tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // The rows waiting to ride this room's next round go in now, before the round takes what it carries.
    self.0.hooks.riders(self.agent(), chat_id).await;

    let taken = {
      let mut state = self.state();
      let Some(conv) = state.active.get_mut(&chat_id) else {
        return Ok(());
      };
      let held_words = conv.held_words.clone();
      let mut batch = held_words.clone();

      batch.append(&mut conv.buffer);

      let roads = std::mem::take(&mut conv.instruction_roads);

      if !batch.is_empty() && conv.asked_at.is_none() {
        conv.asked_at = batch.iter().map(|message| message.sent_at).min();
      }

      let instructions = std::mem::take(&mut conv.instructions);
      let ids = std::mem::take(&mut conv.instruction_ids);
      let events = std::mem::take(&mut conv.instruction_events);

      record.settling.extend(ids.iter().copied());

      (
        held_words,
        batch,
        roads,
        instructions,
        ids,
        events,
        conv.room,
        conv.first_call,
        conv.held.clone(),
      )
    };
    let (held_words, mut batch, roads, mut instructions, ids, events, room, first_call, mut held) = taken;
    let mut via = self.road_of(chat_id, &batch, &roads).await;

    // A round of a pieces-shaped agent is its companion's own.
    if self.0.pieces.of(&receiver, self.agent()).await {
      let ended = self
        .0
        .pieces
        .round(
          self,
          self.agent(),
          &PiecesRound {
            chat_id,
            batch,
            instructions,
            ids,
            events,
            via: via.clone(),
          },
          turn,
          round,
          record,
        )
        .await?;

      self.pieces_ended(chat_id, ended, &via, room, round).await;

      return Ok(());
    }

    let mut owed = if chat_id == owner {
      batch.iter().filter(|message| message.sender_id == Some(owner)).count()
    } else {
      0
    };

    let final_round = force || (batch.is_empty() && !instructions.is_empty());
    let tools_allowed = true;
    let mode = self.mode().await;
    let waited_on: HashSet<i64> = held_words.iter().map(|message| message.message_id).collect();
    let pending: HashSet<i64> = self.pending(chat_id).into_iter().collect();
    let queue_line = self.queue_line(chat_id).await;
    let woken = self.state().woken.get(&chat_id).cloned();
    let composed = prompt::compose(
      &receiver,
      &Round {
        agent: self.agent(),
        chat_id,
        room,
        owner,
        batch: &batch,
        instructions: &instructions,
        instruction_ids: &ids,
        events: &events,
        force: final_round,
        mode: &mode,
        tools_allowed,
        waited_on: &waited_on,
        first_call,
        all_from_owner: held.all_from_owner,
        session: held.session.as_deref(),
        pending: &pending,
        queue_line: &queue_line,
        woken: woken.as_deref(),
        reader: Some(receiver.roads()),
        round: Some(round.to_owned()),
      },
    )
    .await
    .map_err(|why| Failure::Failed(why.detail))?;

    if let Some(conv) = self.state().active.get_mut(&chat_id) {
      conv.first_call = false;
    }

    self.consume(chat_id, &batch).await;
    held.last_consumed_id = self.state().active.get(&chat_id).and_then(|conv| conv.held.last_consumed_id);

    let follower = Follower::new(self, chat_id);

    if let Some(conv) = self.state().active.get_mut(&chat_id) {
      conv.carried = Some(Carried {
        round: round.to_owned(),
        privileged: held.all_from_owner,
        tools_allowed,
        messages: batch.clone(),
        roads: roads.clone(),
        settling: vec![],
        pushed: vec![],
        pushed_instructions: vec![],
        holds: composed.selection.entries.iter().map(|entry| entry.path.clone()).collect(),
        open: true,
        shut: false,
        pushing: false,
      });
    }

    let started = now_ms();
    let asking = Asking {
      chat_id,
      session: held.session.clone().unwrap_or_default(),
      prompt: window::composed(&composed.selection, &composed.prompt),
      via: match via.as_str() {
        "" => None,
        road => Some(road.to_owned()),
      },
      privileged: held.all_from_owner,
      answers: batch.iter().map(|message| message.message_id).collect(),
      began: started,
      context: composed.context.clone(),
    };
    let asked = self.0.carrying.carry(&asking, &held, &follower).await;
    let carried = follower.finish();
    let mut scope = composed.scope.clone();
    let mut recent = composed.recent.clone();

    // What the turn took while it worked is the round's as much as what it froze, so everything decided from here
    // on reads the grown set: the road the answer goes back on, the answer owed, the wait it ends, the scope.
    if let Some(carried) = carried {
      let grown: Vec<Inbound> = carried.messages[batch.len().min(carried.messages.len())..].to_vec();

      record.settling.extend(carried.settling.iter().copied());

      if !grown.is_empty() || carried.roads.len() > roads.len() {
        via = self.road_of(chat_id, &carried.messages, &carried.roads).await;
      }

      if !grown.is_empty() {
        batch = carried.messages.clone();
        owed = if chat_id == owner {
          batch.iter().filter(|message| message.sender_id == Some(owner)).count()
        } else {
          0
        };

        if let Some(conv) = self.state().active.get_mut(&chat_id) {
          conv.asked_at = conv.asked_at.or_else(|| grown.iter().map(|message| message.sent_at).min());
        }

        scope.message_ids = batch.iter().map(|message| message.message_id).collect();

        for message in &grown {
          if !recent.iter().any(|shown| shown.message_id == message.message_id) {
            recent.push(message.clone());
          }
        }
      }

      if !carried.pushed_instructions.is_empty() {
        scope.instructions = std::iter::once(scope.instructions.clone())
          .chain(carried.pushed_instructions.iter().cloned())
          .filter(|text| !text.is_empty())
          .collect::<Vec<_>>()
          .join("\n\n");
        instructions.extend(carried.pushed_instructions);
      }
    }

    // A push moves what the round has read up to.
    held.last_consumed_id = self.state().active.get(&chat_id).and_then(|conv| conv.held.last_consumed_id);

    let reply = asked?;
    let stripped = crate::form::strip_prompt_scaffolding(&reply.text);
    let mut reply = Reply {
      text: if stripped.is_empty() { super::sentinels::SKIP.to_owned() } else { stripped },
      ..reply.clone()
    };
    let ratio = reply.context_ratio();

    self.0.noting.note(&receiver, self.agent(), &held, &reply, now_ms() - started).await;
    held.context_tokens = reply.context_tokens.or(held.context_tokens);
    held.session = reply.session.clone().or(held.session);
    held.replaced = reply.replaced.clone();

    if let Some(reason) = held.replaced.last() {
      self.state().woken.insert(chat_id, reason.to_string());
    }

    self.keep_session(chat_id, &held);

    if is_wait(&reply.text) {
      return self.waited_round(chat_id, &batch, &held_words, final_round, record).await;
    }

    reply = self
      .0
      .checks
      .checks(
        self,
        &receiver,
        self.agent(),
        &held,
        &Checked {
          reply: reply.clone(),
          scope: &scope,
          recent: &recent,
          batch: &batch,
          tools_allowed,
          owed,
          room,
          via: via.to_owned(),
          privileged: held.all_from_owner,
          pending: self.pending(chat_id),
          selection: &composed.selection,
        },
      )
      .await?;

    if is_skip(&reply.text) {
      self.keep_session(chat_id, &held);

      let consumed = {
        let mut state = self.state();
        let Some(conv) = state.active.get_mut(&chat_id) else {
          return Ok(());
        };

        conv.asked_at = None;
        conv.held.last_consumed_id
      };

      self.persist(&held).await;
      self.record_skip(chat_id, (!batch.is_empty()).then_some(consumed).flatten()).await;

      // The round is over once its skip is written down, as an answered round is once its answer has gone out.
      let close = {
        let mut state = self.state();
        let Some(conv) = state.active.get_mut(&chat_id) else {
          return Ok(());
        };

        conv.in_flight = false;

        if batch.is_empty() {
          !conv.answered && conv.buffer.is_empty() && conv.instructions.is_empty()
        } else {
          conv.answered = true;
          false
        }
      };

      if close {
        self.close(chat_id).await;
        return Ok(());
      }

      self.pump_pending(chat_id);
      return Ok(());
    }

    self.keep_session(chat_id, &held);

    // The room is about to be answered, so a replacement is behind it.
    self.state().woken.remove(&chat_id);

    self.persist(&held).await;

    if let Some(conv) = self.state().active.get_mut(&chat_id) {
      conv.answered = true;
    }

    self.restart_window(chat_id).await;

    let waiting = {
      let state = self.state();

      state.active.get(&chat_id).map(|conv| (conv.reply_target, conv.asked_at))
    };
    let (reply_to, asked_at) = waiting.unwrap_or((None, None));
    let sent = if self.silenced(chat_id).await {
      None
    } else {
      self
        .0
        .sending
        .send(
          &receiver,
          self.agent(),
          &ReplyRoad {
            chat_id,
            reply_to,
            via: via.to_owned(),
            round: round.to_owned(),
            asked_at,
          },
          &reply,
        )
        .await
    };

    if let Some(sent) = &sent {
      if let Some(conv) = self.state().active.get_mut(&chat_id) {
        conv.asked_at = None;
      }

      self.publish(json!({
        "type": "sent",
        "chatId": chat_id,
        "messageId": sent.message_id,
        "detail": sent.text,
        "round": round,
      }));

      self.hand_filed(chat_id, sent.message_id, &via, room).await;
      self.relay_answer(chat_id, &sent.text).await;
    }

    if let Some(conv) = self.state().active.get_mut(&chat_id) {
      conv.in_flight = false;
    }

    self.maybe_fold(chat_id, room, held.session.as_deref(), ratio).await;
    self.pump_pending(chat_id);

    Ok(())
  }

  // An answer once filed reaches the receiver as every word filed in a room does, so another agent of the room
  // may be woken by it (docs/wake.md, 「Who is woken」).
  async fn hand_filed(&self, chat_id: i64, message_id: i64, via: &str, room: Room) {
    let declared = Room {
      owner_room: room.owner_room,
      identity: self.0.receiver.roads().identity_in(chat_id).await.or(room.identity),
      private_to: room.private_to,
    };

    if let Err(why) = self.0.receiver.filed(chat_id, message_id, via, declared).await {
      eprintln!("chat {chat_id}: the answer could not be handed to the receiver: {why}");
    }
  }

  // A companion's round ends free for the next round. A turn that came to none leaves the room as it was; one that
  // failed is told to the room's screens with why, under the companion's name; and where its turn answered, the
  // answer that went out is told to them and handed to the receiver as filed. Either way the room is answered and
  // its window moved, and only then is the hold its reset's cut-off waits on let go.
  async fn pieces_ended(&self, chat_id: i64, ended: PiecesEnded, via: &str, room: Room, round: &str) {
    let (sent, keeping) = match ended {
      PiecesEnded::NoTurn => {
        self.no_turn(chat_id);
        return;
      }
      PiecesEnded::Failed { reason, message } => {
        self.publish(json!({
          "type": "run-failed",
          "chatId": chat_id,
          "messageId": message,
          "detail": reason,
        }));

        (None, None)
      }
      PiecesEnded::Answered { sent, keeping } => (sent, Some(keeping)),
    };

    if let Some(sent) = sent {
      let mut event = json!({
        "type": "sent",
        "chatId": chat_id,
        "messageId": sent.message_id,
        "detail": sent.text,
        "round": round,
      });

      if let Some(speaker) = self.state().speakers.get(&chat_id).cloned() {
        event["speaker"] = speaker;
      }

      self.publish(event);
      self.hand_filed(chat_id, sent.message_id, via, room).await;
    }

    if let Some(conv) = self.state().active.get_mut(&chat_id) {
      conv.answered = true;
    }

    // The room stays in flight until its window is written, so a room read as idle has its window moved.
    self.restart_window(chat_id).await;

    if let Some(conv) = self.state().active.get_mut(&chat_id) {
      conv.in_flight = false;
    }

    drop(keeping);
    self.pump_pending(chat_id);
  }

  // A round that comes to no turn: the room free for the next round, and nothing of it kept.
  pub fn no_turn(&self, chat_id: i64) {
    if let Some(conv) = self.state().active.get_mut(&chat_id) {
      conv.in_flight = false;
    }

    self.pump_pending(chat_id);
  }

  /// What came into a room while its round waited, taken now for the turn that waited: the words and the quiet
  /// instructions with the rows they came as.
  pub fn take_waiting(&self, chat_id: i64) -> Waiting {
    let mut state = self.state();
    let Some(conv) = state.active.get_mut(&chat_id) else {
      return Waiting::default();
    };
    let words = std::mem::take(&mut conv.buffer);
    let ids = std::mem::take(&mut conv.instruction_ids);
    let events = std::mem::take(&mut conv.instruction_events);
    let mut quiet: Vec<Quiet> = std::mem::take(&mut conv.instructions)
      .into_iter()
      .enumerate()
      .map(|(at, text)| Quiet {
        id: ids.get(at).copied().unwrap_or_default(),
        text,
        events: vec![],
      })
      .collect();

    conv.instruction_roads.clear();

    if let Some(first) = quiet.first_mut() {
      first.events = events;
    }

    Waiting { words, quiet }
  }

  // What the turn's session keeps of the room: the read mark the conversation holds, and whether every word it
  // took was the owner's, which only ever falls.
  fn keep_session(&self, chat_id: i64, kept: &Held) {
    if let Some(conv) = self.state().active.get_mut(&chat_id) {
      let consumed = conv.held.last_consumed_id;
      let all_from_owner = conv.held.all_from_owner && kept.all_from_owner;

      conv.held = Held {
        last_consumed_id: consumed,
        all_from_owner,
        replaced: vec![],
        ..kept.clone()
      };
    }
  }

  // A round that asked to wait: its words held for the round after, or, on a round that had no next one, the
  // conversation closed and the owner told nothing broke.
  async fn waited_round(
    &self,
    chat_id: i64,
    batch: &[Inbound],
    held: &[Inbound],
    final_round: bool,
    record: &mut Record,
  ) -> Result<(), Failure> {
    let (message, again, force) = {
      let mut state = self.state();
      let Some(conv) = state.active.get_mut(&chat_id) else {
        return Ok(());
      };

      conv.in_flight = false;

      if final_round {
        (conv.held.last_consumed_id, false, false)
      } else {
        let held_ids: HashSet<i64> = held.iter().map(|message| message.message_id).collect();
        let fresh: HashSet<i64> = batch
          .iter()
          .filter(|message| !held_ids.contains(&message.message_id))
          .map(held_person)
          .collect();
        let since = conv.held_since.clone();
        let now = now_ms();

        conv.held_words = batch.to_vec();
        conv.held_since.clear();

        for person in batch.iter().map(held_person).collect::<HashSet<_>>() {
          let at = if fresh.contains(&person) { now } else { since.get(&person).copied().unwrap_or(now) };

          conv.held_since.insert(person, at);
        }

        record.held_over = true;

        let again = conv.expired || !conv.buffer.is_empty() || !conv.instructions.is_empty();

        if !again && !conv.answered {
          self.arm_wait(conv, chat_id);
        }

        (conv.held.last_consumed_id, again, conv.expired)
      }
    };

    if final_round {
      self.close(chat_id).await;
      self.publish(waited(chat_id, message, Some("WAIT")));
      self.0.health.died(&self.0.receiver, chat_id).await;

      return Ok(());
    }

    if again {
      self.spawn_pump(chat_id, force);
      return Ok(());
    }

    self.publish(waited(chat_id, message, Some("WAIT")));

    Ok(())
  }

  // A round that ended in silence is written down as the ending it was, on the message it read.
  async fn record_skip(&self, chat_id: i64, message_id: Option<i64>) {
    let judgement = Judgement {
      chat: chat_id,
      message: message_id,
      stage: "skipped".to_owned(),
      verdict: "SKIP".to_owned(),
      detail: None,
      ms: None,
      round: rounds::current(),
    };

    if let Err(why) = self.0.receiver.units().gate.judge(&judgement).await {
      eprintln!("chat {chat_id}: the skip could not be written down: {why}");
    }

    self
      .publish(json!({ "type": "skipped", "chatId": chat_id, "messageId": message_id, "verdict": "SKIP" }));
  }

  // The answer a room asked for of this one, as a quiet instruction to the room that asked.
  async fn relay_answer(&self, chat_id: i64, answer: &str) {
    let db = self.0.receiver.db();
    let now = now_ms();
    let row = one(
      db,
      r#"SELECT "id", "reply_to", "opening" FROM "say_relays"
         WHERE "chat_id" = $1 AND "delivered_at" IS NULL AND "expires_at" > $2 ORDER BY "id" ASC LIMIT 1"#,
      vec![chat_id.into(), now.into()],
    )
    .await
    .ok()
    .flatten();
    let Some(row) = row else { return };
    let (Ok(id), Ok(reply_to), Ok(opening)) = (
      row.try_get::<i64>("", "id"),
      row.try_get::<i64>("", "reply_to"),
      row.try_get::<String>("", "opening"),
    ) else {
      return;
    };

    let _ = run(
      db,
      r#"UPDATE "say_relays" SET "delivered_at" = $1 WHERE "id" = $2"#,
      vec![now.into(), id.into()],
    )
    .await;
    println!("answer from chat {chat_id} relayed to chat {reply_to}");

    let text = fill_marks(
      &CHAT_ROUND.relay.answer,
      &[("chat", &chat_id.to_string()), ("opening", &opening), ("answer", answer)],
    );
    let queued = super::instructions::Queued {
      sort: Some("relay"),
      ..super::instructions::Queued::default()
    };

    if let Err(why) = super::instructions::queue(db, self.agent(), reply_to, &text, &queued).await {
      eprintln!("the relayed answer could not be queued: {}", why.detail);
    }
  }

  // What waits in the agent's queue behind this turn: the room's words not yet taken, the quiet instructions not
  // yet delivered, and the other rooms waiting their turn.
  async fn queue_line(&self, chat_id: i64) -> String {
    let words = self.pending(chat_id).len();
    let (held, rooms) = {
      let state = self.state();

      (
        state.active.get(&chat_id).map_or(0, |conv| conv.instructions.len()),
        state
          .active
          .iter()
          .filter(|(other, conv)| {
            **other != chat_id && (!conv.buffer.is_empty() || !conv.instructions.is_empty())
          })
          .count(),
      )
    };
    let waiting = one(
      self.0.receiver.db(),
      r#"SELECT count(*) AS "waiting" FROM "instructions"
         WHERE "delivered_at" IS NULL AND "handed_at" IS NULL AND "agent" = $1
           AND ("rides" = 0 OR ("ride_until" IS NOT NULL AND "ride_until" <= $2))"#,
      vec![self.agent().into(), now_ms().into()],
    )
    .await
    .ok()
    .flatten()
    .and_then(|row| row.try_get::<i64>("", "waiting").ok())
    .unwrap_or(0) as usize;
    let instructions = held + waiting;
    let lines = &CHAT_ROUND.queue_line;

    if words + instructions + rooms == 0 {
      return lines.nothing.clone();
    }

    fill_marks(
      &lines.waiting,
      &[
        ("words", &words.to_string()),
        ("instructions", &instructions.to_string()),
        ("rooms", &rooms.to_string()),
      ],
    )
  }

  // The words a room still has waiting for a round: its buffer and what the queue is gathering.
  fn pending(&self, chat_id: i64) -> Vec<i64> {
    let state = self.state();
    let buffered = state.active.get(&chat_id).map(|conv| conv.buffer.clone()).unwrap_or_default();
    let gathering = state.queue.get(&chat_id).cloned().unwrap_or_default();
    let mut ids: Vec<i64> = vec![];

    for message in buffered.iter().chain(gathering.iter()) {
      if !ids.contains(&message.message_id) {
        ids.push(message.message_id);
      }
    }

    ids
  }

  // A session past its threshold folded, as work of its own after the round: the room marked folding while it
  // runs, so its next round waits and the touch of the quiet rooms skips it.
  async fn maybe_fold(&self, chat_id: i64, room: Room, session: Option<&str>, ratio: Option<f64>) {
    if self.state().folding.contains(&chat_id) || !super::fold::past_fold(session, ratio) {
      return;
    }

    let this = self.clone();

    self.state().folding.insert(chat_id);
    self.0.receiver.news().begin_fold(chat_id);

    tokio::spawn(async move {
      let receiver = this.receiver();
      let held = this.state().active.get(&chat_id).map(|conv| conv.held.clone());

      if let Some(held) = held {
        this.0.noting.fold(&this, &receiver, this.agent(), chat_id, &held, room).await;
      }

      this.state().folding.remove(&chat_id);
      this.0.receiver.news().end_fold(chat_id);
    });
  }

  /// The keepalive's touch of the quiet rooms' sessions, on the idle stretch the pass names where it shortens
  /// it.
  pub async fn touch(&self, idle_override_ms: Option<i64>) -> TouchAnswer {
    let lock = self.0.turn.lock().await;
    let touched = super::touch::touch(&self.0.receiver, self, self.agent(), idle_override_ms).await;

    drop(lock);
    touched
  }

  /// What a line a person typed into the chat agent's program at `at_ms` goes with (docs/bmo-window.md,
  /// 「A line typed into the program」): a round of the owner's private chat holding the line as one new message
  /// of his, built by the round's own prompt builder over the window's own selection and opened against the
  /// program's session, which then holds what went in full. Nothing is asked and nothing waits behind a running
  /// round: the person's turn is already under way in the program, and its answer is the program's.
  pub async fn typed_line(&self, text: &str, session_id: &str, at_ms: i64) -> Result<String, Fault> {
    let receiver = self.receiver();
    let db = receiver.db();
    let owner = self.owner();
    let name = crate::receiver::reads::display_name(db, owner)
      .await?
      .unwrap_or_else(|| CHAT_ROUND.typed.nameless.clone());
    let batch = [super::typed::message(text, owner, &name, at_ms)];
    let pending: HashSet<i64> = self
      .state()
      .queue
      .get(&owner)
      .map(|gathering| gathering.iter().map(|message| message.message_id).collect())
      .unwrap_or_default();
    let mode = self.mode().await;
    let queue_line = self.queue_line(owner).await;
    let composed = prompt::compose(
      &receiver,
      &Round {
        agent: self.agent(),
        chat_id: owner,
        room: prompt::quiet_room(&receiver, owner, owner).await?,
        owner,
        batch: &batch,
        instructions: &[],
        instruction_ids: &[],
        events: &[],
        force: true,
        mode: &mode,
        tools_allowed: true,
        waited_on: &HashSet::new(),
        first_call: false,
        all_from_owner: true,
        session: Some(session_id),
        pending: &pending,
        queue_line: &queue_line,
        woken: None,
        reader: Some(receiver.roads()),
        round: None,
      },
    )
    .await?;
    let room = room_context(&receiver, owner, owner, None).await?;
    let written = window::composed(&composed.selection, &composed.prompt);
    let (sent, _) = sent_prompt(&receiver, Some(session_id), Some(&room), true, &written).await?;

    remember_held(&receiver, Some(session_id), session_id, Some(&room), true, &written).await?;

    Ok(super::typed::context(&sent))
  }

  /// A turn of a console talk (docs/unified-bmo.md, 「Console conversation in the private chat」): the owner
  /// speaking, so a round of his private chat on the session that chat resumes — his tools, his room's context,
  /// waiting its place behind whatever round is running or fold under way — whose answer is handed back rather
  /// than sent: no output gate reads it, no room sees it, and no room's window, read mark or typing moves for it.
  pub async fn talk(&self, request: &TalkRequest) -> Result<Option<String>, TalkFailure> {
    let _turn = self.0.turn.lock().await;

    while !self.state().folding.is_empty() {
      tokio::time::sleep(Duration::from_millis(50)).await;
    }

    super::talk::answer(self, request).await
  }

  /// The conversations' own clock: the failure orders nobody took up looked over on the health policy's
  /// minute, for as long as the process stands.
  pub fn keep_clocks(&self) {
    let this = self.clone();
    let every = Duration::from_secs(receiver_policy::chat_health::CHAT_HEALTH.order.check_seconds);

    tokio::spawn(async move {
      loop {
        tokio::time::sleep(every).await;
        this.check_failure_orders().await;
      }
    });
  }

  /// The service's own line for every failure order the agent cannot answer, asked on the agent's own clock.
  pub async fn check_failure_orders(&self) {
    self.0.orders.check_failure_orders(&self.0.receiver, self.agent());
  }
}

// What follows a round's turn on its conversation's behalf: the turn's picture published to the room's screens,
// at once where what the block draws changed and at most once a gap where only the counts did, and what the room
// says meanwhile put into the turn at its next look.
pub struct Follower {
  conversations: Conversations,
  chat_id: i64,
  drawn: Mutex<Option<(Value, i64)>>,
  offered: Mutex<Option<Offered>>,
}

impl Follower {
  pub fn new(conversations: &Conversations, chat_id: i64) -> Self {
    Self {
      conversations: conversations.clone(),
      chat_id,
      drawn: Mutex::new(None),
      offered: Mutex::new(None),
    }
  }

  fn offered(&self) -> MutexGuard<'_, Option<Offered>> {
    self.offered.lock().unwrap_or_else(PoisonError::into_inner)
  }

  // What the room queue delivered while the turn works, taken where the round's turn still takes words: into a
  // privileged round, only messages that are all the owner's. Instructions take the same road.
  pub async fn offer(&self) -> Option<String> {
    let chat_id = self.chat_id;
    let conversations = &self.conversations;
    let owner = conversations.owner();
    let (messages, instructions, ids, roads, events, room, holds, round, consumed, tools_allowed) = {
      let mut state = conversations.state();
      let conv = state.active.get_mut(&chat_id)?;
      let carried = conv.carried.as_ref()?;

      if !carried.open || carried.shut || carried.pushing {
        return None;
      }

      let keeps = !carried.privileged || conv.buffer.iter().all(|message| message.sender_id == Some(owner));
      let messages = if keeps { std::mem::take(&mut conv.buffer) } else { vec![] };
      let instructions = std::mem::take(&mut conv.instructions);
      let ids = std::mem::take(&mut conv.instruction_ids);
      let roads = std::mem::take(&mut conv.instruction_roads);
      let events = std::mem::take(&mut conv.instruction_events);

      if messages.is_empty() && instructions.is_empty() {
        conv.give_back(messages, instructions, ids, roads, events);
        return None;
      }

      let carried = conv.carried.as_mut()?;

      carried.pushing = true;

      (
        messages,
        instructions,
        ids,
        roads,
        events,
        conv.room,
        carried.holds.clone(),
        carried.round.clone(),
        conv.held.last_consumed_id,
        carried.tools_allowed,
      )
    };
    let built = super::mid_turn::paragraph(&Push {
      receiver: &conversations.0.receiver,
      agent: conversations.agent(),
      owner,
      chat_id,
      room,
      round: &round,
      messages: &messages,
      instructions: &instructions,
      events: &events,
      holds: &holds,
      consumed,
      tools_allowed,
      reader: Some(conversations.0.receiver.roads()),
    })
    .await;
    let mut state = conversations.state();
    let conv = state.active.get_mut(&chat_id)?;
    let open = conv.carried.as_ref().is_some_and(|carried| carried.open);

    match built {
      Ok(paragraph) if open => {
        let text = paragraph.text.clone();

        *self.offered() = Some(Offered {
          messages,
          instructions,
          ids,
          roads,
          events,
          paragraph,
        });

        Some(text)
      }
      built => {
        if let Err(why) = built {
          eprintln!("chat {chat_id}: words for the working turn could not be put in: {}", why.detail);
        }

        conv.give_back(messages, instructions, ids, roads, events);

        if let Some(carried) = conv.carried.as_mut() {
          carried.pushing = false;
        }

        None
      }
    }
  }

  // What the turn took joins the round's carried set; what it refused goes back to the front of what waits, and
  // a turn that says it is waiting takes no more.
  pub async fn settle(&self, outcome: AsideOutcome) {
    let chat_id = self.chat_id;
    let Some(offered) = self.offered().take() else { return };
    let conversations = &self.conversations;
    let taken = {
      let mut state = conversations.state();
      let Some(conv) = state.active.get_mut(&chat_id) else { return };

      if let Some(carried) = conv.carried.as_mut() {
        carried.pushing = false;
      }

      match (outcome, conv.carried.as_mut()) {
        (AsideOutcome::Delivered, Some(carried)) => {
          let Offered {
            messages,
            instructions,
            ids,
            roads,
            paragraph,
            ..
          } = offered;
          let counts = (messages.len(), instructions.len());

          carried.messages.extend(messages);
          carried.roads.extend(roads);
          carried.settling.extend(ids);
          carried.pushed.push(paragraph.text);
          carried.holds.extend(paragraph.reference);

          if !paragraph.instructions.is_empty() {
            carried.pushed_instructions.push(paragraph.instructions);
          }

          Some((carried.messages.clone(), counts))
        }
        (outcome, _) => {
          let Offered {
            messages,
            instructions,
            ids,
            roads,
            events,
            ..
          } = offered;

          conv.give_back(messages, instructions, ids, roads, events);

          if outcome == AsideOutcome::Waiting
            && let Some(carried) = conv.carried.as_mut()
          {
            carried.shut = true;
          }

          None
        }
      }
    };
    let Some((all, (messages, pushed))) = taken else { return };

    conversations.consume(chat_id, &all).await;

    println!(
      "chat {chat_id}: {messages} message(s) and {pushed} instruction(s) went into the working turn"
    );
  }

  // The round's turn is over: it takes no more, words offered and never answered for go back, and what the
  // pushes grew the round by is the round's to decide on.
  pub fn finish(&self) -> Option<Carried> {
    let offered = self.offered().take();
    let mut state = self.conversations.state();
    let conv = state.active.get_mut(&self.chat_id)?;

    if let Some(Offered {
      messages,
      instructions,
      ids,
      roads,
      events,
      ..
    }) = offered
    {
      conv.give_back(messages, instructions, ids, roads, events);
    }

    conv.carried.take()
  }

  // The turn's picture, published to the room's screens: at once where what the block draws changed, and at
  // most once a gap where only the counts did.
  pub fn progressed(&self, picture: Value) {
    let now = now_ms();

    {
      let mut drawn = self.drawn.lock().unwrap_or_else(PoisonError::into_inner);

      if let Some((last, at)) = drawn.as_ref()
        && *last == picture
        && now - at < CHAT_ROUND.progress.gap_ms as i64
      {
        return;
      }

      *drawn = Some((picture.clone(), now));
    }

    let message = self
      .conversations
      .state()
      .active
      .get(&self.chat_id)
      .and_then(|conv| conv.held.last_consumed_id);

    self.conversations.publish(
      json!({ "type": "run-progress", "chatId": self.chat_id, "messageId": message, "progress": picture }),
    );
  }

  // The paragraphs pushed into the round's turn so far, in the order they went in.
  pub fn pushed(&self) -> Vec<String> {
    self
      .conversations
      .state()
      .active
      .get(&self.chat_id)
      .and_then(|conv| conv.carried.as_ref())
      .map(|carried| carried.pushed.clone())
      .unwrap_or_default()
  }
}

// The person a wait stands on: the sender, or the room itself where no sender is known.
fn held_person(message: &Inbound) -> i64 {
  message.sender_id.unwrap_or(message.chat_id)
}

// Every event of a room goes out as the agent's own: under the speaker its turns there are shown as, unless the
// event already names its own.
pub fn told_as(mut event: Value, speaker: Option<Value>) -> Value {
  let named = event.get("type").and_then(Value::as_str) == Some("sent") || event.get("speaker").is_some();

  if !named && let Some(speaker) = speaker {
    event["speaker"] = speaker.clone();
  }

  event
}

// The room's screens told that its conversation is waiting on what it read, and nothing more has come.
fn waited(chat_id: i64, message_id: Option<i64>, verdict: Option<&str>) -> Value {
  json!({
    "type": "waited",
    "chatId": chat_id,
    "messageId": message_id,
    "verdict": verdict,
  })
}

// Whether one message of a room stands at or before another, by the moment it was sent and the order it was
// filed in rather than by its number, since a room's numbers come from the service and are not its own.
async fn stands_at_or_before(
  db: &DatabaseConnection,
  chat_id: i64,
  message_id: i64,
  other_id: i64,
) -> bool {
  let row = one(
    db,
    r#"SELECT (m."sent_at", m."rowid") <= (r."sent_at", r."rowid") AS "earlier"
       FROM "messages" m, "messages" r
       WHERE m."chat_id" = $1 AND m."message_id" = $2 AND r."chat_id" = $1 AND r."message_id" = $3"#,
    vec![chat_id.into(), message_id.into(), other_id.into()],
  )
  .await
  .ok()
  .flatten();

  match row {
    None => message_id <= other_id,
    Some(row) => row.try_get::<bool>("", "earlier").unwrap_or(message_id <= other_id),
  }
}

// The rows of the quiet instructions no round took, back in the queue.
async fn return_instructions(db: &DatabaseConnection, ids: Vec<i64>) {
  let placeholders = (1..=ids.len()).map(|at| format!("${at}")).collect::<Vec<_>>().join(", ");

  let query = format!(
    r#"UPDATE "instructions" SET "handed_at" = NULL WHERE "id" IN ({placeholders}) AND "delivered_at" IS NULL"#
  );

  if let Err(why) = run(db, &query, ids.iter().map(|id| (*id).into()).collect()).await {
    eprintln!("instructions could not be returned: {}", why.detail);
  }
}

// The rows of the quiet instructions a round took, settled under the round that ended.
async fn settle_instructions(db: &DatabaseConnection, now: i64, round: &str, ids: Vec<i64>) {
  let numbered = (1..=ids.len()).map(|at| format!("${}", at + 2)).collect::<Vec<_>>().join(", ");
  let query = format!(
    r#"UPDATE "instructions" SET "delivered_at" = $1, "round" = $2 WHERE "id" IN ({numbered}) AND "delivered_at" IS NULL"#
  );
  let mut values: Vec<sea_orm::Value> = vec![now.into(), round.into()];

  values.extend(ids.iter().map(|id| (*id).into()));

  if let Err(why) = run(db, &query, values).await {
    eprintln!("instructions could not be settled: {}", why.detail);
  }
}

/// Whether the agent's turns go on one session across its rooms, as its registration says.
pub async fn unified_session(receiver: &Receiver, agent: &str) -> bool {
  let listed = receiver.units().agency.agents().await.ok();

  listed
    .as_ref()
    .and_then(|listed| listed.agents.iter().find(|one| one.name == agent))
    .and_then(|found| found.row.get("session_scope").and_then(Value::as_str))
    .is_some_and(|scope| scope == "unified")
}

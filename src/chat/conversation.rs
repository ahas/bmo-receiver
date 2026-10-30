//! The conversations of one conversational agent, every room's in one (docs/wake.md; docs/bmo-window.md): what a
//! room says joins its queue and is handed over together once the queue's delay is up; the room's conversation
//! wakes, or takes the words into the round it is running. A round reads what is waiting, asks the session, makes
//! the message from its draft through the draft process, and sends it — or waits for more, or passes the room by —
//! and the room's window keeps it listening without being called while it stands.

use std::collections::{HashMap, HashSet};
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use tokio::task::JoinHandle;

use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::fill_marks;
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};

use super::ask::Carry;
use super::context::{remember_held, sent_prompt};
use super::draft::{Drafter, Plan};
use super::health::{FailureOrders, RoomHealth};
use super::mid_turn::{Paragraph, Push};
use super::prompt::{self, Round, Scope as RoundScope};
use super::reply::Sender;
use super::room::room_context;
use super::sentinels::{is_skip, is_wait};
use super::shape::Reading;
use super::talk::{TalkFailure, TalkRequest};
use super::thread::ThreadMark;
use super::touch::TouchAnswer;
use super::turnover::Noter;
use super::window::Selection;

use crate::Fault;
use crate::now_ms;
use crate::receiver::{Inbound, Receiver, Room, rows, wake};
use crate::roads::Road;
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
/// the session now holds against the window it holds it in, the run whose log holds its calls and what the session
/// held as the turn went out.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reply {
  pub text: String,
  pub mode: Option<String>,
  pub account: Option<String>,
  pub session: Option<String>,
  pub context_tokens: Option<i64>,
  pub context_window: Option<i64>,
  /// The share of the window the session folds at, as the agency gives it for its provider and tier.
  pub fold_ratio: Option<f64>,
  /// Whether the session's program carries its own fold, as the agency says.
  pub folds_itself: bool,
  /// The tier the agency names for the session, where it names one.
  pub tier: Option<String>,
  pub replaced: Vec<SessionReason>,
  pub run: Option<String>,
  /// What the session held as the turn went out, which what a round read is counted from.
  pub before: super::draft::Before,
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

/// Whether waiting words join a working round, by the senders of their people's words (docs/drafting.md, 「What came
/// in meanwhile」): into the owner's round where every person among them is he, an agent's line alone keeping
/// nothing out; into a round that is not his where they are not his alone, so his own words open the round after.
pub fn joins_round(privileged: bool, peoples: &[i64], owner: i64) -> bool {
  let all_his = peoples.iter().all(|sender| *sender == owner);

  if privileged { all_his } else { peoples.is_empty() || !all_his }
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
  /// The thread line this turn handed, kept under the session once the turn is delivered.
  pub thread_mark: Option<ThreadMark>,
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

/// What the draft process makes the message from (docs/drafting.md): the process the round opened, the answer its
/// turn gave to the first question, its scope, the words the round read and the words around it, how many quiet
/// instructions it carried, whether its turn may use tools and what it still owes the owner.
pub struct Drafted<'a> {
  pub plan: &'a Plan,
  pub reply: Reply,
  pub scope: &'a RoundScope,
  pub recent: &'a [Inbound],
  pub batch: &'a [Inbound],
  /// The room's messages pushed into the round's own turn, shown to the process from its start.
  pub pushed: &'a [Inbound],
  pub instructions: usize,
  pub tools_allowed: bool,
  pub owed: usize,
  /// The room the round answers, the road its answer goes back on, whether every word it took was the owner's,
  /// and the messages waiting for the next turn: what a turn of the process goes out with beside its question.
  pub room: Room,
  pub via: String,
  pub privileged: bool,
  pub pending: Vec<i64>,
  /// The round's own selection, which every turn of the process goes out with the same reference on.
  pub selection: &'a Selection,
  /// When the round began, in epoch milliseconds, which the round log's times count from; when its turn went
  /// out, which the ceiling counts from; and the round's name.
  pub began: i64,
  pub opened: i64,
  pub round: &'a str,
}

/// What the draft process made: the message, a skip where nothing goes out, with what the last turn carried of
/// the session; and the room's words that arrived meanwhile and were shown to it, as the agent's marks sort them
/// (docs/drafting.md, 「What came in meanwhile」).
pub struct Made {
  pub reply: Reply,
  /// The messages shown to the round that its message answers.
  pub took: Vec<Inbound>,
  /// The messages shown to the round that the agent marked as needing no answer.
  pub done: Vec<Inbound>,
  /// The messages shown to the round that the agent marked to be answered in a round of their own.
  pub separate: Vec<Inbound>,
  /// The message the agent marked as the one its answer answers, by its number, where it marked one.
  pub reply_to: Option<i64>,
}

/// The reply pin a turn goes out with (docs/drafting.md, 「The answer is pinned to the message it answers」): the
/// message the agent named, or one another line has come after; none for the newest line or an unnamed message.
pub fn visible_pin(target: Option<i64>, explicit: bool, newest: Option<i64>) -> Option<i64> {
  match target {
    Some(id) if explicit || newest != Some(id) => Some(id),
    _ => None,
  }
}

/// The message a round's answer is pinned to (docs/drafting.md, 「The answer is pinned to the message it
/// answers」): the one the agent named among those it answers; else the oldest of them that asks something of the
/// agent; else the newest.
pub fn pin_target(
  answered: &[Inbound],
  named: Option<i64>,
  asks: impl Fn(&Inbound) -> bool,
) -> Option<&Inbound> {
  named
    .and_then(|id| answered.iter().find(|message| message.message_id == id))
    .or_else(|| answered.iter().find(|message| asks(message)))
    .or_else(|| answered.last())
}

/// Whether a message asks something of an agent: it calls the agent by a word it answers to, or asks a question.
pub fn asks_of(agent: &wake::agents::Designated, message: &Inbound) -> bool {
  agent.called_by(&message.text) || message.text.contains('?') || message.text.contains('\u{ff1f}')
}

/// What makes a round's message (docs/drafting.md): the process a round opens before its turn goes out, whose
/// first question the turn carries, and the rest of it once the turn answered. Filled by the module that owns the
/// process.
pub trait Drafting: Send + Sync {
  fn plan<'a>(
    &'a self,
    agent: &'a str,
    chat_id: i64,
    batch: &'a [Inbound],
    instructions: usize,
  ) -> BoxFuture<'a, Plan>;

  fn draft<'a>(
    &'a self,
    conversations: &'a Conversations,
    agent: &'a str,
    held: &'a Held,
    drafted: &'a Drafted<'a>,
  ) -> BoxFuture<'a, Result<Made, Failure>>;
}

/// Where a round's answer goes out: the room it goes out in, the message it answers, the road it answers on, the
/// round that wrote it and when the room said the earliest thing it answers.
pub struct ReplyRoad {
  pub chat_id: i64,
  pub reply_to: Option<i64>,
  pub via: String,
  pub round: String,
  pub asked_at: Option<i64>,
  /// When the round began, in epoch milliseconds: its judgements are the ones written since.
  pub began: i64,
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
  reply_marked: Option<i64>,
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

/// The turn of a round that is working, as what is pushed into it grows it: whether the round is the owner's and
/// whether this turn takes quiet instructions as well as the room's words, the room's messages pushed into it and
/// waiting for the round's marks, the instructions it took with their roads and rows, the paragraphs pushed and the
/// instructions among them as the session read them, what it holds by path, and whether it still takes more.
pub struct Carried {
  round: String,
  tools_allowed: bool,
  privileged: bool,
  takes_instructions: bool,
  shown: Vec<Inbound>,
  roads: Vec<String>,
  settling: Vec<i64>,
  pushed: Vec<String>,
  pushed_instructions: Vec<String>,
  holds: HashSet<String>,
  open: bool,
  shut: bool,
  pushing: bool,
}

impl Carried {
  /// A working turn of the round `round` opened to pushes (docs/bmo-window.md, 「The openings」, `push`): the
  /// room's words where they keep the round's privilege, and the quiet instructions too where `takes_instructions`;
  /// `holds` is what the round already holds by path, `roads` the roads it was opened with.
  pub fn open(
    round: &str,
    tools_allowed: bool,
    privileged: bool,
    takes_instructions: bool,
    holds: HashSet<String>,
    roads: Vec<String>,
  ) -> Self {
    Carried {
      round: round.to_owned(),
      tools_allowed,
      privileged,
      takes_instructions,
      shown: vec![],
      roads,
      settling: vec![],
      pushed: vec![],
      pushed_instructions: vec![],
      holds,
      open: true,
      shut: false,
      pushing: false,
    }
  }

  /// The room's messages the turn took, shown to the round and settled by its marks (docs/drafting.md, 「What
  /// came in meanwhile」).
  pub fn into_shown(self) -> Vec<Inbound> {
    self.shown
  }
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
      reply_marked: None,
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
  // The sessions the journal was told carry their own fold, so it is told once for each.
  folds_itself: HashSet<String>,
  // The speaker each room's turns are shown as: the agent's name and the number its words are filed under there.
  speakers: HashMap<i64, Value>,
  // The agent's own runs going now, by name with the room each runs for, which a reset or a room's removal
  // stands its cancellation over.
  going: HashMap<String, i64>,
  // The messages of each room that reached the session on its open window alone, which the agent judges first.
  window_only: HashMap<i64, HashSet<i64>>,
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
  carrying: Arc<dyn Carrying>,
  drafting: Arc<dyn Drafting>,
  sending: Arc<dyn Sending>,
  noting: Arc<dyn Noting>,
  health: Arc<dyn Health>,
  orders: Arc<dyn Orders>,
}

/// One conversational agent's conversations, every room's in one: cloned freely, every clone the same.
#[derive(Clone)]
pub struct Conversations(Arc<Inner>);

impl Conversations {
  /// An agent's conversations on the receiver's own parts of a round: its turns carried into the agency's
  /// sessions, its answers made into messages by the draft process and sent on the road they answer on, its
  /// sessions noted and folded, and the health and failure orders of its rooms. The
  /// talk and the touch are the conversations' own, taken behind whatever round is running, and so is the typed
  /// line, composed at once.
  pub fn new(receiver: Receiver, hooks: Arc<dyn Hooks>, timings: Timings, agent: &str) -> Self {
    Self(Arc::new(Inner {
      agent: agent.to_owned(),
      carrying: Arc::new(Carry::looking_every(receiver.clone(), timings.look)),
      drafting: Arc::new(Drafter::new(receiver.clone())),
      sending: Arc::new(Sender::new(receiver.clone())),
      noting: Arc::new(Noter::new(receiver.clone())),
      health: Arc::new(RoomHealth::new(receiver.clone())),
      orders: Arc::new(FailureOrders::new(receiver.clone())),
      receiver,
      hooks,
      timings,
      state: Mutex::new(State::default()),
      turn: tokio::sync::Mutex::new(()),
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

  /// The owner as a sender.
  pub fn owner(&self) -> i64 {
    self.0.receiver.owner_id()
  }

  /// The owner's private room.
  pub fn owner_room(&self) -> i64 {
    self.0.receiver.owner_room()
  }

  /// Every event of a room goes out as the agent's own: under the speaker its turns there are shown as.
  pub(crate) fn publish(&self, event: Value) {
    let speaker = event
      .get("chatId")
      .and_then(Value::as_i64)
      .and_then(|chat_id| self.state().speakers.get(&chat_id).cloned());

    self.0.receiver.news().publish(&told_as(event, speaker));
  }

  // The speaker the agent's turns in a room are shown as, read again as each turn begins.
  async fn speaker_in(&self, chat_id: i64) {
    let speaker = speaker(&self.0.receiver, self.agent(), chat_id).await;

    self.state().speakers.insert(chat_id, speaker);
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

  /// A run of the agent's own in a room named as going until what this answers is dropped
  /// (docs/wake.md, 「An agent's reset」).
  pub fn going(&self, run: &str, chat_id: i64) -> Going {
    self.state().going.insert(run.to_owned(), chat_id);

    Going {
      conversations: self.clone(),
      run: run.to_owned(),
    }
  }

  /// Everything waiting for the agent's turns in every room let go unread, as its reset cuts it off: the words the
  /// queue gathers and those a conversation holds or waits on, and its quiet instructions, whose rows it answers
  /// with the runs of its own going now (docs/wake.md, 「An agent's reset」).
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

    let listed = self.0.receiver.agents().await.ok();
    let row = listed
      .as_ref()
      .and_then(|listed| listed.agents.iter().find(|one| one.name == self.agent()));
    // The policy's span stands wherever the agent's row names none, the row being found or not.
    let seconds = row
      .and_then(|row| row.row.get("windowSeconds").and_then(Value::as_i64))
      .unwrap_or(CHAT_ROUND.conversation.window_seconds);

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
      wake::agents::watch_window(&self.0.receiver, chat_id, self.agent(), until);
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
  /// the room's window now, so what the caller adds meanwhile is let in. A message the open window alone let in is
  /// marked so, for the round to judge first (docs/wake.md, 「The room's queue and the round」).
  pub async fn enqueue(&self, message: Inbound, room: Room, opens_window: bool, window_only: bool) {
    let chat_id = message.chat_id;

    if opens_window {
      self.open_window(chat_id).await;
    }

    let mut state = self.state();

    if window_only {
      state.window_only.entry(chat_id).or_default().insert(message.message_id);
    }

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

  /// Whether a round of the agent's stands in any room now.
  pub fn busy(&self) -> bool {
    self.state().active.values().any(|conv| conv.in_flight)
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

    // A turn closes the window it opened and never one it did not; one closed before its end is told closed now.
    let standing = self.window_until(chat_id).await;

    if standing <= conv.window_until {
      self.set_window(chat_id, 0).await;

      let now = now_ms();

      if standing > now {
        self.0.receiver.news().window_closed(chat_id, self.agent(), now);
      }
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

    // Every turn is shown as its agent's, from whatever it comes to: the default answerer as the room's own voice,
    // any other agent under the number its words are filed under.
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
    let (messages, instructions, read) = {
      let state = self.state();
      let Some(conv) = state.active.get(&chat_id) else {
        drop(state);
        drop(turn_lock);
        return;
      };

      let read: Vec<i64> = conv
        .held_words
        .iter()
        .chain(conv.buffer.iter())
        .map(|message| message.message_id)
        .collect();

      (conv.buffer.len(), conv.instructions.len(), read)
    };
    let reading = read.last().copied();
    let mut picture = json!({
      "chatId": chat_id,
      "messageId": reading,
      "messageIds": read,
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
      rounds::within(round.clone(), Box::pin(self.round(chat_id, force, &round, &mut record))).await;

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

    // A round that ended, however it ended, has dealt with the rows it took, and its turn's mark goes with it.
    if !settling.is_empty() {
      settle_instructions(self.0.receiver.db(), now_ms(), &round, settling).await;
    }

    let _ = super::turn_mark::forget(self.0.receiver.db(), &round).await;

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

  // The round's answer pinned to the message it answers among those it answers, where a road can aim at it and
  // the pin tells the reader which line it answers: an explicit mark, or a message another line has come after.
  async fn pin(&self, chat_id: i64, answered: &[Inbound], named: Option<i64>) {
    let db = self.0.receiver.db();
    let named_message = match named {
      Some(id) => match answered.iter().find(|message| message.message_id == id) {
        Some(message) => Some(message.clone()),
        None => rows::message(db, chat_id, id, "").await.ok().flatten(),
      },
      None => None,
    };
    let explicit = named_message.is_some();
    let message = match named_message {
      Some(message) => Some(message),
      None => {
        let agent = designated(&self.receiver(), self.agent()).await;

        pin_target(answered, None, |message| asks_of(&agent, message)).cloned()
      }
    };
    let Some(message) = message else {
      self.reply_target(chat_id, None);

      return;
    };
    let pins = self.0.receiver.roads().pinnable(&message.via, message.mode.as_deref()).await;
    let target = (!message.via_reader.unwrap_or(false) && pins).then_some(message.message_id);

    let newest = if !explicit && target.is_some() {
      rows::recent_messages(self.0.receiver.db(), chat_id, 1, "")
        .await
        .ok()
        .and_then(|mut recent| recent.pop())
        .map(|newest| newest.message_id)
    } else {
      None
    };

    self.reply_target(chat_id, visible_pin(target, explicit, newest));
  }

  // The message the next answer is pinned to, where one is: none where the answer is to the newest line or no
  // road can aim at it.
  fn reply_target(&self, chat_id: i64, target: Option<i64>) {
    if let Some(conv) = self.state().active.get_mut(&chat_id) {
      conv.reply_target = target;
    }
  }

  // Whether every instruction this round carries is an agent-only notice: a record the agent reads and no room
  // hears. A round of such rows alone sends no message (docs/bmo-window.md, 「Quiet instruction」).
  async fn agent_only_round(receiver: &Receiver, ids: &[i64]) -> bool {
    if ids.is_empty() {
      return false;
    }

    for id in ids {
      let row = match one(
        receiver.db(),
        r#"SELECT "agent_only" FROM "instructions" WHERE "id" = $1"#,
        vec![(*id).into()],
      )
      .await
      {
        Ok(Some(row)) => row,
        _ => return false,
      };

      if !row.try_get::<bool>("", "agent_only").unwrap_or(false) {
        return false;
      }
    }

    true
  }

  // One round, from what is waiting to whatever it ends in.
  async fn round(
    &self,
    chat_id: i64,
    force: bool,
    round: &str,
    record: &mut Record,
  ) -> Result<(), Failure> {
    let receiver = self.receiver();
    let owner = self.owner();

    while self.state().folding.contains(&chat_id) {
      tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // The round begins here: every judgement the gate writes of it, its reference's among them, comes after.
    let began = now_ms();

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

    // The round's privilege is its own people's words', in any room and whatever an earlier round of the
    // conversation read; an agent's line neither grants nor takes it, and a round of no people's words is the
    // owner's in his private room alone (docs/unified-bmo.md).
    let peoples = self.peoples_senders(chat_id, &batch, room).await;

    held.all_from_owner = if peoples.is_empty() {
      room.owner_room
    } else {
      peoples.iter().all(|sender| *sender == owner)
    };
    let mut via = self.road_of(chat_id, &batch, &roads).await;

    // Every round with words updates the room's situation note first, the owner's private chat included; the
    // judgement and the draft then read the note as it now stands.
    if !batch.is_empty() && receiver.situation_updates() {
      self.situation_update(chat_id, &batch, &mut held, &via).await?;
    }

    // A round the open window alone let in is the agent's to judge first, before any reference or gate;
    // a skip ends it there with nothing sent.
    if self.judged_first(chat_id, room, &batch, instructions.len())
      && self.supervise(chat_id, round, &batch, &mut held, &via).await?
    {
      self.consume(chat_id, &batch).await;
      held.last_consumed_id = self.state().active.get(&chat_id).and_then(|conv| conv.held.last_consumed_id);

      return self.silent_end(chat_id, &batch, &held).await;
    }

    let owed = if chat_id == self.owner_room() {
      batch.iter().filter(|message| message.sender_id == Some(owner)).count()
    } else {
      0
    };

    let final_round = force || (batch.is_empty() && !instructions.is_empty());
    let tools_allowed = true;
    let waited_on: HashSet<i64> = held_words.iter().map(|message| message.message_id).collect();
    let pending: HashSet<i64> = self.pending(chat_id).into_iter().collect();
    let queue_line = self.queue_line(chat_id).await;
    let woken = self.state().woken.get(&chat_id).cloned();
    // The process is read before the turn goes out, since the turn carries its first question.
    let plan = self.0.drafting.plan(self.agent(), chat_id, &batch, instructions.len()).await;
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
        questioned: plan.questioned(),
      },
    )
    .await
    .map_err(|why| Failure::Failed(why.detail))?;
    let prompt = match plan.first_words() {
      Some(question) => format!("{}\n\n{question}", composed.prompt),
      None => composed.prompt.clone(),
    };

    if let Some(conv) = self.state().active.get_mut(&chat_id) {
      conv.first_call = false;
    }

    self.consume(chat_id, &batch).await;
    held.last_consumed_id = self.state().active.get(&chat_id).and_then(|conv| conv.held.last_consumed_id);

    let follower = Follower::new(self, chat_id);

    self.carry_turn(
      chat_id,
      Carried::open(
        round,
        tools_allowed,
        held.all_from_owner,
        true,
        composed.selection.entries.iter().map(|entry| entry.path.clone()).collect(),
        roads.clone(),
      ),
    );

    let started = now_ms();
    let asking = Asking {
      chat_id,
      session: held.session.clone().unwrap_or_default(),
      prompt: prompt.clone(),
      via: match via.as_str() {
        "" => None,
        road => Some(road.to_owned()),
      },
      privileged: held.all_from_owner,
      answers: batch.iter().map(|message| message.message_id).collect(),
      began: started,
      context: composed.context.clone(),
      thread_mark: composed.selection.thread_mark.clone(),
    };

    // The first question is told as it starts, as every later one is.
    if let Some((question, _)) = plan.questions.first() {
      super::draft::log::tell_start(self, round, chat_id, self.agent(), 1, plan.questions.len(), question);
    }

    let asked = self.0.carrying.carry(&asking, &held, &follower).await;
    let carried = follower.finish();
    let mut scope = composed.scope.clone();
    let recent = composed.recent.clone();
    let mut pushed: Vec<Inbound> = vec![];

    // The instructions the turn took while it worked are the round's as much as those it froze, so the road the
    // answer goes back on and the scope read the grown set. The room's messages it took are shown to the round,
    // and the marks the process reads settle them (docs/bmo-window.md, 「The openings」, `push`).
    if let Some(carried) = carried {
      record.settling.extend(carried.settling.iter().copied());

      if carried.roads.len() > roads.len() {
        via = self.road_of(chat_id, &batch, &carried.roads).await;
      }

      if !carried.pushed_instructions.is_empty() {
        scope.instructions = std::iter::once(scope.instructions.clone())
          .chain(carried.pushed_instructions.iter().cloned())
          .filter(|text| !text.is_empty())
          .collect::<Vec<_>>()
          .join("\n\n");
        instructions.extend(carried.pushed_instructions.iter().cloned());
      }

      pushed = carried.into_shown();
    }

    // A turn whose run failed showed nothing: what was pushed into it goes back to wait.
    if asked.is_err() {
      self.give_back_words(chat_id, std::mem::take(&mut pushed));
    }

    let reply = asked?;
    let stripped = crate::form::strip_prompt_scaffolding(&reply.text);
    let mut reply = Reply {
      text: if stripped.is_empty() { super::sentinels::SKIP.to_owned() } else { stripped },
      ..reply.clone()
    };
    let ratio = reply.context_ratio();
    let fold_at = reply.fold_ratio;
    let folds_itself = reply.folds_itself;

    self.0.noting.note(&receiver, self.agent(), &held, &reply, now_ms() - started).await;
    held.context_tokens = reply.context_tokens.or(held.context_tokens);
    held.session = reply.session.clone().or(held.session);
    held.replaced = reply.replaced.clone();

    if let Some(reason) = held.replaced.last() {
      self.state().woken.insert(chat_id, reason.to_string());
    }

    self.keep_session(chat_id, &held);

    // An agent-only notice alone in a round is a quiet turn: the agent reads it, and nothing it answers goes into
    // the room. A room notice, or a message pushed with it, keeps the round a room's (docs/bmo-window.md, 「Quiet
    // instruction」).
    if batch.is_empty()
      && pushed.is_empty()
      && !instructions.is_empty()
      && Self::agent_only_round(&receiver, &record.settling).await
    {
      return self.silent_end(chat_id, &batch, &held).await;
    }

    // A wait holds what was pushed with the rest of the round's words.
    if is_wait(&reply.text) {
      if !pushed.is_empty() {
        batch.extend(pushed);
        self.consume(chat_id, &batch).await;
      }

      return self.waited_round(chat_id, &batch, &held_words, final_round, record).await;
    }

    let made = self
      .0
      .drafting
      .draft(
        self,
        self.agent(),
        &held,
        &Drafted {
          plan: &plan,
          reply: reply.clone(),
          scope: &scope,
          recent: &recent,
          batch: &batch,
          pushed: &pushed,
          instructions: instructions.len(),
          tools_allowed,
          owed,
          room,
          via: via.to_owned(),
          privileged: held.all_from_owner,
          pending: self.pending(chat_id),
          selection: &composed.selection,
          began,
          opened: started,
          round,
        },
      )
      .await?;

    reply = made.reply;

    // A message marked to be answered on its own goes back to wait, and opens the round after this one.
    self.give_back_words(chat_id, made.separate);

    // The words shown to the process are the round's own: the read mark moves past those marked done and to those
    // the message answers, and the answer is pinned to the newest of them.
    if !made.took.is_empty() || !made.done.is_empty() {
      let read: Vec<Inbound> =
        made.done.into_iter().chain(batch.iter().cloned()).chain(made.took.clone()).collect();

      batch.extend(made.took);
      self.consume(chat_id, &read).await;
      held.last_consumed_id = self.state().active.get(&chat_id).and_then(|conv| conv.held.last_consumed_id);
    }

    // The answer is pinned to the message it answers, whatever the owner is owed; a mark the supervisor's
    // first answer set stands in where this answer names none.
    let named = made
      .reply_to
      .or_else(|| self.state().active.get(&chat_id).and_then(|conv| conv.reply_marked));
    self.pin(chat_id, &batch, named).await;

    if let Some(conv) = self.state().active.get_mut(&chat_id) {
      conv.reply_marked = None;
    }

    // The fold reads the context the process's last turn left.
    let ratio = reply.context_ratio().or(ratio);
    let fold_at = reply.fold_ratio.or(fold_at);
    let folds_itself = reply.folds_itself || folds_itself;

    held.context_tokens = reply.context_tokens.or(held.context_tokens);

    if is_skip(&reply.text) {
      return self.silent_end(chat_id, &batch, &held).await;
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
            began,
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

    // A notice turn of the owner's private chat is heard as well as read (docs/voiced-notices.md, 「The summary of
    // a notice turn」): nobody spoke in its round, and it sent a line.
    if let Some(sent) = &sent
      && chat_id == self.owner_room()
      && batch.is_empty()
      && !instructions.is_empty()
      && super::briefing::speaks(&receiver, self.agent()).await
    {
      self.summarise(chat_id, &mut held, &via, sent).await;
    }

    if let Some(conv) = self.state().active.get_mut(&chat_id) {
      conv.in_flight = false;
    }

    self
      .maybe_fold(chat_id, room, held.session.as_deref(), ratio, fold_at, folds_itself)
      .await;
    self.pump_pending(chat_id);

    Ok(())
  }

  // A round that ends in silence: its session kept, its skip written down and the round over, as an answered round
  // is once its answer has gone out; a round of the system's lines alone closes a conversation nothing keeps open.
  async fn silent_end(&self, chat_id: i64, batch: &[Inbound], held: &Held) -> Result<(), Failure> {
    self.keep_session(chat_id, held);

    let consumed = {
      let mut state = self.state();
      let Some(conv) = state.active.get_mut(&chat_id) else {
        return Ok(());
      };

      conv.asked_at = None;
      conv.held.last_consumed_id
    };

    self.persist(held).await;
    self.record_skip(chat_id, (!batch.is_empty()).then_some(consumed).flatten()).await;

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
    Ok(())
  }

  // Whether a round is the agent's to judge first: words of a room that is not the owner's chat, every one of
  // them let in by the open window alone, with no quiet instruction beside them. The marks are spent either way.
  fn judged_first(&self, chat_id: i64, room: Room, batch: &[Inbound], instructions: usize) -> bool {
    let mut state = self.state();
    let marked = state.window_only.entry(chat_id).or_default();
    let alone = !batch.is_empty() && batch.iter().all(|message| marked.contains(&message.message_id));

    for message in batch {
      marked.remove(&message.message_id);
    }

    alone && instructions == 0 && !room.owner_room && chat_id != self.owner_room()
  }

  // The room's situation note updated from the words this round judges, one light turn of the same session
  // (docs/drafting.md, 「The situation note」): the note as it stands and the messages that came in since it last
  // read the room's record, in the policy's words; its answer is the note, kept with the row it read to and
  // attached to every message judged. Every round with words takes it, the owner's private chat included.
  async fn situation_update(
    &self,
    chat_id: i64,
    batch: &[Inbound],
    held: &mut Held,
    via: &str,
  ) -> Result<(), Failure> {
    let receiver = self.receiver();
    // What the note has not read yet: the room's record after the row it read to, or, where the room has no note
    // yet, the latest messages the policy names, so the first note has a room to stand on. A gap longer than the
    // policy's bound is read as its newest messages alone, so a room left long is caught up from its latest words
    // and the mark moves to the newest of them. The words this round holds are shown with them, whether or not the
    // room's record carries them yet.
    let cursor = super::situation::read_through(receiver.db(), chat_id).await.unwrap_or(None);
    let gap = CHAT_ROUND.situation.gap_messages;
    let rows = match cursor {
      Some(after) => {
        let standing = rows::count_since_row(receiver.db(), chat_id, after).await;

        match standing {
          Ok(standing) if super::situation::catch_up(gap, standing) => {
            rows::recent_rows(receiver.db(), chat_id, gap as u64, "").await
          }
          _ => rows::messages_since_row(receiver.db(), chat_id, after, "").await,
        }
      }
      None => rows::recent_rows(receiver.db(), chat_id, CHAT_ROUND.recent_messages, "").await,
    }
    .map_err(|why| Failure::Failed(why.detail))?;
    let through = rows.iter().map(|(row, _)| *row).max();
    let mut messages: Vec<Inbound> = rows.into_iter().map(|(_, message)| message).collect();

    for message in batch {
      if !messages.iter().any(|shown| shown.message_id == message.message_id) {
        messages.push(message.clone());
      }
    }

    messages.sort_by_key(|message| (message.sent_at, message.message_id));

    let shown: HashSet<i64> = messages.iter().map(|message| message.message_id).collect();
    let reading = Reading {
      receiver: &receiver,
      owner: self.owner(),
      fetch_on_demand: false,
      shown: &shown,
    };
    let lines = reading.lines(&messages).await.map_err(|why| Failure::Failed(why.detail))?;
    let old = super::situation::read(receiver.db(), chat_id).await.unwrap_or(None);
    let prompt = super::situation::update_prompt(old.as_deref(), &lines);
    let asking = Asking {
      chat_id,
      session: held.session.clone().unwrap_or_default(),
      prompt,
      via: (!via.is_empty()).then(|| via.to_owned()),
      privileged: held.all_from_owner,
      answers: batch.iter().map(|message| message.message_id).collect(),
      began: now_ms(),
      context: vec![],
      thread_mark: None,
    };
    let follower = Follower::new(self, chat_id);
    let reply = self.0.carrying.carry(&asking, held, &follower).await?;

    follower.finish();
    held.context_tokens = reply.context_tokens.or(held.context_tokens);
    held.session = reply.session.clone().or(held.session.clone());
    held.replaced = reply.replaced.clone();
    self.keep_session(chat_id, held);

    let note = super::situation::note_from_answer(&reply.text, &lines).or(old.clone());
    let changed = note.is_some() && note != old;

    if let Some(note) = &note
      && let Err(why) = super::situation::write(receiver.db(), chat_id, note, through).await
    {
      eprintln!("chat {chat_id}: the situation note was not kept: {}", why.detail);
    }

    if let Err(why) = super::situation::attach(
      receiver.db(),
      chat_id,
      &batch.iter().map(|message| message.message_id).collect::<Vec<_>>(),
      note.as_deref(),
      changed,
    )
    .await
    {
      eprintln!("chat {chat_id}: the situation note was not kept on the judged messages: {}", why.detail);
    }

    Ok(())
  }

  // The agent's own judgement of whether to answer words that did not name it (docs/wake.md, 「The room's queue and
  // the round」): one light turn of the same session carrying the room's recent words and the words judged, in the
  // room shape, and the round policy's question; no reference and no gate. Its row stands in the
  // round log of its own kind. Answers whether the agent passed the words over.
  async fn supervise(
    &self,
    chat_id: i64,
    round: &str,
    batch: &[Inbound],
    held: &mut Held,
    via: &str,
  ) -> Result<bool, Failure> {
    let receiver = self.receiver();
    let said = &CHAT_ROUND.supervisor;
    let judged: HashSet<i64> = batch.iter().map(|message| message.message_id).collect();
    let recent: Vec<Inbound> =
      rows::recent_messages(receiver.db(), chat_id, CHAT_ROUND.recent_messages, "")
        .await
        .map_err(|why| Failure::Failed(why.detail))?
        .into_iter()
        .filter(|message| !judged.contains(&message.message_id))
        .collect();
    let shown: HashSet<i64> =
      recent.iter().map(|message| message.message_id).chain(judged.iter().copied()).collect();
    let reading = Reading {
      receiver: &receiver,
      owner: self.owner(),
      fetch_on_demand: false,
      shown: &shown,
    };
    let recent_lines = reading.lines(&recent).await.map_err(|why| Failure::Failed(why.detail))?;
    let judged_lines = reading.lines(batch).await.map_err(|why| Failure::Failed(why.detail))?;
    let opening = super::draft::table::supervisor_text(receiver.db())
      .await
      .map_err(|why| Failure::Failed(why.detail))?;
    let note = super::situation::read(receiver.db(), chat_id).await.unwrap_or(None);
    let mut parts = vec![said.structure.clone(), String::new(), opening, String::new()];

    if let Some(note) = &note {
      parts.push(said.situation.clone());
      parts.push(note.clone());
      parts.push(String::new());
    }

    parts.push(said.recent.clone());
    parts.extend(recent_lines);
    parts.push(String::new());
    parts.push(said.judged.clone());
    parts.extend(judged_lines);

    let started = now_ms();
    let asking = Asking {
      chat_id,
      session: held.session.clone().unwrap_or_default(),
      prompt: parts.join("\n"),
      via: (!via.is_empty()).then(|| via.to_owned()),
      privileged: held.all_from_owner,
      answers: batch.iter().map(|message| message.message_id).collect(),
      began: started,
      context: vec![],
      thread_mark: None,
    };
    let follower = Follower::new(self, chat_id);
    let reply = self.0.carrying.carry(&asking, held, &follower).await?;

    follower.finish();
    held.context_tokens = reply.context_tokens.or(held.context_tokens);
    held.session = reply.session.clone().or(held.session.clone());
    held.replaced.extend(reply.replaced.clone());
    self.keep_session(chat_id, held);

    let passed = is_skip(&crate::form::strip_prompt_scaffolding(&reply.text));

    if !passed
      && let Some(id) = super::sentinels::round_marks(&reply.text)
        .into_iter()
        .rev()
        .find(|(mark, _)| *mark == super::sentinels::Mark::Reply)
        .map(|(_, id)| id)
      && let Some(conv) = self.state().active.get_mut(&chat_id)
    {
      conv.reply_marked = Some(id);
    }

    let mut log = super::draft::log::Log::new(self, round, chat_id, self.agent()).numbered_from(0);

    log
      .turn(
        super::draft::log::Kind::Supervisor,
        &said.name,
        Some(&reply.text),
        if passed {
          super::draft::log::Answered::Skip
        } else {
          super::draft::log::Answered::Answered
        },
        now_ms() - started,
      )
      .await;

    Ok(passed)
  }

  // A notice turn's summary, written by the same session in one light turn of the summary's quiet instruction once
  // the line has gone out, and handed to the voice engine with the line it speaks; a session that gives none has
  // the line's opening spoken.
  async fn summarise(&self, chat_id: i64, held: &mut Held, via: &str, sent: &Sent) {
    let receiver = self.receiver();
    let asking = Asking {
      chat_id,
      session: held.session.clone().unwrap_or_default(),
      prompt: fill_marks(&CHAT_ROUND.voice.summary, &[("line", &sent.text)]),
      via: (!via.is_empty()).then(|| via.to_owned()),
      privileged: held.all_from_owner,
      answers: vec![],
      began: now_ms(),
      context: vec![],
      thread_mark: None,
    };
    let follower = Follower::new(self, chat_id);
    let answer = match self.0.carrying.carry(&asking, held, &follower).await {
      Ok(reply) => {
        held.context_tokens = reply.context_tokens.or(held.context_tokens);
        held.session = reply.session.clone().or(held.session.clone());
        held.replaced = reply.replaced.clone();
        self.keep_session(chat_id, held);

        Some(crate::form::strip_prompt_scaffolding(&reply.text))
      }
      Err(why) => {
        eprintln!("notice turn in {chat_id}: its summary was not written ({why:?}); its opening is spoken");

        None
      }
    };

    follower.finish();
    super::briefing::speak_summary(
      &receiver,
      self.agent(),
      chat_id,
      sent.message_id,
      &sent.text,
      answer.as_deref(),
    )
    .await;
  }

  /// A line written as a spoken script by the agent's own session in the line's room (docs/voiced-notices.md,
  /// 「The full reading on a line's chip」): one light turn of the reading's quiet instruction, held behind any
  /// round of the agent's; none where the session wrote nothing or the turn failed.
  pub async fn spoken(&self, chat_id: i64, line: &str) -> Option<String> {
    // A session past its cache lifetime is never woken (docs/unified-bmo.md, 「Replacing the session」).
    if super::cache::expired(self.receiver().db(), self.agent()).await {
      return None;
    }

    let _turn = self.0.turn.lock().await;
    let mut held = self.held_session(chat_id).await;
    let asking = Asking {
      chat_id,
      session: String::new(),
      prompt: fill_marks(&CHAT_ROUND.voice.reading, &[("line", line)]),
      via: None,
      privileged: false,
      answers: vec![],
      began: now_ms(),
      context: vec![],
      thread_mark: None,
    };
    let follower = Follower::new(self, chat_id);
    let carried = self.0.carrying.carry(&asking, &held, &follower).await;

    follower.finish();

    match carried {
      Ok(reply) => {
        held.session = reply.session.clone();
        held.replaced = reply.replaced.clone();
        self.keep_session(chat_id, &held);

        let text = crate::form::strip_prompt_scaffolding(&reply.text);

        (!text.trim().is_empty() && !is_skip(&text)).then_some(text)
      }
      Err(why) => {
        eprintln!(
          "chat {chat_id}: a line's spoken script was not written ({why:?}); it is spoken as it stands"
        );

        None
      }
    }
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

  /// The words that came into a room while its round's draft process ran, taken for the process's next question or
  /// its closing check where they carry the round's privilege, read by people's words alone (docs/drafting.md,
  /// 「What came in meanwhile」): into the owner's round, words whose people's words are all his; into a round that is
  /// not his, never words that are his alone. The others wait and open the round after.
  pub async fn meanwhile(&self, chat_id: i64, privileged: bool) -> Vec<Inbound> {
    let owner = self.owner();
    let (waiting, room) = {
      let state = self.state();
      let Some(conv) = state.active.get(&chat_id) else {
        return vec![];
      };

      (conv.buffer.clone(), conv.room)
    };

    if waiting.is_empty() {
      return vec![];
    }

    let peoples = self.peoples_senders(chat_id, &waiting, room).await;

    if !joins_round(privileged, &peoples, owner) {
      return vec![];
    }

    // Exactly the words looked at go; any that came since wait for the next look.
    let mut state = self.state();
    let Some(conv) = state.active.get_mut(&chat_id) else {
      return vec![];
    };
    let looked: HashSet<i64> = waiting.iter().map(|message| message.message_id).collect();
    let (took, kept): (Vec<Inbound>, Vec<Inbound>) = std::mem::take(&mut conv.buffer)
      .into_iter()
      .partition(|message| looked.contains(&message.message_id));

    conv.buffer = kept;
    took
  }

  /// Words the process showed and did not answer for, back at the front of what waits.
  pub fn give_back_words(&self, chat_id: i64, words: Vec<Inbound>) {
    if let Some(conv) = self.state().active.get_mut(&chat_id) {
      conv.buffer.splice(0..0, words);
    }
  }

  /// A turn of the room's round about to go out opened to what is pushed into it while it works; its follower's
  /// finish closes it again.
  pub fn carry_turn(&self, chat_id: i64, carried: Carried) {
    if let Some(conv) = self.state().active.get_mut(&chat_id) {
      conv.carried = Some(carried);
    }
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
      room_id: chat_id,
      message_id,
      gate_name: "skipped".to_owned(),
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
    let mut parts: Vec<String> = Vec::new();

    for (count, line) in [(words, &lines.words), (instructions, &lines.instructions), (rooms, &lines.rooms)]
    {
      if count > 0 {
        parts.push(fill_marks(line, &[("count", &count.to_string())]));
      }
    }

    if parts.is_empty() {
      return lines.nothing.clone();
    }

    fill_marks(&lines.waiting, &[("parts", &parts.join(" · "))])
  }

  // The words a room still has waiting for a round: its buffer and what the queue is gathering.
  /// The messages waiting for a room's next round: those its conversation holds and those its queue gathers.
  /// The words a room's conversation holds for its next look, those the queue still gathers left out: what a
  /// round's next question or closing check takes.
  pub fn buffered(&self, chat_id: i64) -> Vec<i64> {
    self
      .state()
      .active
      .get(&chat_id)
      .map(|conv| conv.buffer.iter().map(|message| message.message_id).collect())
      .unwrap_or_default()
  }

  pub fn pending(&self, chat_id: i64) -> Vec<i64> {
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
  async fn maybe_fold(
    &self,
    chat_id: i64,
    room: Room,
    session: Option<&str>,
    ratio: Option<f64>,
    fold_at: Option<f64>,
    folds_itself: bool,
  ) {
    // A fold that is not made says why, unless the context is simply under the share (docs/wake.md).
    let Some(held) = session else {
      eprintln!("chat {chat_id}: no fold: the round holds no session");
      return;
    };

    // A session whose program carries its own fold is never folded here, and the journal says so once for it.
    if folds_itself {
      if self.state().folds_itself.insert(held.to_owned()) {
        eprintln!("chat {chat_id}: no fold of session {held}: the program carries its fold");
      }

      return;
    }
    let Some(carried) = ratio else {
      eprintln!("chat {chat_id}: no fold judged: the state of session {held} names no context");
      return;
    };

    if !super::fold::past_fold(session, ratio, fold_at) {
      return;
    }

    if self.state().folding.contains(&chat_id) {
      eprintln!(
        "chat {chat_id}: no fold of session {held} at {carried:.2}: a fold of the room is under way"
      );
      return;
    }

    // A session past its cache lifetime is not folded: the fold would pay its whole prefix again for nothing.
    if super::cache::expired(self.0.receiver.db(), self.agent()).await {
      eprintln!("chat {chat_id}: no fold of session {held} at {carried:.2}: it is past its cache lifetime");
      return;
    }

    eprintln!("chat {chat_id}: folding session {held} at {carried:.2} of its window");

    let this = self.clone();

    self.state().folding.insert(chat_id);
    self.0.receiver.news().begin_fold(chat_id);

    tokio::spawn(async move {
      let receiver = this.receiver();
      let held = this.state().active.get(&chat_id).map(|conv| conv.held.clone());

      match held {
        Some(held) => this.0.noting.fold(&this, &receiver, this.agent(), chat_id, &held, room).await,
        None => eprintln!("chat {chat_id}: no fold: the room's conversation ended before its fold began"),
      }

      this.state().folding.remove(&chat_id);
      this.0.receiver.news().end_fold(chat_id);
    });
  }

  // The senders of a batch's people's words: every message but an agent's line, which the record marks with its
  // agent, which stands under no sender, or under the agent's own number in the room.
  async fn peoples_senders(&self, chat_id: i64, batch: &[Inbound], room: Room) -> Vec<i64> {
    let ids: Vec<sea_orm::Value> = batch.iter().map(|message| message.message_id.into()).collect();
    let agents: HashSet<i64> = if ids.is_empty() {
      HashSet::new()
    } else {
      let placeholders = (2..ids.len() + 2).map(|n| format!("${n}")).collect::<Vec<_>>().join(", ");
      let values = std::iter::once(chat_id.into()).chain(ids).collect();

      crate::all(
        self.0.receiver.db(),
        &format!(
          r#"SELECT "message_id" FROM "messages" WHERE "chat_id" = $1 AND "agent_name" IS NOT NULL
             AND "message_id" IN ({placeholders})"#
        ),
        values,
      )
      .await
      .map(|rows| rows.iter().filter_map(|row| row.try_get::<i64>("", "message_id").ok()).collect())
      .unwrap_or_default()
    };

    batch
      .iter()
      .filter(|message| !agents.contains(&message.message_id))
      .filter_map(|message| message.sender_id)
      .filter(|sender| Some(*sender) != room.identity)
      .collect()
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
    let room_id = self.owner_room();
    let name = crate::receiver::reads::display_name(db, owner)
      .await?
      .unwrap_or_else(|| CHAT_ROUND.typed.nameless.clone());
    let batch = [super::typed::message(text, room_id, owner, &name, at_ms)];
    let pending: HashSet<i64> = self
      .state()
      .queue
      .get(&room_id)
      .map(|gathering| gathering.iter().map(|message| message.message_id).collect())
      .unwrap_or_default();
    let queue_line = self.queue_line(room_id).await;
    let composed = prompt::compose(
      &receiver,
      &Round {
        agent: self.agent(),
        chat_id: room_id,
        room: prompt::quiet_room(&receiver, room_id).await?,
        owner,
        batch: &batch,
        instructions: &[],
        instruction_ids: &[],
        events: &[],
        force: true,
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
        questioned: false,
      },
    )
    .await?;
    let room = room_context(&receiver, room_id, None).await?;
    let written = composed.prompt.clone();
    let (sent, _) = sent_prompt(&receiver, Some(session_id), Some(&room), true, &written).await?;

    remember_held(
      &receiver,
      Some(session_id),
      session_id,
      Some(&room),
      true,
      &written,
      composed.selection.thread_mark,
    )
    .await?;

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

  // What came for the room while the turn works, taken where the turn still takes it (docs/bmo-window.md, 「The
  // openings」, `push`): the room's words where they keep the round's privilege, as a question boundary takes
  // them, and the quiet instructions delivered meanwhile where the turn takes those too.
  pub async fn offer(&self) -> Option<String> {
    let chat_id = self.chat_id;
    let conversations = &self.conversations;
    let owner = conversations.owner();
    let privileged = {
      let mut state = conversations.state();
      let carried = state.active.get_mut(&chat_id)?.carried.as_mut()?;

      if !carried.open || carried.shut || carried.pushing {
        return None;
      }

      carried.pushing = true;
      carried.privileged
    };
    let messages = conversations.meanwhile(chat_id, privileged).await;
    let (instructions, ids, roads, events, holds, round, consumed, tools_allowed) = {
      let mut state = conversations.state();
      let conv = state.active.get_mut(&chat_id)?;
      let Some(takes_instructions) = conv.carried.as_ref().map(|carried| carried.takes_instructions) else {
        conv.give_back(messages, vec![], vec![], vec![], vec![]);
        return None;
      };
      let (instructions, ids, roads, events) = if takes_instructions {
        (
          std::mem::take(&mut conv.instructions),
          std::mem::take(&mut conv.instruction_ids),
          std::mem::take(&mut conv.instruction_roads),
          std::mem::take(&mut conv.instruction_events),
        )
      } else {
        Default::default()
      };

      if messages.is_empty() && instructions.is_empty() {
        conv.give_back(messages, instructions, ids, roads, events);

        if let Some(carried) = conv.carried.as_mut() {
          carried.pushing = false;
        }

        return None;
      }

      let carried = conv.carried.as_ref()?;

      (
        instructions,
        ids,
        roads,
        events,
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
      round: &round,
      messages: &messages,
      instructions: &instructions,
      events: &events,
      holds: &holds,
      consumed,
      tools_allowed,
      reader: Some(conversations.0.receiver.roads()),
      judged: true,
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

  // What the turn took joins the round: its messages shown to the round, waiting for the marks that settle them,
  // its instructions among the rows the round settles. What it refused goes back to the front of what waits, and
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

          carried.shown.extend(messages);
          carried.roads.extend(roads);
          carried.settling.extend(ids);
          carried.pushed.push(paragraph.text);
          carried.holds.extend(paragraph.reference);

          if !paragraph.instructions.is_empty() {
            carried.pushed_instructions.push(paragraph.instructions);
          }

          Some(counts)
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
    let Some((messages, pushed)) = taken else { return };

    println!(
      "chat {chat_id}: {messages} message(s) and {pushed} instruction(s) went into the working turn"
    );
  }

  // The working turn is over: it takes no more, words offered and never answered for go back, and what the
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

/// Every event of a room goes out as the agent's own: under the speaker its turns there are shown as, unless the
/// event already names its own.
pub fn told_as(mut event: Value, speaker: Option<Value>) -> Value {
  if event.get("speaker").is_none()
    && let Some(speaker) = speaker
  {
    event["speaker"] = speaker;
  }

  event
}

/// Who an agent's turns in a room are shown as (docs/api.md, 「Events and the live board」): `{id, name}`, its
/// registration name and the number its words are filed under there — the room's own identity for the default
/// answerer, the number the room's road or else the record gave any other agent — or a null number where the room
/// gives it none.
pub async fn speaker(receiver: &Receiver, agent: &str, chat_id: i64) -> Value {
  let roads = receiver.roads();
  let identity = roads.identity_in(chat_id).await;
  let designated = designated(receiver, agent).await;
  let tried = roads.room_road(chat_id).await.into_iter().chain([Road::Record.key().to_owned()]);
  let mut id = None;

  for road in tried {
    id = wake::agents::own_number(receiver, &designated, &road, identity).await.ok().flatten();

    if id.is_some() {
      break;
    }
  }

  json!({ "id": id, "name": agent })
}

// The agent as the wake judges it: the default answerer known as such whether or not a registration marks it, and
// where the agency does not answer, the deployment's own agent taken for it.
async fn designated(receiver: &Receiver, agent: &str) -> wake::agents::Designated {
  if let Ok(default) = wake::agents::default_designated(receiver).await
    && default.name == agent
  {
    return default;
  }

  match wake::agents::registered(receiver, agent).await {
    Ok(Some(designated)) => designated,
    _ => wake::agents::Designated {
      name: agent.to_owned(),
      call_names: Vec::new(),
      window_seconds: None,
      default_answerer: agent == receiver.deployment().agent,
      kind: None,
    },
  }
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
  let listed = receiver.agents().await.ok();

  listed
    .as_ref()
    .and_then(|listed| listed.agents.iter().find(|one| one.name == agent))
    .and_then(|found| found.row.get("sessionScope").and_then(Value::as_str))
    .is_some_and(|scope| scope == "agent")
}

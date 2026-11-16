//! The conversations of one conversational agent, every room's in one (docs/wake.md; docs/bmo-window.md): what a
//! room says joins its queue and is handed over together once the queue's delay is up; the room's conversation
//! wakes, or takes the words into the round it is running. A round reads what is waiting, asks the session, makes
//! the message from its draft through the draft process, and sends it — or waits for more, or passes the room by —
//! and the room's window keeps it listening without being called while it stands.

use std::collections::{HashMap, HashSet};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use tokio::task::JoinHandle;

use receiver_policy::chat_line::CHAT_LINE;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::drafting::DRAFTING;
use receiver_policy::fill_marks;
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};

use super::ask::Carry;
use super::context::{remember_held, sent_prompt};
use super::draft::{Drafter, Plan};
use super::health::{FailureOrders, RoomHealth};
use super::mid_turn::{Paragraph, Push};
use super::progress::{RunProgress, Told};
use super::prompt::{self, Round, Scope as RoundScope};
use super::reply::Sender;
use super::room::room_context;
use super::sentinels::{is_handoff, is_skip, is_wait};
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

/// How long the room queue gathers a room's words, how long a wait holds them, how long the default answerer's
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
  /// The round policy's, the window the default answerer's row keeps.
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
  /// Whether the failure counts against the rows and the words its round took: one the agency answered, or a step
  /// of the receiver's own; never one where the agency was unreachable, a rest, or a stop asked
  /// (docs/bmo-window.md, 「Quiet instruction」).
  pub fn counted(&self) -> bool {
    matches!(self, Failure::Failed(_) | Failure::Engine { .. })
  }

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
  /// The share of the window the session folds at, its supervisor's compaction threshold as the agency
  /// gives it.
  pub fold_ratio: Option<f64>,
  /// Whether the session's program carries its own fold, as the agency says.
  pub folds_itself: bool,
  pub replaced: Vec<SessionReason>,
  pub run: Option<String>,
  /// What the session held as the turn went out, which what a round read is counted from.
  pub before: super::draft::Before,
  /// The answers of the program's turns the turn spanned, where it spanned more than one; `text` was built from
  /// them (docs/drafting.md, 「What the receiver reads of an answer」).
  pub spanned: Vec<super::draft::Spanned>,
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
  /// The turn goes on the session as it stands, and no replacement is judged for it: a situation note is written
  /// by the session that answered in the room, whatever is about to become of that session.
  pub standing: bool,
  /// The turn is carried by a run of the agent's double, where its round was handed over (docs/drafting.md,
  /// 「A round handed to the double」).
  pub double: Option<Doubled>,
}

/// A turn of a round handed to the double: the round's own turn, which carries the room's block and every
/// document in full, or a later turn of the process, which carries its words alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Doubled {
  Round,
  Later,
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
  /// Whether the round was handed to the agent's double, whose run every later turn of the process goes to.
  pub double: bool,
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
  /// The round's own messages the agent marked as needing no answer.
  pub own_done: Vec<Inbound>,
  /// The round's own messages the agent marked to be answered in a round of their own.
  pub own_separate: Vec<Inbound>,
  /// The message the agent marked as the one its answer answers, by its number, where it marked one: zero for a
  /// mark naming none, which is the newest message the round answers.
  pub reply_to: Option<i64>,
}

/// What makes a round's message (docs/drafting.md): the process a round opens before its turn goes out, whose
/// first question the turn carries, and the rest of it once the turn answered. Filled by the module that owns the
/// process.
pub trait Drafting: Send + Sync {
  fn plan<'a>(&'a self, agent: &'a str, chat_id: i64) -> BoxFuture<'a, Plan>;

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
/// out as and its words, or why nothing went out. Filled by the module that owns the send.
pub trait Sending: Send + Sync {
  fn send<'a>(
    &'a self,
    receiver: &'a Receiver,
    agent: &'a str,
    road: &'a ReplyRoad,
    reply: &'a Reply,
  ) -> BoxFuture<'a, Result<Sent, Unsent>>;
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
    receiver: &'a Receiver,
    agent: &'a str,
    chat_id: i64,
    held: &'a Held,
  ) -> BoxFuture<'a, ()>;
}

/// The health of the rooms' sessions as the conversations read it: whether a room has fallen silent, the order a
/// broken run raises, and the minute the failure orders are looked over on. Filled by the module that owns the
/// health.
pub trait Health: Send + Sync {
  fn silenced<'a>(&'a self, receiver: &'a Receiver, chat_id: i64) -> BoxFuture<'a, bool>;
  fn failed<'a>(&'a self, receiver: &'a Receiver, chat_id: i64, failure: &'a Failure) -> BoxFuture<'a, ()>;
  fn orders(&self, receiver: &Receiver);
}

/// What a round's answer went out as, where it went out: the number of its message and its words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sent {
  pub message_id: i64,
  pub text: String,
}

/// Why an answer did not reach its room: the reason, and whether the send failed after it may have reached the
/// chat service, so the room may hold it though no number came back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unsent {
  pub reason: String,
  pub may_have_gone: bool,
}

impl Unsent {
  /// A send that proved nothing reached the room.
  pub fn nothing_went(reason: impl Into<String>) -> Self {
    Unsent {
      reason: reason.into(),
      may_have_gone: false,
    }
  }
}

/// What a turn of a console talk is asked with, and what it answers with (docs/unified-bmo.md, 「Console
/// conversation in the private chat」): the conversations take it as a round of the owner's private chat.
pub trait Talking: Send + Sync {
  fn talk<'a>(&'a self, request: &'a TalkRequest) -> BoxFuture<'a, Result<Option<String>, TalkFailure>>;
}

/// The touch of the quiet rooms' sessions, on the idle stretch the warmer names where it shortens it
/// (docs/bmo-window.md, 「The touch」): the conversations take it behind whatever round is running. The replacement
/// an agent's one session owes is made the same way, where the agent is quiet (docs/unified-bmo.md, 「Replacing the
/// session」).
pub trait Touching: Send + Sync {
  fn touch<'a>(
    &'a self,
    receiver: &'a Receiver,
    agent: &'a str,
    idle_override_ms: Option<i64>,
  ) -> BoxFuture<'a, Result<TouchAnswer, Fault>>;

  /// The replacement the agent's one session owes made now where the agent is quiet: true where one was made,
  /// false where none was owed or a round holds the agent's turn, which makes it once it ends.
  fn settle_owed<'a>(&'a self, receiver: &'a Receiver, agent: &'a str) -> BoxFuture<'a, bool>;

  /// The agent's standing session asked for its handoff ahead of a fresh session the agency opens, behind any
  /// round of the agent's: the file it wrote, none where it had no session to ask or wrote nothing.
  fn hand_off<'a>(
    &'a self,
    receiver: &'a Receiver,
    agent: &'a str,
    room: Option<i64>,
  ) -> BoxFuture<'a, Result<Option<super::handoff::Handoff>, Failure>>;
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
  // How often the words it opened on were given back by a round that failed.
  given_back: i64,
}

// The words a round that failed left unread, waiting for the room's next round (docs/messages.md, 「Read
// marker」): the words, what their road said of the room, and how often they were given back.
struct GivenBack {
  words: Vec<Inbound>,
  room: Room,
  times: i64,
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
  // What the push under way took out of the conversation, until the agency answers for it; a cut-off takes it
  // from here.
  taken: Option<Taken>,
  // The rows the round's own turn holds in its prompt, and the session and turn carrying the round now, as the
  // agency named the turn.
  own: Vec<i64>,
  turn: Option<(String, i64)>,
}

/// How far the pushes into a round's turn had come at a moment: what a turn begun then took is everything after.
#[derive(Clone, Copy, Debug, Default)]
pub struct PushMark {
  shown: usize,
  settling: usize,
  pushed: usize,
  pushed_instructions: usize,
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
      taken: None,
      own: vec![],
      turn: None,
    }
  }

  /// The rows the round's own turn holds in its prompt, written as carried by the turn the agency names for it.
  pub fn holding(mut self, own: Vec<i64>) -> Self {
    self.own = own;
    self
  }

  /// The room's messages the turn took, shown to the round and settled by its marks (docs/drafting.md, 「What
  /// came in meanwhile」).
  pub fn into_shown(self) -> Vec<Inbound> {
    self.shown
  }
}

// The words a push took out of the conversation, held with the working turn until the agency says what became of
// them.
struct Taken {
  messages: Vec<Inbound>,
  instructions: Vec<String>,
  ids: Vec<i64>,
  roads: Vec<String>,
  events: Vec<TurnEvent>,
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

  // What the push under way took, back at the front of what waits; nothing where a cut-off let it go meanwhile.
  fn give_back_taken(&mut self) {
    if let Some(taken) = self.carried.as_mut().and_then(|carried| carried.taken.take()) {
      self.give_back(taken.messages, taken.instructions, taken.ids, taken.roads, taken.events);
    }
  }

  // The words a failed round left for this conversation, at the front of what waits, under the room their road
  // spoke of.
  fn take_back(&mut self, back: GivenBack) {
    let words: Vec<Inbound> =
      back.words.into_iter().filter(|message| self.seen.insert(message.message_id)).collect();

    self.buffer.splice(0..0, words);
    self.room = back.room;
    self.given_back = back.times;
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
      given_back: 0,
    }
  }
}

// A round under way, counted until it is dropped.
struct UnderWay(Conversations);

impl Drop for UnderWay {
  fn drop(&mut self) {
    let mut state = self.0.state();

    state.rounds = state.rounds.saturating_sub(1);
  }
}

#[derive(Default)]
struct State {
  active: HashMap<i64, Conv>,
  // The rounds under way, each from the moment it takes the agent's turn to the moment it gives it back, the
  // replacement it may make after its last turn included.
  rounds: usize,
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
  // The turn each room's round stands as in the live picture while it runs, which the words it takes move on.
  pictured: HashMap<i64, u64>,
  // The agent's own runs going now, by name with the room each runs for, which a reset or a room's removal
  // stands its cancellation over.
  going: HashMap<String, i64>,
  // The messages of each room that reached the session on its open window alone, which the agent judges first.
  window_only: HashMap<i64, HashSet<i64>>,
  // The words a failed round left unread in each room, which the room's next conversation opens on.
  given_back: HashMap<i64, GivenBack>,
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

/// Every room's queue of an agent's conversations kept gathering until this is dropped.
pub struct QueueHold {
  _held: tokio::sync::OwnedRwLockWriteGuard<()>,
}

/// What a reset cut off of an agent's conversations: the rows of the quiet instructions let go unread, and the
/// runs of its own going then.
#[derive(Debug, Default)]
pub struct CutOff {
  pub instructions: Vec<i64>,
  pub going: Vec<String>,
}

// What a round of the pump keeps of itself: whether its turn was written down, the rows it settles, whether it
// held its words over, and the words it took with the read mark as it stood, which a failure gives back.
#[derive(Default)]
pub struct Record {
  settling: Vec<i64>,
  held_over: bool,
  // The words the round took, and where the read mark stood before it took them.
  took: Vec<Inbound>,
  read_before: Option<i64>,
}

struct Inner {
  /// The agent whose conversations these are.
  agent: String,
  receiver: Receiver,
  hooks: Arc<dyn Hooks>,
  timings: Timings,
  state: Mutex<State>,
  turn: tokio::sync::Mutex<()>,
  // Read by every room's queue as its delay is up, before it hands over what it gathered: whoever holds it
  // written keeps every queue gathering until it lets go.
  queue_hold: Arc<tokio::sync::RwLock<()>>,
  // The rooms a voice briefing turn of the agent's is being carried in.
  briefings: Arc<super::briefing_turn::BriefingTurns>,
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
      queue_hold: Arc::default(),
      briefings: Arc::default(),
    }))
  }

  /// Holds every room's queue gathering past its delay until the hold is dropped, so what is said meanwhile joins
  /// the one batch the queue hands over once it is let go (docs/wake.md, 「The room's queue and the round」).
  pub async fn hold_queues(&self) -> QueueHold {
    QueueHold {
      _held: self.0.queue_hold.clone().write_owned().await,
    }
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
    self.0.receiver.news().publish(&self.under_speaker(event));
  }

  // An event of a room's working turn that also stands, in `slot`, for a screen that connects later.
  fn publish_standing(&self, slot: u64, event: Value) {
    self.0.receiver.news().publish_standing(slot, &self.under_speaker(event));
  }

  // An event as the room's speaker tells it.
  fn under_speaker(&self, event: Value) -> Value {
    let speaker = event
      .get("chatId")
      .and_then(Value::as_i64)
      .and_then(|chat_id| self.state().speakers.get(&chat_id).cloned());

    told_as(event, speaker)
  }

  // The room shown typing on its own road, or no longer, and the stream told the answerer's mark at the same moments
  // (docs/receiver.md, 「Typing and presence」).
  async fn shown_typing(&self, chat_id: i64, on: bool) {
    let speaker = self.state().speakers.get(&chat_id).cloned();

    crate::presence::taken::answerer(&self.0.receiver, chat_id, self.agent(), speaker, on);
    self.0.hooks.typing(chat_id, on).await;
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
  /// queue gathers, those a conversation holds or waits on and those a push took for a working turn the agency
  /// has not answered for, and its quiet instructions, among them a push's, whose rows it answers with the runs of
  /// its own going now (docs/wake.md, 「An agent's reset」).
  pub fn cut_off(&self) -> CutOff {
    let mut state = self.state();
    let mut rooms: Vec<i64> = state.queue.drain().map(|(chat_id, _)| chat_id).collect();
    let mut instructions = vec![];

    state.given_back.clear();

    for (chat_id, conv) in &mut state.active {
      rooms.push(*chat_id);
      conv.buffer.clear();
      conv.held_words.clear();
      conv.held_since.clear();
      conv.instructions.clear();
      conv.instruction_roads.clear();
      conv.instruction_events.clear();
      instructions.append(&mut conv.instruction_ids);

      if let Some(taken) = conv.carried.as_mut().and_then(|carried| carried.taken.take()) {
        instructions.extend(taken.ids);
      }
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
      state.given_back.remove(&chat_id);
      state.called_windows.remove(&chat_id);
      state.woken.remove(&chat_id);
      state.folding.remove(&chat_id);
      state.speakers.remove(&chat_id);
      state.active.remove(&chat_id)
    };

    if let Some(timer) = conv.and_then(|mut conv| conv.timer.take()) {
      timer.abort();
    }

    self.shown_typing(chat_id, false).await;
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

    state.rounds == 0
      && state.queue.is_empty()
      && state.delivering == 0
      && state.folding.is_empty()
      && state.active.values().all(|conv| {
        !conv.in_flight && !conv.waiting && conv.buffer.is_empty() && conv.instructions.is_empty()
      })
  }

  /// Whether no words of any room wait for the agent: nothing gathering, on its way, or left unread for a round
  /// still to come.
  fn quiet(&self) -> bool {
    let state = self.state();

    state.queue.is_empty()
      && state.delivering == 0
      && state.active.values().all(|conv| conv.buffer.is_empty() && conv.instructions.is_empty())
  }

  /// The replacement the agent's one session owes made now where the agent is quiet, behind its turn: true where
  /// one was made. A round holding the turn makes it once it ends.
  pub async fn settle_owed(&self) -> bool {
    let Ok(turn) = self.0.turn.try_lock() else {
      return false;
    };
    let settled = self.settle_owed_behind_turn().await;

    drop(turn);
    settled
  }

  // The owed replacement made by whoever holds the agent's turn, where no words wait; words that arrive while
  // it is made wait for the fresh session, and words failed rounds gave back go to it.
  async fn settle_owed_behind_turn(&self) -> bool {
    if !self.quiet() {
      return false;
    }

    let replaced = super::ask::replace_owed(&self.0.receiver, self.agent()).await;

    if replaced {
      self.hand_given_back(None).await;
    }

    replaced
  }

  /// The agent's standing session asked for its handoff ahead of a fresh session the agency opens
  /// (docs/unified-bmo.md, 「Replacing the session」), behind the agent's turn as an owed replacement is: a round
  /// under way is waited for to its end, not only the turn it is in, and words that arrive meanwhile wait for the
  /// fresh session. Once it is asked, the words failed rounds gave back go to the fresh session's first turn in
  /// their rooms.
  pub async fn hand_off(&self, room: Option<i64>) -> Result<Option<super::handoff::Handoff>, Failure> {
    let turn = self.0.turn.lock().await;
    let handed = Carry::new(self.0.receiver.clone()).hand_off(self.agent(), room).await;

    if handed.is_ok() {
      self.hand_given_back(None).await;
    }

    drop(turn);
    handed
  }

  // The words failed rounds gave back while the agent's old session stood, each room's put at the front of what
  // waits for its next round and that round opened on them, so the fresh session's first turn in the room holds
  // them as its new messages (docs/messages.md, 「Read marker」). The failures the old session counted on them are
  // cleared. `except` is a room whose words a round failed on the fresh session gave back, left as they stand.
  // Called by whoever holds the agent's turn: the rounds it opens wait for it.
  async fn hand_given_back(&self, except: Option<i64>) {
    let backs: Vec<(i64, GivenBack)> = {
      let mut state = self.state();
      let rooms: Vec<i64> =
        state.given_back.keys().copied().filter(|chat_id| Some(*chat_id) != except).collect();

      rooms
        .into_iter()
        .filter_map(|chat_id| state.given_back.remove(&chat_id).map(|back| (chat_id, back)))
        .collect()
    };

    for (chat_id, back) in backs {
      let back = GivenBack { times: 0, ..back };
      let held = self.held_session(chat_id).await;
      let window = self.window_ms().await;
      let owner = self.owner();
      let pumps = {
        let mut state = self.state();

        match state.active.entry(chat_id) {
          std::collections::hash_map::Entry::Occupied(mut standing) => {
            let conv = standing.get_mut();

            conv.take_back(back);
            !conv.in_flight
          }
          std::collections::hash_map::Entry::Vacant(vacant) => {
            let mut conv = Conv::new(chat_id, back.room, held);

            conv.take_back(back);
            conv.reply_target = conv
              .buffer
              .iter()
              .rev()
              .find(|message| !message.via_reader.unwrap_or(false))
              .map(|message| message.message_id);
            conv.held.all_from_owner = conv.buffer.iter().all(|message| message.sender_id == Some(owner));
            self.set_deadline(&mut conv, chat_id, window, false);
            vacant.insert(conv);
            true
          }
        }
      };

      println!(
        "chat {chat_id}: the words a failed round gave back go to {}'s fresh session",
        self.agent()
      );

      if pumps {
        self.spawn_pump(chat_id, false);
      }
    }
  }

  // The fixed line telling a room its words wait for the fresh session, said as the agent.
  async fn say_tidying(&self, chat_id: i64) {
    let receiver = &self.0.receiver;
    let Some(line) = receiver.speech().rendered(&receiver_codes::FixedLine::MemoryTidying).await else {
      return;
    };

    if let Err(why) = receiver.speech().say(chat_id, &line).await {
      eprintln!("chat {chat_id}: the memory-tidying line was not said: {why}");
    }
  }

  // A round counted among those under way for as long as the guard stands.
  fn round_under_way(&self) -> UnderWay {
    self.state().rounds += 1;

    UnderWay(self.clone())
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
          drop(this.0.queue_hold.read().await);

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

        if let Some(back) = state.given_back.remove(&chat_id) {
          conv.take_back(back);
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
      sealed: false,
    };
    let held = self.held_session(chat_id).await;
    let window = self.window_ms().await;
    let pumps = {
      let mut state = self.state();

      // The words a failed round left unread open the conversation with the instruction.
      let back = if state.active.contains_key(&chat_id) {
        None
      } else {
        state.given_back.remove(&chat_id)
      };

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

          if let Some(back) = back {
            conv.take_back(back);
          }

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

  /// Whether nothing of a room's stands to be answered now: no round in flight there and no words in its buffer,
  /// or no conversation in it at all.
  pub fn settled(&self, chat_id: i64) -> bool {
    self
      .state()
      .active
      .get(&chat_id)
      .is_none_or(|conv| !conv.in_flight && conv.buffer.is_empty())
  }

  /// Whether a round of the agent's stands in any room now.
  pub fn busy(&self) -> bool {
    self.state().active.values().any(|conv| conv.in_flight)
  }

  /// Whether a round of the agent's is working in the room now, from its first turn to the send of its answer.
  pub fn in_round(&self, chat_id: i64) -> bool {
    self.state().active.get(&chat_id).is_some_and(|conv| conv.in_flight)
  }

  /// Whether a voice briefing turn of the agent's, a notice turn's summary or a line's full reading, is being
  /// carried in the room now.
  pub fn in_briefing(&self, chat_id: i64) -> bool {
    self.0.briefings.in_flight(chat_id)
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
    // Words that arrive while the agent's session is replaced wait for the fresh session; the room is told so once
    // (docs/unified-bmo.md, 「Replacing the session」).
    if self.0.receiver.replacing_untold(self.agent(), chat_id) {
      self.say_tidying(chat_id).await;
    }

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
    let handovers = self.0.receiver.handovers(self.agent());
    let under_way = self.round_under_way();
    let (messages, instructions, read) = {
      let state = self.state();
      let Some(conv) = state.active.get(&chat_id) else {
        drop(state);
        drop(under_way);
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

    self.state().pictured.insert(chat_id, turn);

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
    // A round that failed for anything but a stop asked of it gives back every row and word it took, the failure
    // counted only where the agency answered it; one where the agency was unreachable holds the agent's rows until
    // it answers again (docs/bmo-window.md, 「Quiet instruction」).
    let stopped = matches!(&ended, Err(Failure::Stopped));
    let failed = ended.is_err() && !stopped;
    let counted = matches!(&ended, Err(failure) if failure.counted());

    if let Err(failure) = ended {
      if failed {
        self.leave_unread(chat_id, &record, counted);
      }

      if failed
        && !counted
        && let Some(pass) = self.0.receiver.pass()
      {
        pass.hold(self.agent());
      }

      self.fail_round(chat_id, failure).await;
    } else if let Some(conv) = self.state().active.get_mut(&chat_id) {
      // A round that ended has dealt with the words a failed one gave back.
      conv.given_back = 0;
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

    // The rows the round took end with it: settled where a turn carried them and the round ended with its message
    // or its skip, let go where a stop was asked of it, and back to pending otherwise. A write that failed leaves
    // them handed, for the reconcile or the next start to give back. Its turn's mark goes with it.
    let db = self.0.receiver.db();
    let ending = if stopped {
      super::instructions::let_go(db, &settling, super::instructions::why::STOPPED, Some(&round))
        .await
        .map(|()| vec![])
    } else if failed {
      super::instructions::give_back(db, &round, &settling, counted).await
    } else {
      super::instructions::settle(db, &round, &settling).await
    };

    match ending {
      Ok(given) if !given.is_empty() => {
        if let Some(pass) = self.0.receiver.pass() {
          pass.kick();
        }
      }
      Ok(_) => {}
      Err(why) => eprintln!("chat {chat_id}: the rows of round {round} were not settled: {}", why.detail),
    }

    let _ = super::turn_mark::forget(self.0.receiver.db(), &round).await;

    self.state().pictured.remove(&chat_id);
    self.0.receiver.news().end_turn(turn);
    self.shown_typing(chat_id, false).await;

    // A session the round replaced ahead of its turn leaves the words failed rounds gave back in other rooms to
    // the fresh session.
    if self.0.receiver.handovers(self.agent()) != handovers {
      self.hand_given_back(Some(chat_id)).await;
    }

    // A replacement the round found owed is made now that it has ended, where no words wait (docs/unified-bmo.md,
    // 「Replacing the session」).
    self.settle_owed_behind_turn().await;

    drop(under_way);
    drop(turn_lock);
  }

  // The words of a round that failed for anything but a stop left unread (docs/messages.md, 「Read marker」): the
  // read mark back where it stood before the round, and the words it took, with those that came in while it
  // worked, kept for the room's next conversation. A failure the agency answered is counted on them, and words
  // whose count reaches the policy's `failure_limit` go with the round as read; one where the agency was
  // unreachable counts nothing.
  fn leave_unread(&self, chat_id: i64, record: &Record, counted: bool) {
    let mut state = self.state();
    let Some(conv) = state.active.get_mut(&chat_id) else { return };
    let times = conv.given_back + i64::from(counted);

    if times >= CHAT_ROUND.instructions.failure_limit {
      eprintln!("chat {chat_id}: the words of a round failed {times} times go with it as read");

      return;
    }

    let mut kept = HashSet::new();
    let words: Vec<Inbound> = record
      .took
      .iter()
      .cloned()
      .chain(conv.buffer.drain(..))
      .filter(|message| kept.insert(message.message_id))
      .collect();

    if words.is_empty() {
      return;
    }

    conv.held.last_consumed_id = record.read_before;

    let room = conv.room;

    state.given_back.insert(chat_id, GivenBack { words, room, times });
  }

  // A round that failed, wherever in it the failure came from: it ends the conversation the way a failed run
  // does, a stop with nothing of the failure road, a limit or an outage with its rest, any other with its order.
  async fn fail_round(&self, chat_id: i64, failure: Failure) {
    self.shown_typing(chat_id, false).await;

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

  // The round's answer pinned to the message the agent marked (docs/drafting.md, 「The answer is pinned only to
  // the message the agent marks」): the one its number names, the newest it answers where the mark names none, and
  // nothing where it set no mark, where the room's record does not hold the message or no road can aim at it.
  async fn pin(&self, chat_id: i64, answered: &[Inbound], named: Option<i64>) {
    let message = match named {
      Some(0) => answered.last().cloned(),
      Some(id) => match answered.iter().find(|message| message.message_id == id) {
        Some(message) => Some(message.clone()),
        None => rows::message(self.0.receiver.db(), chat_id, id, "").await.ok().flatten(),
      },
      None => None,
    };
    let Some(message) = message else {
      self.reply_target(chat_id, None);

      return;
    };
    let pins = self.0.receiver.roads().pinnable(&message.via, message.mode.as_deref()).await;

    self
      .reply_target(chat_id, (!message.via_reader.unwrap_or(false) && pins).then_some(message.message_id));
  }

  // The message the next answer is pinned to, where one is: none where the agent marked none or no road can aim
  // at it.
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
      record.took = batch.clone();
      record.read_before = conv.held.last_consumed_id;

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

    // A round with words in a room other than the one the agent last answered in writes that room's situation
    // note first (docs/drafting.md, 「The situation note」); the note of this room as it stands is kept on the
    // messages the round judges, for the console's record.
    if !batch.is_empty() && receiver.situation_updates() {
      self.leave_notes(chat_id, &held).await?;
      self.note_judged(chat_id, &batch).await;
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
    let plan = self.0.drafting.plan(self.agent(), chat_id).await;
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
      )
      .holding(ids.clone()),
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
      standing: false,
      double: None,
    };

    // The draft turn is told as it starts, under the name its row of the round log goes by.
    if let Some(names) = plan.names() {
      super::draft::log::tell_start(self, round, chat_id, self.agent(), 1, 1, &names);
    }

    let asked = self.0.carrying.carry(&asking, &held, &follower).await;
    // A draft turn that answers the hand-off sentinel hands its round over: the same turn is carried again, whole,
    // by a run of the agent's double, and every later turn of the process goes to that run (docs/drafting.md,
    // 「A round handed to the double」). The agent's own session keeps what its turn left it.
    let mut doubled = false;
    let asked = match asked {
      Ok(reply) if is_handoff(&crate::form::strip_prompt_scaffolding(&reply.text)) => {
        doubled = true;
        held.context_tokens = reply.context_tokens.or(held.context_tokens);
        held.session = reply.session.clone().or(held.session.clone());
        self.keep_session(chat_id, &held);
        println!("chat {chat_id}: {} handed the round to its double", self.agent());

        let handed = Asking {
          double: Some(Doubled::Round),
          began: now_ms(),
          ..asking.clone()
        };
        let mark = follower.mark();
        let own_turn = follower.carrier();

        match self.0.carrying.carry(&handed, &held, &follower).await {
          // A double that did not take the round hands it back: what was pushed into its turn goes back to wait,
          // and the agent's own session, which holds the round already, is asked once more to answer it itself, and
          // the process goes on there.
          Err(Failure::Failed(why)) | Err(Failure::Transport(why)) => {
            doubled = false;
            eprintln!("chat {chat_id}: the double did not take the round: {why}");
            follower.hand_back(mark, own_turn).await;

            let own = Asking {
              prompt: fill_marks(&CHAT_LINE.room.handed_back, &[("why", why.as_str())]),
              began: now_ms(),
              ..asking.clone()
            };

            match self.0.carrying.carry(&own, &held, &follower).await {
              Ok(reply) if is_handoff(&crate::form::strip_prompt_scaffolding(&reply.text)) => {
                Err(Failure::Failed(
                  "the agent handed the round over again after its double gave it back".to_owned(),
                ))
              }
              other => other,
            }
          }
          other => other,
        }
      }
      other => other,
    };
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
          double: doubled,
        },
      )
      .await?;

    reply = made.reply;

    // Each message shown to the round carries what its mark made of it, naming where the round's own verdict stands.
    self.judge_shown(chat_id, held.last_consumed_id, &made.took, &made.done).await;

    // A message marked to be answered on its own goes back to wait, and opens the round after this one.
    self.give_back_words(chat_id, made.separate);

    // The round's own words, where several people said them, each carry what the round made of them
    // (docs/drafting.md, 「Words of several people」): answered by its message, or marked as needing no answer. One
    // marked to be answered on its own leaves the round and goes back to wait, as a message shown to it does,
    // unless every one of them was: a round sets none of its words apart from itself.
    if prompt::speakers(&batch) > 1 {
      let separate = if made.own_separate.len() < batch.len() { made.own_separate } else { vec![] };
      let set_apart = |message: &Inbound| {
        made
          .own_done
          .iter()
          .chain(separate.iter())
          .any(|marked| marked.message_id == message.message_id)
      };
      let answered: Vec<Inbound> = batch.iter().filter(|message| !set_apart(message)).cloned().collect();

      self.judge_shown(chat_id, held.last_consumed_id, &answered, &made.own_done).await;

      if !separate.is_empty() {
        batch.retain(|message| !separate.iter().any(|marked| marked.message_id == message.message_id));
        self.give_back_words(chat_id, separate);
      }
    }

    // The words shown to the process are the round's own: the read mark moves past those marked done and to those
    // the message answers, and they join the messages a bare reply mark names the newest of.
    if !made.took.is_empty() || !made.done.is_empty() {
      let read: Vec<Inbound> =
        made.done.into_iter().chain(batch.iter().cloned()).chain(made.took.clone()).collect();

      batch.extend(made.took);
      self.consume(chat_id, &read).await;
      held.last_consumed_id = self.state().active.get(&chat_id).and_then(|conv| conv.held.last_consumed_id);
    }

    // The answer is pinned to the message the agent marked, whatever the owner is owed; a mark the supervisor's
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

    let waiting = {
      let state = self.state();

      state.active.get(&chat_id).map(|conv| (conv.reply_target, conv.asked_at))
    };
    let (reply_to, asked_at) = waiting.unwrap_or((None, None));
    // The round is answered only once its message went out (docs/wake.md, 「The room's queue and the round」): a
    // message no try put into the room fails the round before its read mark is written, and every word it read
    // waits for the next.
    let sent = if self.silenced(chat_id).await {
      None
    } else {
      let road = ReplyRoad {
        chat_id,
        reply_to,
        via: via.to_owned(),
        round: round.to_owned(),
        asked_at,
        began,
      };

      // The answer is let through: the room is shown typing until the round ends (docs/messages.md, 「Typing」).
      self.shown_typing(chat_id, true).await;

      match self.send_tried(&receiver, &road, &reply).await {
        Ok(sent) => Some(sent),
        Err(failure) => {
          self.give_back_words(chat_id, batch);

          return Err(failure);
        }
      }
    };

    self.persist(&held).await;

    if let Some(conv) = self.state().active.get_mut(&chat_id) {
      conv.answered = true;
    }

    self.restart_window(chat_id).await;

    if let Some(sent) = &sent {
      if let Some(conv) = self.state().active.get_mut(&chat_id) {
        conv.asked_at = None;
      }

      // The room is owed its situation note from this answer on, and is the room the agent last answered in.
      if receiver.situation_updates() && unified_session(&receiver, self.agent()).await {
        super::situation::owe(receiver.db(), self.agent(), chat_id).await;
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

    self.maybe_fold(chat_id, held.session.as_deref(), ratio, fold_at, folds_itself).await;
    self.pump_pending(chat_id);

    Ok(())
  }

  // The round's message put into its room, tried the round policy's number of times, its gap apart: the message
  // as it went out, or the round's failure, naming the last try's reason, where no try put it in. A send that
  // failed after it may have reached the chat service is not tried again, so the room never reads it twice.
  async fn send_tried(
    &self,
    receiver: &Receiver,
    road: &ReplyRoad,
    reply: &Reply,
  ) -> Result<Sent, Failure> {
    let tries = CHAT_ROUND.send.tries.max(1);
    let mut why = String::new();

    for tried in 1..=tries {
      match self.0.sending.send(receiver, self.agent(), road, reply).await {
        Ok(sent) => return Ok(sent),
        Err(unsent) if unsent.may_have_gone => {
          return Err(Failure::Failed(format!(
            "the round's message may have reached the room though its send failed, so it was not sent again: {}",
            unsent.reason
          )));
        }
        Err(unsent) => why = unsent.reason,
      }

      if tried < tries {
        tokio::time::sleep(receiver.send_again_pace()).await;
      }
    }

    Err(Failure::Failed(format!("the round's message was not sent after {tries} tries: {why}")))
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

  // The situation notes the agent owes written before a round goes on in `entering` (docs/drafting.md, 「The
  // situation note」): each room it answered in since that room's note was last updated, other than the room the
  // round is in, updated by one turn of the agent's one session, in that room. A turn whose run failed leaves
  // its room owed and the round goes on; a stop ends the round.
  async fn leave_notes(&self, entering: i64, held: &Held) -> Result<(), Failure> {
    self.owed_notes(Some(entering), held.session.as_deref()).await.map(drop)
  }

  /// The situation notes the agent owes written, each by one turn of its one session in the room the note
  /// is of: every room it answered in since that room's note was last updated, but `staying`, the room a round is
  /// about to go on in. A turn whose run failed leaves its room owed and the rest go on; a stop ends them. How
  /// many turns went in.
  pub async fn owed_notes(&self, staying: Option<i64>, session: Option<&str>) -> Result<usize, Failure> {
    let receiver = self.receiver();
    let agent = self.agent();
    let left: Vec<i64> = super::situation::owed(receiver.db(), agent)
      .await
      .into_iter()
      .filter(|room| Some(*room) != staying)
      .collect();

    if left.is_empty() || !receiver.situation_updates() || !unified_session(&receiver, agent).await {
      return Ok(0);
    }

    let mut written = 0;

    for room in left {
      match update_note(&receiver, self, self.0.carrying.as_ref(), agent, room, session).await {
        Err(Failure::Stopped) => return Err(Failure::Stopped),
        Err(why) => eprintln!("chat {room}: the situation note was not updated: {why:?}"),
        Ok(went) => written += usize::from(went),
      }
    }

    Ok(written)
  }

  // The room's note as it stands, kept on the messages a round judges, so the console's judgement record says
  // what the room's situation was as the round found it.
  async fn note_judged(&self, chat_id: i64, batch: &[Inbound]) {
    let receiver = self.receiver();
    let note = super::situation::read(receiver.db(), chat_id).await.unwrap_or(None);
    let judged: Vec<i64> = batch.iter().map(|message| message.message_id).collect();

    if let Err(why) =
      super::situation::attach(receiver.db(), chat_id, &judged, note.as_deref(), false).await
    {
      eprintln!("chat {chat_id}: the situation note was not kept on the judged messages: {}", why.detail);
    }
  }

  // The agent's own judgement of whether to answer words that did not name it (docs/wake.md, 「The room's queue and
  // the round」): one turn of the same session carrying the room's situation note where the session has not
  // read it, the room's recent words and the words judged, in the room shape; no reference and no gate. Its row stands in the
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
    // A session reads a room's note once: its place is filled where the session holds none of this room.
    let mut parts = vec![
      said.marker.clone(),
      String::new(),
      super::situation::first_reading(chat_id),
      String::new(),
    ];

    parts.push(said.recent.clone());
    parts.extend(recent_lines);
    parts.push(String::new());
    parts.push(said.judged.clone());
    parts.extend(judged_lines);

    let started = now_ms();
    let mut asking = Asking {
      chat_id,
      session: held.session.clone().unwrap_or_default(),
      prompt: parts.join("\n"),
      via: (!via.is_empty()).then(|| via.to_owned()),
      privileged: held.all_from_owner,
      answers: batch.iter().map(|message| message.message_id).collect(),
      began: started,
      context: vec![],
      thread_mark: None,
      standing: false,
      double: None,
    };
    let mut log = super::draft::log::Log::new(self, round, chat_id, self.agent()).numbered_from(0);
    // A judgement that skipped with no reason is no verdict: the marker and the line naming the form are the whole
    // of the turn asked over, the session holding the words already (docs/drafting.md, 「What the receiver reads
    // of an answer」). The asking over is asked for once.
    let mut over_skip = false;
    let (reply, passed) = loop {
      let follower = Follower::new(self, chat_id);
      let began = now_ms();
      let reply = self.0.carrying.carry(&asking, held, &follower).await?;

      follower.finish();
      held.context_tokens = reply.context_tokens.or(held.context_tokens);
      held.session = reply.session.clone().or(held.session.clone());
      held.replaced.extend(reply.replaced.clone());
      self.keep_session(chat_id, held);

      let (ended, passed, reason) =
        match super::sentinels::skip_alone(&crate::form::strip_prompt_scaffolding(&reply.text)) {
          Some(super::sentinels::Skip::Reasoned(reason)) => {
            (super::draft::log::Answered::Skip, true, Some(reason))
          }
          Some(super::sentinels::Skip::Bare) if over_skip => {
            (super::draft::log::Answered::Skip, true, Some(super::draft::log::NO_REASON.to_owned()))
          }
          Some(super::sentinels::Skip::Bare) => (super::draft::log::Answered::NoVerdict, false, None),
          None => (super::draft::log::Answered::Answered, false, None),
        };

      log
        .turn(
          super::draft::log::Kind::Supervisor,
          &said.name,
          Some(&reply.text),
          ended,
          reason.as_deref(),
          super::draft::log::Timed::of(now_ms() - began, &reply.spanned),
        )
        .await;

      if ended != super::draft::log::Answered::NoVerdict {
        break (reply, passed);
      }

      over_skip = true;
      asking.prompt = format!("{}\n{}", said.marker, DRAFTING.skip_again);
    };

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

    Ok(passed)
  }

  // A notice turn's summary, written by the same session in one turn of the summary's quiet instruction once
  // the line has gone out, and handed to the voice engine with the line it speaks; a session that gives none has
  // the line's opening spoken.
  async fn summarise(&self, chat_id: i64, held: &mut Held, via: &str, sent: &Sent) {
    let receiver = self.receiver();
    let asking = Asking {
      chat_id,
      session: held.session.clone().unwrap_or_default(),
      prompt: format!(
        "{}\n{}",
        CHAT_ROUND.voice.summary_marker,
        fill_marks(&CHAT_ROUND.voice.summary, &[("line", &sent.text)])
      ),
      via: (!via.is_empty()).then(|| via.to_owned()),
      privileged: held.all_from_owner,
      answers: vec![],
      began: now_ms(),
      context: vec![],
      thread_mark: None,
      standing: false,
      double: None,
    };
    let follower = Follower::new(self, chat_id);
    let briefing = self.0.briefings.carrying(chat_id);
    let carried = self.0.carrying.carry(&asking, held, &follower).await;

    drop(briefing);

    let answer = match carried {
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
  /// 「The full reading on a line's chip」): one turn of the reading's quiet instruction, held behind any
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
      prompt: format!(
        "{}\n{}",
        CHAT_ROUND.voice.reading_marker,
        fill_marks(&CHAT_ROUND.voice.reading, &[("line", line)])
      ),
      via: None,
      privileged: false,
      answers: vec![],
      began: now_ms(),
      context: vec![],
      thread_mark: None,
      standing: false,
      double: None,
    };
    let follower = Follower::new(self, chat_id);
    let briefing = self.0.briefings.carrying(chat_id);
    let carried = self.0.carrying.carry(&asking, &held, &follower).await;

    drop(briefing);
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
      sealed: room.sealed,
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

  /// The room's turn in the live picture moved on to messages its round took while it works, pushed into a turn or
  /// shown under a question (docs/receiver.md, 「The event stream」).
  pub fn took_meanwhile(&self, chat_id: i64, messages: &[Inbound]) {
    let Some(turn) = self.state().pictured.get(&chat_id).copied() else {
      return;
    };
    let ids: Vec<i64> = messages.iter().map(|message| message.message_id).collect();

    self.0.receiver.news().move_turn(turn, &ids);
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

      return Ok(());
    }

    if again {
      self.spawn_pump(chat_id, force);
      return Ok(());
    }

    self.publish(waited(chat_id, message, Some("WAIT")));

    Ok(())
  }

  // The messages shown to the round written down under themselves (docs/drafting.md, 「What came in meanwhile」):
  // those that became the round's own as read with the message its verdict stands under, those marked done as
  // skipped beside it.
  async fn judge_shown(&self, chat_id: i64, under: Option<i64>, took: &[Inbound], done: &[Inbound]) {
    let named = under.map(|id| format!(": #{id}")).unwrap_or_default();

    for (messages, word) in [(took, "READ"), (done, "SKIP")] {
      for message in messages {
        let verdict = format!("{word}{named}");

        super::turnover::judge(
          &self.0.receiver,
          chat_id,
          Some(message.message_id),
          "read-with",
          &verdict,
          None,
          None,
        )
        .await;
      }
    }
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

    let text = format!(
      "{}\n{}",
      CHAT_ROUND.relay.marker,
      fill_marks(
        &CHAT_ROUND.relay.answer,
        &[("chat", &chat_id.to_string()), ("opening", &opening), ("answer", answer)],
      )
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
        Some(held) => this.0.noting.fold(&receiver, this.agent(), chat_id, &held).await,
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

    // A replacement owed is made ahead of the touch: the agent is quiet, and the fresh session is what the touch
    // keeps warm (docs/unified-bmo.md, 「Replacing the session」).
    self.settle_owed_behind_turn().await;

    let touched = super::touch::touch(&self.0.receiver, self, self.agent(), idle_override_ms).await;

    drop(lock);
    touched
  }

  /// What a line a person typed into the default answerer's program at `at_ms` goes with (docs/bmo-window.md,
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
      },
    )
    .await?;
    let room = room_context(&receiver, self.agent(), room_id, None).await?;
    let written = composed.prompt.clone();
    let (sent, _, notes) =
      sent_prompt(&receiver, Some(session_id), Some(&room), true, self.agent(), &written).await?;

    // A note the line's context carried is one the session now holds, and no round hands it again.
    for note in &notes {
      super::situation::keep(receiver.db(), session_id, note).await?;
    }

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

// The slots followers' pictures stand in, one a follower.
static FOLLOWERS: AtomicU64 = AtomicU64::new(1);

// What follows a round's turn on its conversation's behalf: the turn's picture published to the room's screens
// only where it changed, standing for a screen that connects later until the turn ends, and what the room says
// meanwhile put into the turn at its next look.
pub struct Follower {
  conversations: Conversations,
  chat_id: i64,
  // The slot the turn's picture stands in.
  slot: u64,
  told: Mutex<Told>,
  offered: Mutex<Option<Paragraph>>,
}

impl Follower {
  pub fn new(conversations: &Conversations, chat_id: i64) -> Self {
    Self {
      conversations: conversations.clone(),
      chat_id,
      slot: FOLLOWERS.fetch_add(1, Ordering::Relaxed),
      told: Mutex::new(Told::default()),
      offered: Mutex::new(None),
    }
  }

  fn offered(&self) -> MutexGuard<'_, Option<Paragraph>> {
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
    // What the push takes stands with the turn until the agency answers, where a cut-off reaches it; the paragraph
    // is built from a copy.
    let (instructions, ids, events, holds, round, consumed, tools_allowed) = {
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

      let consumed = conv.held.last_consumed_id;
      let carried = conv.carried.as_mut()?;
      let built = (
        instructions.clone(),
        ids.clone(),
        events.clone(),
        carried.holds.clone(),
        carried.round.clone(),
        consumed,
        carried.tools_allowed,
      );

      carried.taken = Some(Taken {
        messages: messages.clone(),
        instructions,
        ids,
        roads,
        events,
      });
      built
    };
    let built = super::mid_turn::paragraph(&Push {
      receiver: &conversations.0.receiver,
      agent: conversations.agent(),
      owner,
      chat_id,
      round: &round,
      messages: &messages,
      instructions: &instructions,
      instruction_ids: &ids,
      events: &events,
      holds: &holds,
      consumed,
      tools_allowed,
      reader: Some(conversations.0.receiver.roads()),
      announced: true,
    })
    .await;
    let mut state = conversations.state();
    let conv = state.active.get_mut(&chat_id)?;
    // A cut-off heard while the paragraph was built let go what it carries, so it does not go out.
    let goes = conv.carried.as_ref().is_some_and(|carried| carried.open && carried.taken.is_some());

    match built {
      Ok(paragraph) if goes => {
        let text = paragraph.text.clone();

        *self.offered() = Some(paragraph);

        Some(text)
      }
      built => {
        if let Err(why) = built {
          eprintln!("chat {chat_id}: words for the working turn could not be put in: {}", why.detail);
        }

        conv.give_back_taken();

        if let Some(carried) = conv.carried.as_mut() {
          carried.pushing = false;
        }

        None
      }
    }
  }

  /// The turn carrying the round now, as the agency named it: the rows its prompt holds written as carried by it
  /// where no turn carried them before, and every row pushed into it from now on.
  pub async fn carried_by(&self, session: &str, turn: i64) {
    let own = {
      let mut state = self.conversations.state();
      let Some(carried) = state.active.get_mut(&self.chat_id).and_then(|conv| conv.carried.as_mut()) else {
        return;
      };

      carried.turn = Some((session.to_owned(), turn));
      carried.own.clone()
    };

    if let Err(why) =
      super::instructions::carried_by(self.conversations.0.receiver.db(), &own, session, turn).await
    {
      eprintln!(
        "chat {}: the rows turn {turn} carries were not written down: {}",
        self.chat_id, why.detail
      );
    }
  }

  /// How far the pushes into the round's turn have come now.
  pub fn mark(&self) -> PushMark {
    self
      .conversations
      .state()
      .active
      .get(&self.chat_id)
      .and_then(|conv| conv.carried.as_ref())
      .map(|carried| PushMark {
        shown: carried.shown.len(),
        settling: carried.settling.len(),
        pushed: carried.pushed.len(),
        pushed_instructions: carried.pushed_instructions.len(),
      })
      .unwrap_or_default()
  }

  /// What was pushed into a turn begun at the mark, given back as that turn is handed back: the session that goes
  /// on never received it (docs/drafting.md, 「A round handed to the double」). Its words go back to wait at the
  /// front of the room's queue, its rows back to pending with their carrying cleared, and the turn the round
  /// carried before stands as the round's carrier again.
  pub async fn hand_back(&self, mark: PushMark, before: Option<(String, i64)>) {
    let chat_id = self.chat_id;
    let (words, rows) = {
      let mut state = self.conversations.state();
      let Some(carried) = state.active.get_mut(&chat_id).and_then(|conv| conv.carried.as_mut()) else {
        return;
      };
      let words: Vec<Inbound> = carried.shown.drain(mark.shown.min(carried.shown.len())..).collect();
      let rows: Vec<i64> = carried.settling.drain(mark.settling.min(carried.settling.len())..).collect();

      carried.pushed.truncate(mark.pushed);
      carried.pushed_instructions.truncate(mark.pushed_instructions);
      carried.turn = before;

      (words, rows)
    };

    if !words.is_empty() {
      println!(
        "chat {chat_id}: {} message(s) pushed into the double's turn go back to wait",
        words.len()
      );
    }

    self.conversations.give_back_words(chat_id, words);

    let receiver = &self.conversations.0.receiver;

    match super::instructions::returned(receiver.db(), &rows).await {
      Ok(given) if !given.is_empty() => {
        let named: Vec<String> = given.iter().map(i64::to_string).collect();

        println!(
          "chat {chat_id}: instructions {} pushed into the double's turn are pending again",
          named.join(", ")
        );

        if let Some(pass) = receiver.pass() {
          pass.kick();
        }
      }
      Ok(_) => {}
      Err(why) => eprintln!(
        "chat {chat_id}: the rows pushed into the double's turn were not given back: {}",
        why.detail
      ),
    }
  }

  /// The session and turn carrying the round now, where the agency named one.
  pub fn carrier(&self) -> Option<(String, i64)> {
    self
      .conversations
      .state()
      .active
      .get(&self.chat_id)
      .and_then(|conv| conv.carried.as_ref())
      .and_then(|carried| carried.turn.clone())
  }

  // What the turn took joins the round: its messages shown to the round, waiting for the marks that settle them,
  // its instructions among the rows the round settles, carried by the turn that took them. What it refused goes
  // back to the front of what waits, and a turn that says it is waiting takes no more. What a cut-off let go while
  // the push was out neither joins the round nor goes back.
  pub async fn settle(&self, outcome: AsideOutcome) {
    let chat_id = self.chat_id;
    let Some(paragraph) = self.offered().take() else { return };
    let conversations = &self.conversations;
    let taken = {
      let mut state = conversations.state();
      let Some(conv) = state.active.get_mut(&chat_id) else { return };
      let Some(carried) = conv.carried.as_mut() else { return };

      carried.pushing = false;

      if outcome == AsideOutcome::Waiting {
        carried.shut = true;
      }

      let Some(taken) = carried.taken.take() else { return };

      if outcome == AsideOutcome::Delivered {
        let Taken {
          messages,
          instructions,
          ids,
          roads,
          ..
        } = taken;
        let took =
          (messages.len(), instructions.len(), messages.clone(), ids.clone(), carried.turn.clone());

        carried.shown.extend(messages);
        carried.roads.extend(roads);
        carried.settling.extend(ids);
        carried.pushed.push(paragraph.text);
        carried.holds.extend(paragraph.reference);

        if !paragraph.instructions.is_empty() {
          carried.pushed_instructions.push(paragraph.instructions);
        }

        Some(took)
      } else {
        conv.give_back(taken.messages, taken.instructions, taken.ids, taken.roads, taken.events);
        None
      }
    };
    let Some((messages, pushed, took, ids, turn)) = taken else { return };

    if let Some((session, turn)) = turn
      && let Err(why) =
        super::instructions::carried_by(conversations.0.receiver.db(), &ids, &session, turn).await
    {
      eprintln!("chat {chat_id}: the rows pushed into turn {turn} were not written down: {}", why.detail);
    }

    conversations.took_meanwhile(chat_id, &took);
    println!(
      "chat {chat_id}: {messages} message(s) and {pushed} instruction(s) went into the working turn"
    );
  }

  // The working turn is over: it takes no more, words offered and never answered for go back, and what the
  // pushes grew the round by is the round's to decide on.
  pub fn finish(&self) -> Option<Carried> {
    self.offered().take();
    self.conversations.0.receiver.news().clear_standing(self.slot);

    let mut state = self.conversations.state();
    let conv = state.active.get_mut(&self.chat_id)?;

    conv.give_back_taken();
    conv.carried.take()
  }

  // The turn's picture, published to the room's screens where it changed (docs/receiver.md, 「The event stream」).
  pub fn progressed(&self, progress: &RunProgress) {
    let told = self.told.lock().unwrap_or_else(PoisonError::into_inner).tell(
      progress,
      now_ms(),
      CHAT_ROUND.progress.gap_ms as i64,
    );
    let Some(picture) = told else { return };

    let message = self
      .conversations
      .state()
      .active
      .get(&self.chat_id)
      .and_then(|conv| conv.held.last_consumed_id);

    self.conversations.publish_standing(
      self.slot,
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

// A turn left on a way that never finished it lets its picture go all the same.
impl Drop for Follower {
  fn drop(&mut self) {
    self.conversations.0.receiver.news().clear_standing(self.slot);
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

// The agent as the wake judges it: the default answerer known as such whether or not the agency registers it, and
// where the agency does not answer, the agent by its name alone.
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
      owner_account: None,
      default_answerer: agent == receiver.default_answerer(),
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

/// Whether the agent's turns go on one session across its rooms, as its registration says.
/// One room's situation note updated by one turn of the agent's one session, in that room (docs/drafting.md,
/// 「The situation note」): the note as it stands and what the room's record holds past it go in, the answer is the
/// note, and the room is owed none from then on. A room whose record holds nothing the note has not read is
/// settled with no turn. A turn whose run failed leaves the room owed. The turn goes on the session as it stands:
/// the session that answered in the room writes its note, whatever is about to become of it. Whether a turn went in.
async fn update_note(
  receiver: &Receiver,
  conversations: &Conversations,
  carrying: &dyn Carrying,
  agent: &str,
  chat_id: i64,
  session: Option<&str>,
) -> Result<bool, Failure> {
  let db = receiver.db();
  let update = super::situation::update_of(receiver, receiver.owner_id(), chat_id)
    .await
    .map_err(|why| Failure::Failed(why.detail))?;
  let Some(update) = update else {
    super::situation::settle(db, agent, chat_id).await;

    return Ok(false);
  };
  let asking = Asking {
    chat_id,
    session: session.unwrap_or_default().to_owned(),
    prompt: update.prompt.clone(),
    via: None,
    privileged: false,
    answers: vec![],
    began: now_ms(),
    context: vec![],
    thread_mark: None,
    standing: true,
    double: None,
  };
  let held = Held {
    agent: agent.to_owned(),
    chat_id,
    session: session.map(str::to_owned),
    account: None,
    context_tokens: None,
    all_from_owner: false,
    replaced: vec![],
    last_consumed_id: None,
  };
  let follower = Follower::new(conversations, chat_id);
  let carried = carrying.carry(&asking, &held, &follower).await;

  follower.finish();

  let reply = carried?;

  if let Err(why) = super::situation::written(db, &update, reply.session.as_deref(), &reply.text).await {
    eprintln!("chat {chat_id}: the situation note was not kept: {}", why.detail);
  }

  super::situation::settle(db, agent, chat_id).await;

  Ok(true)
}

pub async fn unified_session(receiver: &Receiver, agent: &str) -> bool {
  let listed = receiver.agents().await.ok();

  listed
    .as_ref()
    .and_then(|listed| listed.agents.iter().find(|one| one.name == agent))
    .and_then(|found| found.row.get("sessionScope").and_then(Value::as_str))
    .is_some_and(|scope| scope == "agent")
}

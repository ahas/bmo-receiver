//! The agency port: the supervisors, the roles and the repositories, the sessions a turn is carried in and
//! the words pushed into a working one, the documents a session was handed in full, a room forgotten, a
//! supervisor registered, ended and started afresh, the hooks the receiver answers, a run's log and the event
//! stream (`docs/boundary-api.md`, 「The agency」; the agency's `docs/api.md`). Bodies are camelCase.

use futures_util::StreamExt;
use futures_util::future::BoxFuture;
use futures_util::stream::BoxStream;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sse_stream::SseStream;

use crate::http::{Answer, Caller, Nothing, Peer, PortFailure};

/// One agent the receiver wakes, as the agency lists it among its supervisors: its name, whether it is
/// awake, the brain folder its long-term memory is in, and every other field of its row as the agency gives
/// them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
  pub name: String,
  #[serde(default)]
  pub label: Option<String>,
  /// An agent listed without saying is taken as awake: the agency lists the supervisors it stands.
  #[serde(default = "listed_awake")]
  pub awake: bool,
  #[serde(default)]
  pub brain_folder: Option<String>,
  #[serde(flatten)]
  pub row: Map<String, Value>,
}

fn listed_awake() -> bool {
  true
}

impl Agent {
  /// Whether the deployment seeds the supervisor, which the agency never takes out.
  pub fn seeded(&self) -> bool {
    self.row.get("seeded").and_then(Value::as_bool).unwrap_or(false)
  }
}

/// A supervisor's reset point alone: the moment in epoch milliseconds its last reset cut its turns off at,
/// none where it was never reset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetPoint {
  pub supervisor: String,
  #[serde(default)]
  pub reset_point: Option<i64>,
}

/// Every supervisor, and whether more stand beyond the page (the agency's page of supervisors).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Agents {
  #[serde(rename = "items")]
  pub agents: Vec<Agent>,
  #[serde(default)]
  pub more: bool,
}

/// A bundle of functions a role gives, by its name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bundle {
  pub name: String,
}

/// One role a piece of work runs as: its name, its label, what it is for and the bundles it gives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Role {
  pub name: String,
  #[serde(default)]
  pub label: Option<String>,
  #[serde(default)]
  pub description: Option<String>,
  #[serde(default)]
  pub bundles: Vec<Bundle>,
}

impl Role {
  /// Whether the role gives the bundle named.
  pub fn gives(&self, bundle: &str) -> bool {
    self.bundles.iter().any(|given| given.name == bundle)
  }
}

#[derive(Deserialize)]
struct RoleList {
  #[serde(default)]
  roles: Vec<Role>,
}

/// One repository a job runs in, by its name and its label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repository {
  pub name: String,
  #[serde(default)]
  pub label: Option<String>,
}

#[derive(Deserialize)]
struct RepositoryList {
  #[serde(default)]
  items: Vec<Repository>,
}

/// The scope a session is kept for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Scope {
  /// One session across rooms.
  Agent,
  /// A session of one room.
  Room { room: i64 },
  /// The session of one run of a role.
  Role,
  /// A conversation the caller keeps for itself.
  Conversation,
}

impl Scope {
  /// The scope's word, as a session's start and its member name it.
  pub fn word(&self) -> &'static str {
    match self {
      Self::Agent => "agent",
      Self::Room { .. } => "room",
      Self::Role => "role",
      Self::Conversation => "conversation",
    }
  }

  /// The room a room's scope names, and none for any other.
  pub fn room(&self) -> Option<i64> {
    match self {
      Self::Room { room } => Some(*room),
      _ => None,
    }
  }
}

// The scope a session's member names by its word and its room, or the agent's where it names neither.
fn scope_of(word: &str, room: Option<i64>) -> Scope {
  match word {
    "room" => Scope::Room {
      room: room.unwrap_or_default(),
    },
    "role" => Scope::Role,
    "conversation" => Scope::Conversation,
    _ => Scope::Agent,
  }
}

/// A session: its id, its agent and its scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
  pub id: String,
  pub agent: String,
  pub scope: Scope,
}

/// A session's member as the agency answers it, by its id: the agent and scope it names, and what its route
/// answers beside them.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionMember {
  id: String,
  agent_name: String,
  scope: String,
  #[serde(default)]
  room_id: Option<i64>,
  #[serde(default)]
  provider: Option<String>,
  #[serde(default)]
  model: Option<String>,
  #[serde(default)]
  place: Value,
  #[serde(default)]
  generation: Value,
  #[serde(default)]
  context: Value,
  #[serde(default)]
  fold_ratio: Option<f64>,
  #[serde(default)]
  folds_itself: bool,
  #[serde(default)]
  supervisor: Option<String>,
  #[serde(default)]
  role: Option<String>,
  #[serde(default)]
  prefix_change_reason: Option<String>,
  #[serde(default)]
  handoff_lost: bool,
}

impl SessionMember {
  fn session(self) -> Session {
    Session {
      id: self.id,
      agent: self.agent_name,
      scope: scope_of(&self.scope, self.room_id),
    }
  }

  fn state(self) -> SessionState {
    SessionState {
      session: self.id,
      provider: self.provider.filter(|provider| !provider.is_empty()),
      model: self.model.filter(|model| !model.is_empty()),
      place: self.place,
      generation: self.generation,
      context: self.context,
      fold_ratio: self.fold_ratio,
      folds_itself: self.folds_itself,
      supervisor: self.supervisor.filter(|supervisor| !supervisor.is_empty()),
      role: self.role.filter(|role| !role.is_empty()),
      prefix_change_reason: self.prefix_change_reason.filter(|why| !why.trim().is_empty()),
      handoff_lost: self.handoff_lost,
    }
  }
}

/// The room a turn answers: its chat, its block's text and the mark its answer carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnRoom {
  pub chat_id: i64,
  pub text: String,
  pub mark: String,
}

/// One turn carried into a session: its prompt and the context the receiver composed around it, every part of
/// the context optional.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Turn {
  /// The session the turn goes into, named in the route rather than the body.
  #[serde(skip_serializing)]
  pub session: String,
  pub prompt: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub room: Option<TurnRoom>,
  /// The road the turn answers its room on, where that is not the room's own.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub via: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub privileged: Option<bool>,
  /// The messages the turn answers.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub answers: Vec<i64>,
  /// When the turn took its words, in epoch milliseconds.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub began: Option<i64>,
  /// The name of the round the turn belongs to, which the turn's run keeps.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub round: Option<String>,
  /// The carrying's own key, under which a send made again attaches to the turn standing rather than starting a
  /// second.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub key: Option<String>,
}

/// Whether the program took a turn's words or refused them while it waits on a person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Delivery {
  /// The program took the words; a turn's end names it `taken`.
  #[serde(alias = "taken")]
  Delivered,
  Blocked,
}

/// What became of words pushed into the turn a session is working: the turn took them, its program waits on a
/// person, or they did not go in and are the caller's to carry into the next turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Said {
  Delivered,
  Waiting,
  Refused,
}

/// A session's state: the provider and model it stands on, its place, its generation and the context its last
/// request carried, as the agency gives them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SessionState {
  pub session: String,
  #[serde(default)]
  pub provider: Option<String>,
  #[serde(default)]
  pub model: Option<String>,
  #[serde(default)]
  pub place: Value,
  #[serde(default)]
  pub generation: Value,
  #[serde(default)]
  pub context: Value,
  /// The share of its window at which the session folds, its supervisor's or its role's compaction
  /// threshold.
  #[serde(default, rename = "foldRatio")]
  pub fold_ratio: Option<f64>,
  /// Whether the session's program carries its own fold, so the receiver folds it never.
  #[serde(default, rename = "foldsItself")]
  pub folds_itself: bool,
  /// The supervisor the session runs for, where it is a supervisor's.
  #[serde(default)]
  pub supervisor: Option<String>,
  /// The role the session runs for, where it is a role's run.
  #[serde(default)]
  pub role: Option<String>,
  /// Why the session's prefix changed, where the agency names it: a reason another than the one the session's last
  /// turn stood on says its head moved.
  #[serde(default, rename = "prefixChangeReason")]
  pub prefix_change_reason: Option<String>,
  /// Whether the agency opened the session, or its generation, without the handoff it was owed: its first turn
  /// then opens on the receiver's transcript.
  #[serde(default, rename = "handoffLost")]
  pub handoff_lost: bool,
}

/// A supervisor the console registers: the name it is registered by, what it is called in the room, the
/// repository its items go to where it names one, and its one road: its provider and model, with the effort,
/// thinking and answer ceiling the console names, each handed on as the console gave it for the agency to judge.
/// The rooms it answers are the receiver's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSupervisor {
  pub name: String,
  pub label: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub provider: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub model: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub repository: Option<String>,
  /// A level of the reasoning policy; the provider's own where none is named.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub effort: Option<String>,
  /// `on` or `off`; the provider's own where none is named.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub thinking: Option<String>,
  /// The answer ceiling, on the registry's provider.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub answer_tokens: Option<i64>,
  /// The account the supervisor belongs to; none where the caller gives none.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub owner_account: Option<i64>,
}

/// A supervisor's registration ended: the rooms that name it, read as rooms it shares so each keeps its record,
/// whether its brain goes with it, and whether a turn holding it is ended under it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Removal {
  pub rooms: Vec<i64>,
  pub forget_brain: bool,
  pub force: bool,
}

/// A field of the body the agency refused: the field's name and the English sentence saying why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldRefusal {
  pub field: String,
  pub error: String,
}

/// A table's rows that stayed with the agency as a room was forgotten, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stayed {
  pub table: String,
  pub rows: i64,
  pub why: String,
}

/// What forgetting a room took, table by table, or would take in a dry run, and what stayed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Forgotten {
  #[serde(default)]
  pub gone: Map<String, Value>,
  #[serde(default)]
  pub stayed: Vec<Stayed>,
}

/// One line of a run's log, as the agency gives it.
pub type RunLine = Map<String, Value>;

/// The block a session's answer is still writing, as the agency's model gateway counts it: its place in the
/// answer, its kind (`thinking`, `text` or `tool`) and the bytes it has written so far.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct OpenBlock {
  pub index: u64,
  pub kind: String,
  pub bytes: u64,
}

/// One event off the agency's stream: its kind and its data, and the number the agency gave it and the moment it
/// published it where its envelope names them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Event {
  pub kind: String,
  pub data: Value,
  pub id: Option<i64>,
  pub at: Option<i64>,
}

/// The events the agency still keeps after a number, oldest first, and whether more wait past the page.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KeptEvents {
  pub items: Vec<Event>,
  pub more: bool,
}

/// A turn as the agency's progress read answers it: whether it still runs, and how it ended and what it answered
/// once it has.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct TurnRead {
  #[serde(default)]
  pub state: String,
  #[serde(default)]
  pub outcome: Option<String>,
  #[serde(default)]
  pub answer: Option<String>,
  /// Why it ended as it did, where the agency says.
  #[serde(default)]
  pub reason: Option<String>,
  /// Whether its program took the turn's words, once it has ended.
  #[serde(default)]
  pub delivery: Option<Delivery>,
}

impl TurnRead {
  /// Whether the turn has ended.
  pub fn ended(&self) -> bool {
    self.state == "ended"
  }
}

// One kept event as the agency's page answers it: its envelope.
#[derive(Deserialize)]
struct Envelope {
  id: i64,
  kind: String,
  #[serde(default)]
  at: Option<i64>,
  #[serde(default)]
  data: Value,
}

#[derive(Deserialize)]
struct EnvelopePage {
  #[serde(default)]
  items: Vec<Envelope>,
  #[serde(default)]
  more: bool,
}

// How many kept events one page asks for, the most the agency answers.
const KEPT_PAGE: usize = 1000;

/// The events of the kinds asked, as they come; a stream that drops ends, and is opened again by its reader.
pub type Events = BoxStream<'static, Answer<Event>>;

/// Why a rest stands: a login's usage limit, or the service being down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RestKind {
  Limit,
  Outage,
}

/// The deployment's rest: whether it stands now, until when in epoch seconds, and why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rest {
  pub resting: bool,
  #[serde(default)]
  pub until: Option<i64>,
  #[serde(default)]
  pub kind: Option<RestKind>,
}

/// Why the deployment rested where its rest has just passed: none while it still rests or rested for no
/// reason that ended the silence.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Passed {
  #[serde(default)]
  pub kind: Option<RestKind>,
}

/// A document a session was handed in full, at the version it read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Load {
  pub path: String,
  pub version: String,
}

/// A run as the agency keeps its row, of which the receiver reads the moment it began, in epoch milliseconds.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
  pub started_at: i64,
}

/// A run still open, as the list of runs answers it: its id, its unit's name and when it started.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenRun {
  pub id: i64,
  #[serde(default)]
  pub name: String,
  #[serde(default)]
  pub started_at: i64,
  /// The job the run is a turn of, where it is one.
  #[serde(default)]
  pub job_id: Option<i64>,
  /// The round the run is a turn of, where it is one of the receiver's rounds.
  #[serde(default)]
  pub round: Option<String>,
  /// The room the run's turn answers, where it answers one.
  #[serde(default)]
  pub room_id: Option<i64>,
}

#[derive(Deserialize)]
struct OpenRuns {
  items: Vec<OpenRun>,
}

/// A job starting or running, as the list of jobs answers it: its id, the role it runs as, the repository
/// it runs in and the checkout of it, its item and when it started.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningJob {
  pub id: i64,
  #[serde(default)]
  pub role_name: Option<String>,
  /// None for a checkout no repository holds any longer.
  #[serde(default)]
  pub repository_name: Option<String>,
  #[serde(default)]
  pub worker_name: Option<String>,
  #[serde(default)]
  pub item_id: Option<i64>,
  #[serde(default)]
  pub started_at: Option<i64>,
}

#[derive(Deserialize)]
struct RunningJobs {
  items: Vec<RunningJob>,
}

/// Every call the receiver makes on the agency.
pub trait Agency: Send + Sync {
  /// Every supervisor with its row.
  fn supervisors(&self) -> BoxFuture<'_, Answer<Agents>>;

  /// One supervisor with its row, read afresh; none where no supervisor has the name.
  fn supervisor<'a>(&'a self, name: &'a str) -> BoxFuture<'a, Answer<Option<Agent>>>;

  /// A supervisor's reset point alone, read afresh; none where no supervisor has the name.
  fn reset_point<'a>(&'a self, name: &'a str) -> BoxFuture<'a, Answer<Option<ResetPoint>>>;

  /// Every role a piece of work runs as.
  fn roles(&self) -> BoxFuture<'_, Answer<Vec<Role>>>;

  /// Every repository a job runs in.
  fn repositories(&self) -> BoxFuture<'_, Answer<Vec<Repository>>>;

  /// Whether a supervisor's one road takes a turn now, as a turn's start would judge it; refused 404 where
  /// no supervisor has the name.
  fn road_answers<'a>(&'a self, supervisor: &'a str) -> BoxFuture<'a, Answer<bool>>;

  /// A supervisor's brain and reference switches, as its settings say (docs/drafting.md, 「A supervisor
  /// whose brain or reference is off」): each on where they do not say it, where no supervisor has the name,
  /// and on an agency that keeps no such setting.
  fn switches<'a>(&'a self, _supervisor: &'a str) -> BoxFuture<'a, Answer<Switches>> {
    Box::pin(async { Ok(Switches::default()) })
  }

  /// The deployment's rest: whether it stands, until when, and why.
  fn rest(&self) -> BoxFuture<'_, Answer<Rest>>;

  /// Why the deployment rested where its rest has just passed.
  fn passed_rest(&self) -> BoxFuture<'_, Answer<Passed>>;

  /// The session kept for the agent at the scope, started where none is.
  fn start<'a>(&'a self, agent: &'a str, scope: Scope) -> BoxFuture<'a, Answer<Session>>;

  /// One turn carried into its session until it ends. A fault of the session's comes back as the agency's
  /// answer, its outcome code the failure's code (`session-busy`, `unknown-session` and the rest).
  fn turn<'a>(&'a self, turn: &'a Turn) -> BoxFuture<'a, Answer<Delivery>>;

  /// Everything a slot of the agency's double role asked under `owner` holds given back: the request, its
  /// reservation and the role session bound to it, whose running turn the agency cuts.
  fn double_release<'a>(&'a self, _owner: &'a str) -> BoxFuture<'a, Answer<()>> {
    Box::pin(async { Ok(()) })
  }

  fn state<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<SessionState>>;

  /// The turn a session is working cut short.
  fn interrupt<'a>(&'a self, session: &'a str, turn: i64) -> BoxFuture<'a, Answer<()>>;

  fn fold<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<()>>;

  fn end<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<()>>;

  /// Every session the agency keeps.
  fn sessions(&self) -> BoxFuture<'_, Answer<Vec<Session>>>;

  /// A room's rows forgotten, or counted where `dry_run`.
  fn forget_room(&self, chat_id: i64, dry_run: bool) -> BoxFuture<'_, Answer<Forgotten>>;

  /// The receiver's address registered at a hook's point, in place of any before.
  fn register_hook<'a>(&'a self, point: &'a str, url: &'a str, token: &'a str)
  -> BoxFuture<'a, Answer<()>>;

  /// A run's log lines after the one given, five hundred at most.
  fn run_log<'a>(&'a self, run: &'a str, after: Option<i64>) -> BoxFuture<'a, Answer<Vec<RunLine>>>;

  /// The block a session's answer is still writing; none where none is open, which is all an agency that counts
  /// no block answers.
  fn open_block<'a>(&'a self, _session: &'a str) -> BoxFuture<'a, Answer<Option<OpenBlock>>> {
    Box::pin(async { Ok(None) })
  }

  /// A run's row by its id; none where the agency holds none.
  fn run(&self, id: i64) -> BoxFuture<'_, Answer<Option<Run>>>;

  /// The ids of the runs a round's turns opened, in the order the agency answers them, newest first.
  fn round_runs<'a>(&'a self, round: &'a str) -> BoxFuture<'a, Answer<Vec<i64>>>;

  /// The runs still open, newest first.
  fn open_runs(&self) -> BoxFuture<'_, Answer<Vec<OpenRun>>>;

  /// The jobs starting or running, newest first.
  fn running_jobs(&self) -> BoxFuture<'_, Answer<Vec<RunningJob>>>;

  /// The message a run's answer went out as, attached to the run; its closing stays as it stands.
  fn attach_message(&self, run: i64, message: i64) -> BoxFuture<'_, Answer<()>>;

  /// The documents a session was handed in full, each at the version it read.
  fn loads<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<Vec<Load>>>;

  /// A session's loads kept, every one it now holds in place of those it held.
  fn keep_loads<'a>(&'a self, session: &'a str, loads: &'a [Load]) -> BoxFuture<'a, Answer<()>>;

  fn forget_loads<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<()>>;

  /// Words put into the turn the session is working, answered with what became of them.
  fn push<'a>(&'a self, session: &'a str, turn: i64, words: &'a str) -> BoxFuture<'a, Answer<Said>>;

  /// A supervisor registered, answered with the agency's `supervisor` and `brain`, or the field the agency
  /// refused. A name the agency holds already is its 409 `name-taken`.
  fn add_supervisor<'a>(
    &'a self,
    supervisor: &'a NewSupervisor,
  ) -> BoxFuture<'a, Answer<Result<Value, FieldRefusal>>>;

  /// A supervisor's registration ended with everything of its own; the agency's 409 `seeded` and `running`
  /// are failures carrying those codes.
  fn remove_supervisor<'a>(&'a self, name: &'a str, removal: &'a Removal) -> BoxFuture<'a, Answer<()>>;

  /// A supervisor started afresh in the ranges the body names, answered as the agency answers the reset.
  fn reset_supervisor<'a>(&'a self, name: &'a str, reset: &'a Value) -> BoxFuture<'a, Answer<Value>>;

  /// The stream of the events of the kinds named, from the moment it is opened.
  fn events<'a>(&'a self, kinds: &'a [&'a str]) -> BoxFuture<'a, Answer<Events>>;

  /// The events of every kind the agency still keeps after the number given, from its oldest where none is given.
  /// An agency that keeps none answers none.
  fn kept_events(&self, _after: Option<i64>) -> BoxFuture<'_, Answer<KeptEvents>> {
    Box::pin(async { Ok(KeptEvents::default()) })
  }

  /// A turn of a session as the agency's progress read answers it; `unknown-turn` where the session holds no such
  /// turn.
  fn turn_read<'a>(&'a self, session: &'a str, turn: i64) -> BoxFuture<'a, Answer<TurnRead>> {
    Box::pin(async move {
      Err(PortFailure::Transport {
        url: format!("agency/sessions/{session}/turns/{turn}"),
        why: "this agency reads no turn".to_owned(),
      })
    })
  }
}

/// The agency over HTTP.
#[derive(Debug, Clone)]
pub struct HttpAgency {
  caller: Caller,
}

impl HttpAgency {
  pub fn new(peer: &Peer) -> Self {
    Self::with_caller(Caller::new(peer))
  }

  pub fn with_caller(caller: Caller) -> Self {
    Self { caller }
  }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Start<'a> {
  agent_name: &'a str,
  scope: &'a str,
  #[serde(skip_serializing_if = "Option::is_none")]
  room_id: Option<i64>,
}

#[derive(Deserialize)]
struct Delivered {
  delivery: Delivery,
}

#[derive(Serialize)]
struct Words<'a> {
  words: &'a str,
}

#[derive(Deserialize)]
struct Pushed {
  said: Said,
}

#[derive(Serialize)]
struct Operated<'a> {
  state: &'a str,
}

#[derive(Deserialize)]
struct SessionPage {
  items: Vec<SessionMember>,
}

/// The body that clears the rests that have passed.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RestAsk {
  clear_passed: bool,
}

#[derive(Serialize)]
struct Hook<'a> {
  url: &'a str,
  token: &'a str,
}

#[derive(Deserialize)]
struct Lines {
  items: Vec<RunLine>,
}

/// A page of runs, of which the receiver reads each run's id.
#[derive(Deserialize)]
struct RunPage {
  items: Vec<RunId>,
}

#[derive(Deserialize)]
struct RunId {
  id: i64,
}

/// The body that attaches the message a run's answer went out as, naming nothing else.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Attached {
  message_id: i64,
}

/// A session's loads as the agency keeps them, and the body that keeps them.
#[derive(Serialize, Deserialize)]
struct Documents<T> {
  documents: T,
}

// The event's own data: the agency's stream wraps each event as its number, kind, moment and data, and a reader
// that unwraps none of it is left with the body as the member answers it.
fn body_of(data: Value, kind: &str) -> Value {
  match data.get("kind").and_then(Value::as_str) {
    Some(named) if named == kind => data.get("data").cloned().unwrap_or(Value::Null),
    _ => data,
  }
}

// One log line as the receiver reads it: the agency's own fields, with the line's structured fields laid
// beside them, its reference under the name the receiver reads it by.
fn run_line(line: RunLine) -> RunLine {
  let Some(fields) = line.get("fields").and_then(Value::as_object).cloned() else {
    return line;
  };
  let mut read = line;

  for (key, value) in fields {
    read.entry(key).or_insert(value);
  }

  if let Some(reference) = read.get("reference").cloned() {
    read.entry("ref".to_owned()).or_insert(reference);
  }

  read
}

/// A supervisor's road as the agency answers it, of which the receiver reads whether it takes a turn now.
#[derive(Deserialize)]
struct SupervisorRoad {
  answers: bool,
}

/// A supervisor as its settings page answers it, of which the receiver reads its two switches.
#[derive(Deserialize)]
struct SupervisorSettingsPage {
  #[serde(default)]
  settings: Option<Switches>,
}

/// A supervisor's switches over what of its brain reaches its turns, each on where its settings do not
/// say it (docs/drafting.md, 「A supervisor whose brain or reference is off」).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Switches {
  /// Whether its brain is on.
  #[serde(default = "on")]
  pub brain_on: bool,
  /// Whether its turns are given their reference.
  #[serde(default = "on")]
  pub reference_on: bool,
}

impl Default for Switches {
  fn default() -> Self {
    Switches {
      brain_on: true,
      reference_on: true,
    }
  }
}

impl Switches {
  /// Whether the brain reaches the supervisor's turns: both switches on, since either off closes it.
  pub fn brain_reaches(self) -> bool {
    self.brain_on && self.reference_on
  }
}

fn on() -> bool {
  true
}

impl Agency for HttpAgency {
  fn supervisors(&self) -> BoxFuture<'_, Answer<Agents>> {
    Box::pin(self.caller.get(&["supervisors"], &[]))
  }

  fn supervisor<'a>(&'a self, name: &'a str) -> BoxFuture<'a, Answer<Option<Agent>>> {
    Box::pin(async move { self.caller.found(&["supervisors", name], &[]).await })
  }

  fn reset_point<'a>(&'a self, name: &'a str) -> BoxFuture<'a, Answer<Option<ResetPoint>>> {
    Box::pin(async move { self.caller.found(&["supervisors", name, "reset-point"], &[]).await })
  }

  fn roles(&self) -> BoxFuture<'_, Answer<Vec<Role>>> {
    Box::pin(async move {
      let list: RoleList = self.caller.get(&["roles"], &[]).await?;

      Ok(list.roles)
    })
  }

  fn repositories(&self) -> BoxFuture<'_, Answer<Vec<Repository>>> {
    Box::pin(async move {
      let list: RepositoryList = self.caller.get(&["repositories"], &[]).await?;

      Ok(list.items)
    })
  }

  fn road_answers<'a>(&'a self, supervisor: &'a str) -> BoxFuture<'a, Answer<bool>> {
    Box::pin(async move {
      let road: SupervisorRoad = self.caller.get(&["supervisors", supervisor, "road"], &[]).await?;

      Ok(road.answers)
    })
  }

  fn switches<'a>(&'a self, supervisor: &'a str) -> BoxFuture<'a, Answer<Switches>> {
    Box::pin(async move {
      let page: Option<SupervisorSettingsPage> =
        self.caller.found(&["supervisors", supervisor, "settings"], &[]).await?;

      Ok(page.and_then(|page| page.settings).unwrap_or_default())
    })
  }

  fn rest(&self) -> BoxFuture<'_, Answer<Rest>> {
    Box::pin(self.caller.get(&["rest"], &[]))
  }

  fn passed_rest(&self) -> BoxFuture<'_, Answer<Passed>> {
    Box::pin(async move {
      let ask = RestAsk { clear_passed: true };

      self.caller.patch(&["rest"], &ask).await
    })
  }

  fn start<'a>(&'a self, agent: &'a str, scope: Scope) -> BoxFuture<'a, Answer<Session>> {
    Box::pin(async move {
      let read: SessionMember = self
        .caller
        .post(
          &["sessions"],
          &Start {
            agent_name: agent,
            scope: scope.word(),
            room_id: scope.room(),
          },
        )
        .await?;

      Ok(read.session())
    })
  }

  fn turn<'a>(&'a self, turn: &'a Turn) -> BoxFuture<'a, Answer<Delivery>> {
    Box::pin(async move {
      let delivered: Delivered = self.caller.post(&["sessions", &turn.session, "turns"], turn).await?;

      Ok(delivered.delivery)
    })
  }

  fn double_release<'a>(&'a self, owner: &'a str) -> BoxFuture<'a, Answer<()>> {
    Box::pin(async move {
      let _: Value = self.caller.delete(&["slot-requests"], &[("owner", owner.to_owned())]).await?;

      Ok(())
    })
  }

  fn state<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<SessionState>> {
    Box::pin(async move {
      let read: SessionMember = self.caller.get(&["sessions", session], &[]).await?;

      Ok(read.state())
    })
  }

  fn interrupt<'a>(&'a self, session: &'a str, turn: i64) -> BoxFuture<'a, Answer<()>> {
    Box::pin(async move {
      let turn = turn.to_string();

      self
        .caller
        .done(
          Method::PATCH,
          &["sessions", session, "turns", &turn],
          &Operated {
            state: "interrupted",
          },
        )
        .await
    })
  }

  fn fold<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<()>> {
    Box::pin(
      async move { self.caller.done(Method::POST, &["sessions", session, "folds"], &Nothing {}).await },
    )
  }

  fn end<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<()>> {
    Box::pin(async move { self.caller.without(Method::DELETE, &["sessions", session]).await })
  }

  fn sessions(&self) -> BoxFuture<'_, Answer<Vec<Session>>> {
    Box::pin(async move {
      let page: SessionPage = self.caller.get(&["sessions"], &[]).await?;

      Ok(page.items.into_iter().map(SessionMember::session).collect())
    })
  }

  fn forget_room(&self, chat_id: i64, dry_run: bool) -> BoxFuture<'_, Answer<Forgotten>> {
    Box::pin(async move {
      let room = chat_id.to_string();
      let room = ["room-holdings", room.as_str()];

      if !dry_run {
        return self.caller.without(Method::DELETE, &room).await.map(|()| Forgotten::default());
      }

      let query = [("dryRun", "true".to_owned())];

      self.caller.delete(&room, &query).await
    })
  }

  fn register_hook<'a>(
    &'a self,
    point: &'a str,
    url: &'a str,
    token: &'a str,
  ) -> BoxFuture<'a, Answer<()>> {
    Box::pin(async move { self.caller.done(Method::PUT, &["hooks", point], &Hook { url, token }).await })
  }

  fn run_log<'a>(&'a self, run: &'a str, after: Option<i64>) -> BoxFuture<'a, Answer<Vec<RunLine>>> {
    Box::pin(async move {
      let mut query = vec![("run", run.to_owned())];

      if let Some(after) = after {
        query.push(("after", after.to_string()));
      }

      let lines: Lines = self.caller.get(&["log-lines"], &query).await?;

      Ok(lines.items.into_iter().map(run_line).collect())
    })
  }

  fn run(&self, id: i64) -> BoxFuture<'_, Answer<Option<Run>>> {
    Box::pin(async move { self.caller.found(&["runs", &id.to_string()], &[]).await })
  }

  fn open_block<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<Option<OpenBlock>>> {
    Box::pin(async move { self.caller.present(&["sessions", session, "streaming"], &[]).await })
  }

  fn round_runs<'a>(&'a self, round: &'a str) -> BoxFuture<'a, Answer<Vec<i64>>> {
    Box::pin(async move {
      let page: RunPage = self.caller.get(&["runs"], &[("round", round.to_owned())]).await?;

      Ok(page.items.into_iter().map(|run| run.id).collect())
    })
  }

  fn open_runs(&self) -> BoxFuture<'_, Answer<Vec<OpenRun>>> {
    Box::pin(async move {
      let page: OpenRuns = self
        .caller
        .get(&["runs"], &[("live", "true".to_owned()), ("limit", "500".to_owned())])
        .await?;

      Ok(page.items)
    })
  }

  fn running_jobs(&self) -> BoxFuture<'_, Answer<Vec<RunningJob>>> {
    Box::pin(async move {
      let page: RunningJobs = self
        .caller
        .get(&["jobs"], &[("running", "true".to_owned()), ("limit", "500".to_owned())])
        .await?;

      Ok(page.items)
    })
  }

  fn attach_message(&self, run: i64, message: i64) -> BoxFuture<'_, Answer<()>> {
    Box::pin(async move {
      let run = run.to_string();

      self
        .caller
        .done(
          Method::PATCH,
          &["runs", &run],
          &Attached {
            message_id: message,
          },
        )
        .await
    })
  }

  fn loads<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<Vec<Load>>> {
    Box::pin(async move {
      let read: Documents<Vec<Load>> = self.caller.get(&["session-loads", session], &[]).await?;

      Ok(read.documents)
    })
  }

  fn keep_loads<'a>(&'a self, session: &'a str, loads: &'a [Load]) -> BoxFuture<'a, Answer<()>> {
    Box::pin(async move {
      self
        .caller
        .done(Method::PUT, &["session-loads", session], &Documents { documents: loads })
        .await
    })
  }

  fn forget_loads<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<()>> {
    Box::pin(async move { self.caller.without(Method::DELETE, &["session-loads", session]).await })
  }

  fn push<'a>(&'a self, session: &'a str, turn: i64, words: &'a str) -> BoxFuture<'a, Answer<Said>> {
    Box::pin(async move {
      let turn = turn.to_string();
      let pushed: Pushed = self
        .caller
        .post(&["sessions", session, "turns", &turn, "inputs"], &Words { words })
        .await?;

      Ok(pushed.said)
    })
  }

  fn add_supervisor<'a>(
    &'a self,
    supervisor: &'a NewSupervisor,
  ) -> BoxFuture<'a, Answer<Result<Value, FieldRefusal>>> {
    Box::pin(async move { self.caller.checked(&["supervisors"], supervisor).await })
  }

  fn remove_supervisor<'a>(&'a self, name: &'a str, removal: &'a Removal) -> BoxFuture<'a, Answer<()>> {
    Box::pin(async move {
      let rooms: Vec<String> = removal.rooms.iter().map(i64::to_string).collect();
      let query = [
        ("rooms", rooms.join(",")),
        ("forgetBrain", removal.forget_brain.to_string()),
        ("force", removal.force.to_string()),
      ];

      self.caller.emptied(&["supervisors", name], &query).await
    })
  }

  fn reset_supervisor<'a>(&'a self, name: &'a str, reset: &'a Value) -> BoxFuture<'a, Answer<Value>> {
    Box::pin(async move { self.caller.post(&["supervisors", name, "resets"], reset).await })
  }

  fn events<'a>(&'a self, kinds: &'a [&'a str]) -> BoxFuture<'a, Answer<Events>> {
    Box::pin(async move {
      let query: Vec<(&str, String)> =
        if kinds.is_empty() { vec![] } else { vec![("kinds", kinds.join(","))] };
      let url = self.caller.url(&["events"])?.to_string();
      let opened = self.caller.open(&["events"], &query).await?;
      let events = SseStream::from_bytes_stream(opened.bytes_stream()).filter_map(move |read| {
        let url = url.clone();

        async move {
          match read {
            Err(why) => Some(Err(PortFailure::Transport {
              url,
              why: why.to_string(),
            })),
            Ok(sse) => {
              let kind = sse.event?;
              let mut id = sse.id.as_deref().and_then(|id| id.trim().parse::<i64>().ok());
              let mut at = None;
              let data = match sse.data.as_deref() {
                Some(data) => match serde_json::from_str::<Value>(data) {
                  Ok(data) => {
                    if data.get("kind").and_then(Value::as_str) == Some(kind.as_str()) {
                      id = data.get("id").and_then(Value::as_i64).or(id);
                      at = data.get("at").and_then(Value::as_i64);
                    }

                    body_of(data, &kind)
                  }
                  Err(why) => {
                    return Some(Err(PortFailure::Undecodable {
                      url,
                      why: format!("the {kind} event's data: {why}"),
                    }));
                  }
                },
                None => Value::Null,
              };

              Some(Ok(Event { kind, data, id, at }))
            }
          }
        }
      });

      Ok(events.boxed())
    })
  }

  fn kept_events(&self, after: Option<i64>) -> BoxFuture<'_, Answer<KeptEvents>> {
    Box::pin(async move {
      let mut query = vec![("limit", KEPT_PAGE.to_string())];

      if let Some(after) = after {
        query.push(("after", after.to_string()));
      }

      let page: EnvelopePage = self.caller.get_json(&["events"], &query).await?;

      Ok(KeptEvents {
        items: page
          .items
          .into_iter()
          .map(|kept| Event {
            data: kept.data,
            kind: kept.kind,
            id: Some(kept.id),
            at: kept.at,
          })
          .collect(),
        more: page.more,
      })
    })
  }

  fn turn_read<'a>(&'a self, session: &'a str, turn: i64) -> BoxFuture<'a, Answer<TurnRead>> {
    Box::pin(async move { self.caller.get(&["sessions", session, "turns", &turn.to_string()], &[]).await })
  }
}

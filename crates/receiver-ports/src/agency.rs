//! The agency port: the agents and their roads, the sessions a turn is carried in and the words pushed into a
//! working one, the documents a session was handed in full, a room forgotten, the agent half of an agent added on the console, the hooks
//! the receiver answers, a run's log and the event stream (`docs/boundary-api.md`, 「The agency」; the agency's
//! `docs/api.md`). Bodies are camelCase.

use futures_util::StreamExt;
use futures_util::future::BoxFuture;
use futures_util::stream::BoxStream;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sse_stream::SseStream;

use crate::http::{Answer, Caller, Done, Nothing, Peer, PortFailure};

/// One agent as the agency lists it: its registration name, whether it is awake, the brain folder its long-term
/// memory is in, and every other field of its row and roads as the agency gives them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
  pub name: String,
  #[serde(default)]
  pub label: Option<String>,
  /// An agent listed without saying is taken as awake: the agency lists the agents it stands.
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

/// Every agent, every role an agent may hold, and whether more stand beyond the page (the agency's page of
/// agents).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Agents {
  #[serde(rename = "items")]
  pub agents: Vec<Agent>,
  #[serde(default)]
  pub roles: Vec<Value>,
  #[serde(default)]
  pub more: bool,
}

/// The scope a session is kept for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Scope {
  /// One session across rooms.
  Agent,
  /// A session of one room.
  Room {
    room: i64,
  },
  Worker,
  Editor,
  /// A session thrown away after its one use.
  Apart,
  /// A conversation the caller keeps for itself.
  Conversation,
}

impl Scope {
  /// The scope's word, as a session's start and its member name it.
  pub fn word(&self) -> &'static str {
    match self {
      Self::Agent => "agent",
      Self::Room { .. } => "room",
      Self::Worker => "worker",
      Self::Editor => "editor",
      Self::Apart => "apart",
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
    "worker" => Scope::Worker,
    "editor" => Scope::Editor,
    "apart" => Scope::Apart,
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
  tier: Option<String>,
  #[serde(default)]
  prefix_change_reason: Option<String>,
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
      tier: self.tier.filter(|tier| !tier.is_empty()),
      prefix_change_reason: self.prefix_change_reason.filter(|why| !why.trim().is_empty()),
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
}

/// Whether the program took a turn's words or refused them while it waits on a person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Delivery {
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
  /// The share of its window at which the session folds, as the agency's fold-ratio table gives it.
  #[serde(default, rename = "foldRatio")]
  pub fold_ratio: Option<f64>,
  /// Whether the session's program carries its own fold, so the receiver folds it never.
  #[serde(default, rename = "foldsItself")]
  pub folds_itself: bool,
  /// The tier the agency names for the session, where it names one: the draft process table is matched by it.
  #[serde(default)]
  pub tier: Option<String>,
  /// Why the session's prefix changed, where the agency names it: a reason another than the one the session's last
  /// turn stood on says its head moved.
  #[serde(default, rename = "prefixChangeReason")]
  pub prefix_change_reason: Option<String>,
}

/// The row of an agent the console stands up: the name it is registered by, what it is called in the room, the
/// provider and model it is pinned to, and the worker its items go to. The room it answers is the receiver's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsoleAgentRow {
  pub name: String,
  pub label: String,
  pub provider: String,
  pub model: String,
  pub worker: String,
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

/// One event off the agency's stream: its kind and its data.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
  pub kind: String,
  pub data: Value,
}

/// The events of the kinds asked, as they come; a stream that drops ends, and is opened again by its reader.
pub type Events = BoxStream<'static, Answer<Event>>;

/// Why a rest stands: a login's usage limit, or the service being down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RestKind {
  Limit,
  Outage,
}

impl RestKind {
  /// The word the agency names the kind by.
  pub fn word(self) -> &'static str {
    match self {
      Self::Limit => "limit",
      Self::Outage => "outage",
    }
  }
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

/// A rest written down: whether it is the one that silenced the deployment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Written {
  #[serde(default)]
  pub silenced: bool,
}

/// A rest written: until when in epoch seconds, why, and the login a limit is its own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Resting {
  pub until: i64,
  pub kind: RestKind,
  #[serde(default)]
  pub account: Option<String>,
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

/// Every call the receiver makes on the agency.
pub trait Agency: Send + Sync {
  /// Every agent with its row and roads, and the roles.
  fn agents(&self) -> BoxFuture<'_, Answer<Agents>>;

  /// Whether a stand-in answers an agent while the rest closes its road.
  fn stand_in<'a>(&'a self, agent: &'a str) -> BoxFuture<'a, Answer<bool>>;

  /// The deployment's rest: whether it stands, until when, and why.
  fn rest(&self) -> BoxFuture<'_, Answer<Rest>>;

  /// A rest written down; whether it is the one that silenced the deployment.
  fn write_rest<'a>(&'a self, resting: &'a Resting) -> BoxFuture<'a, Answer<Written>>;

  /// Why the deployment rested where its rest has just passed.
  fn passed_rest(&self) -> BoxFuture<'_, Answer<Passed>>;

  /// The session kept for the agent at the scope, started where none is.
  fn start<'a>(&'a self, agent: &'a str, scope: Scope) -> BoxFuture<'a, Answer<Session>>;

  /// One turn carried into its session until it ends. A fault of the session's comes back as the agency's
  /// answer, its outcome code the failure's code (`session-busy`, `unknown-session` and the rest).
  fn turn<'a>(&'a self, turn: &'a Turn) -> BoxFuture<'a, Answer<Delivery>>;

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

  /// The moment an agent's last reset cut it off at, in epoch milliseconds; none where it was never reset. A
  /// turn begun at or before it is overtaken (the agency's `docs/companions.md`, 「What the reset cuts off」).
  fn reset_point<'a>(&'a self, agent: &'a str) -> BoxFuture<'a, Answer<Option<i64>>>;

  /// A run's row by its id; none where the agency holds none.
  fn run(&self, id: i64) -> BoxFuture<'_, Answer<Option<Run>>>;

  /// The ids of the runs a round's turns opened, in the order the agency answers them, newest first.
  fn round_runs<'a>(&'a self, round: &'a str) -> BoxFuture<'a, Answer<Vec<i64>>>;

  /// The message a run's answer went out as, attached to the run; its closing stays as it stands.
  fn attach_message(&self, run: i64, message: i64) -> BoxFuture<'_, Answer<()>>;

  /// The documents a session was handed in full, each at the version it read.
  fn loads<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<Vec<Load>>>;

  /// A session's loads kept, every one it now holds in place of those it held.
  fn keep_loads<'a>(&'a self, session: &'a str, loads: &'a [Load]) -> BoxFuture<'a, Answer<()>>;

  fn forget_loads<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<()>>;

  /// Words put into the turn the session is working, answered with what became of them.
  fn push<'a>(&'a self, session: &'a str, turn: i64, words: &'a str) -> BoxFuture<'a, Answer<Said>>;

  /// An added agent's row stood up, or the field the agency refused.
  fn add_console_agent<'a>(
    &'a self,
    agent: &'a ConsoleAgentRow,
  ) -> BoxFuture<'a, Answer<Result<(), FieldRefusal>>>;

  /// An added agent's half taken out with everything of its own, or the field the agency refused.
  fn remove_console_agent<'a>(&'a self, agent: &'a str) -> BoxFuture<'a, Answer<Result<(), FieldRefusal>>>;

  /// The stream of the events of the kinds named, from the moment it is opened.
  fn events<'a>(&'a self, kinds: &'a [&'a str]) -> BoxFuture<'a, Answer<Events>>;
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

/// The body that writes a rest down or clears the rests that have passed.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RestAsk<'a> {
  clear_passed: bool,
  until: Option<i64>,
  kind: Option<&'a str>,
  account: Option<&'a str>,
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

/// The fields an agent's own route answers that the rest of its row does not carry.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentFields {
  #[serde(default)]
  reset_at: Option<i64>,
  #[serde(default)]
  stand_in_available: bool,
}

impl Agency for HttpAgency {
  fn agents(&self) -> BoxFuture<'_, Answer<Agents>> {
    Box::pin(self.caller.get(&["agents"], &[]))
  }

  fn stand_in<'a>(&'a self, agent: &'a str) -> BoxFuture<'a, Answer<bool>> {
    Box::pin(async move {
      let read: AgentFields = self.caller.get(&["agents", agent], &[]).await?;

      Ok(read.stand_in_available)
    })
  }

  fn rest(&self) -> BoxFuture<'_, Answer<Rest>> {
    Box::pin(self.caller.get(&["rest"], &[]))
  }

  fn write_rest<'a>(&'a self, resting: &'a Resting) -> BoxFuture<'a, Answer<Written>> {
    Box::pin(async move {
      let ask = RestAsk {
        clear_passed: false,
        until: Some(resting.until),
        kind: Some(resting.kind.word()),
        account: resting.account.as_deref(),
      };

      self.caller.patch(&["rest"], &ask).await
    })
  }

  fn passed_rest(&self) -> BoxFuture<'_, Answer<Passed>> {
    Box::pin(async move {
      let ask = RestAsk {
        clear_passed: true,
        until: None,
        kind: None,
        account: None,
      };

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

  fn reset_point<'a>(&'a self, agent: &'a str) -> BoxFuture<'a, Answer<Option<i64>>> {
    Box::pin(async move {
      let read: AgentFields = self.caller.get(&["agents", agent], &[]).await?;

      Ok(read.reset_at)
    })
  }

  fn run(&self, id: i64) -> BoxFuture<'_, Answer<Option<Run>>> {
    Box::pin(async move { self.caller.found(&["runs", &id.to_string()], &[]).await })
  }

  fn round_runs<'a>(&'a self, round: &'a str) -> BoxFuture<'a, Answer<Vec<i64>>> {
    Box::pin(async move {
      let page: RunPage = self.caller.get(&["runs"], &[("round", round.to_owned())]).await?;

      Ok(page.items.into_iter().map(|run| run.id).collect())
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

  fn add_console_agent<'a>(
    &'a self,
    agent: &'a ConsoleAgentRow,
  ) -> BoxFuture<'a, Answer<Result<(), FieldRefusal>>> {
    Box::pin(async move {
      // The agency's resource keeps its own name for the agents the console adds.
      let done: Result<Done, FieldRefusal> = self.caller.checked(&["companions"], agent).await?;

      Ok(done.map(|_| ()))
    })
  }

  fn remove_console_agent<'a>(&'a self, agent: &'a str) -> BoxFuture<'a, Answer<Result<(), FieldRefusal>>> {
    Box::pin(async move { self.caller.checked_delete(&["companions", agent]).await })
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
              let data = match sse.data.as_deref() {
                Some(data) => match serde_json::from_str::<Value>(data) {
                  Ok(data) => body_of(data, &kind),
                  Err(why) => {
                    return Some(Err(PortFailure::Undecodable {
                      url,
                      why: format!("the {kind} event's data: {why}"),
                    }));
                  }
                },
                None => Value::Null,
              };

              Some(Ok(Event { kind, data }))
            }
          }
        }
      });

      Ok(events.boxed())
    })
  }
}

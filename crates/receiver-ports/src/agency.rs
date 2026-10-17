//! The agency port: the agents and their roads, the sessions a turn is carried in, a room forgotten, the hooks
//! the receiver answers, a run's log and the event stream (`docs/boundary-api.md`, 「The agency」; the agency's
//! `docs/api.md`). Bodies are camelCase.

use futures_util::StreamExt;
use futures_util::future::BoxFuture;
use futures_util::stream::BoxStream;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sse_stream::SseStream;

use crate::http::{Answer, Caller, Done, Peer, PortFailure};

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

/// Every agent, and every role an agent may hold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Agents {
  pub agents: Vec<Agent>,
  #[serde(default)]
  pub roles: Vec<Value>,
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
}

/// A session: its id, its agent and its scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
  pub id: String,
  pub agent: String,
  pub scope: Scope,
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
  pub session: String,
  pub prompt: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub room: Option<TurnRoom>,
  /// The road the turn answers its room on, where that is not the room's own.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub via: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub base_persona: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub privileged: Option<bool>,
  /// The messages the turn answers.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub answers: Vec<i64>,
  /// When the turn took its words, in epoch milliseconds.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub began: Option<i64>,
}

/// Whether the program took a turn's words or refused them while it waits on a person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Delivery {
  Taken,
  Blocked,
}

/// A session's state: the provider and model it stands on, its place, its generation and the context its last
/// request carried, as the agency gives them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
}

/// A table's rows that stayed with the agency as a room was forgotten, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stayed {
  pub table: String,
  pub rows: i64,
  pub why: String,
}

/// What forgetting a room took, table by table, or would take in a dry run, and what stayed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// Whether a stand-in answers an agent while the rest closes the agent's road.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Answers {
  pub answers: bool,
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

  fn interrupt<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<()>>;

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
struct Start<'a> {
  agent: &'a str,
  scope: Scope,
}

#[derive(Deserialize)]
struct Delivered {
  delivery: Delivery,
}

#[derive(Serialize)]
struct Named<'a> {
  session: &'a str,
}

#[derive(Deserialize)]
struct Sessions {
  sessions: Vec<Session>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Forget {
  chat_id: i64,
  dry_run: bool,
}

#[derive(Serialize)]
struct Hook<'a> {
  point: &'a str,
  url: &'a str,
  token: &'a str,
}

#[derive(Deserialize)]
struct Lines {
  lines: Vec<RunLine>,
}

impl HttpAgency {
  async fn operated(&self, operation: &str, session: &str) -> Answer<()> {
    let _: Done = self.caller.post(&["session", operation], &Named { session }).await?;

    Ok(())
  }
}

impl Agency for HttpAgency {
  fn agents(&self) -> BoxFuture<'_, Answer<Agents>> {
    Box::pin(self.caller.get(&["agents"], &[]))
  }

  fn stand_in<'a>(&'a self, agent: &'a str) -> BoxFuture<'a, Answer<bool>> {
    Box::pin(async move {
      let query = vec![("agent", agent.to_owned())];

      let answers: Answers = self.caller.get(&["stand-in"], &query).await?;

      Ok(answers.answers)
    })
  }

  fn rest(&self) -> BoxFuture<'_, Answer<Rest>> {
    Box::pin(self.caller.get(&["rest"], &[]))
  }

  fn write_rest<'a>(&'a self, resting: &'a Resting) -> BoxFuture<'a, Answer<Written>> {
    Box::pin(self.caller.post(&["rest"], resting))
  }

  fn passed_rest(&self) -> BoxFuture<'_, Answer<Passed>> {
    Box::pin(self.caller.get(&["rest", "passed"], &[]))
  }

  fn start<'a>(&'a self, agent: &'a str, scope: Scope) -> BoxFuture<'a, Answer<Session>> {
    Box::pin(async move { self.caller.post(&["session", "start"], &Start { agent, scope }).await })
  }

  fn turn<'a>(&'a self, turn: &'a Turn) -> BoxFuture<'a, Answer<Delivery>> {
    Box::pin(async move {
      let delivered: Delivered = self.caller.post(&["session", "turn"], turn).await?;

      Ok(delivered.delivery)
    })
  }

  fn state<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<SessionState>> {
    Box::pin(
      async move { self.caller.get(&["session", "state"], &[("session", session.to_owned())]).await },
    )
  }

  fn interrupt<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<()>> {
    Box::pin(self.operated("interrupt", session))
  }

  fn fold<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<()>> {
    Box::pin(self.operated("fold", session))
  }

  fn end<'a>(&'a self, session: &'a str) -> BoxFuture<'a, Answer<()>> {
    Box::pin(self.operated("end", session))
  }

  fn sessions(&self) -> BoxFuture<'_, Answer<Vec<Session>>> {
    Box::pin(async move {
      let sessions: Sessions = self.caller.get(&["session", "list"], &[]).await?;

      Ok(sessions.sessions)
    })
  }

  fn forget_room(&self, chat_id: i64, dry_run: bool) -> BoxFuture<'_, Answer<Forgotten>> {
    Box::pin(async move { self.caller.post(&["rooms", "forget"], &Forget { chat_id, dry_run }).await })
  }

  fn register_hook<'a>(
    &'a self,
    point: &'a str,
    url: &'a str,
    token: &'a str,
  ) -> BoxFuture<'a, Answer<()>> {
    Box::pin(async move { self.caller.done(Method::POST, &["hooks"], &Hook { point, url, token }).await })
  }

  fn run_log<'a>(&'a self, run: &'a str, after: Option<i64>) -> BoxFuture<'a, Answer<Vec<RunLine>>> {
    Box::pin(async move {
      let mut query = vec![("run", run.to_owned())];

      if let Some(after) = after {
        query.push(("after", after.to_string()));
      }

      let lines: Lines = self.caller.get(&["run-log"], &query).await?;

      Ok(lines.lines)
    })
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
                Some(data) => match serde_json::from_str(data) {
                  Ok(data) => data,
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

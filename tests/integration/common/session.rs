//! A stand-in for the agency's sessions (docs/boundary-api.md, 「The agency」): a session started for every asking,
//! its state, the turn.created and turn.ended events of every turn it carries on its event stream, with whatever
//! else a test publishes there, and an empty run log. A turn it is told to hold stands working after it started
//! until the test lets it go, so words can come into the room while it works; a turn ends answered, or as the
//! test set it to end.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use futures_util::future::BoxFuture;
use receiver_policy::chat_line::CHAT_LINE;
use receiver_policy::chat_round::CHAT_ROUND;
use receiver_policy::drafting::DRAFTING;
use receiver_ports::agency::{Delivery, Event, Events, RunLine, Scope, Session, SessionState, Turn};
use receiver_ports::http::{Answer, WireError};
use serde_json::json;
use tokio::sync::{Notify, broadcast};

/// What a turn does beside answering.
type Act = Box<dyn Fn(&Turn) + Send + Sync>;

/// The session every turn goes on: every start and turn it was asked, the answer each turn ends with, and the
/// turns it holds working.
pub struct StandInSession {
  pub id: String,
  pub answer: String,
  pub starts: Mutex<Vec<Scope>>,
  pub turns: Mutex<Vec<Turn>>,
  events: broadcast::Sender<Event>,
  // How many turns, from the first, stand working until let go; and the letting go.
  holding: Mutex<usize>,
  let_go: Notify,
  working: Mutex<bool>,
  // What every turn's turn-ended says beside its session, where a test sets it: its answer otherwise.
  ending: Mutex<Option<serde_json::Value>>,
  // The answers the turns give in their order, ahead of `answer`, where a test scripts them.
  script: Mutex<VecDeque<String>>,
  // The turn, counted from 1, that stands working until let go, where a test names one.
  holding_turn: Mutex<Option<usize>>,
  // How long each turn works before it ends.
  working_for: Mutex<Duration>,
  // Why the session's prefix changed, as its state names it, where a test sets it.
  prefix_change: Mutex<Option<String>>,
  // The generation its state names: none until a test sets one.
  generation: Mutex<serde_json::Value>,
  // The provider its state names.
  provider: Mutex<String>,
  // The tokens its state says it carries, of a window of 200,000, and whether its program carries its own fold.
  carried: Mutex<i64>,
  folds_itself: Mutex<bool>,
  // What a turn does beside answering, where a test sets it: a handoff written, as a session with a file tool
  // writes it.
  acting: Mutex<Option<Act>>,
  // The lines its runs' log holds, where a test writes some.
  pub logged: Mutex<Vec<RunLine>>,
  // Whether the situation update's light turn is answered and kept out of the test's list of prompts.
  hide_situation: Mutex<bool>,
  // Whether the review gate's turn is answered with PASS and kept out of the test's list of prompts.
  hide_review: Mutex<bool>,
  // The words a turn's prompt holds for the session to refuse it, as an agency that cannot carry it answers.
  refusing: Mutex<Option<String>>,
  // Whether a turn asked while another works is refused as the agency refuses a session in a turn.
  busy: Mutex<bool>,
}

impl StandInSession {
  pub fn new(id: &str, answer: &str) -> Self {
    StandInSession {
      id: id.to_owned(),
      answer: answer.to_owned(),
      starts: Mutex::default(),
      turns: Mutex::default(),
      events: broadcast::channel(64).0,
      holding: Mutex::new(0),
      let_go: Notify::new(),
      working: Mutex::new(false),
      ending: Mutex::default(),
      script: Mutex::default(),
      holding_turn: Mutex::default(),
      working_for: Mutex::new(Duration::ZERO),
      prefix_change: Mutex::default(),
      generation: Mutex::new(json!(null)),
      logged: Mutex::default(),
      provider: Mutex::new("provider-a".to_owned()),
      carried: Mutex::new(1000),
      folds_itself: Mutex::new(false),
      acting: Mutex::default(),
      hide_situation: Mutex::new(true),
      hide_review: Mutex::new(true),
      refusing: Mutex::default(),
      busy: Mutex::default(),
    }
  }

  /// A turn asked while another works is refused 409 `session-busy`, as the agency refuses a session in a turn.
  pub fn busy(self) -> Self {
    *self.busy.lock().unwrap() = true;
    self
  }

  /// A turn whose prompt holds `words` is refused, kept out of `prompts`.
  pub fn refusing(self, words: &str) -> Self {
    *self.refusing.lock().unwrap() = Some(words.to_owned());
    self
  }

  /// From now on every turn's turn-ended says this beside its session, and its answer where none is named.
  pub fn end_as(&self, ending: Option<serde_json::Value>) {
    *self.ending.lock().unwrap() = ending;
  }

  /// The situation update's light turn answers its default and stays out of `prompts`, so a test about the draft
  /// process reads the same turns it did.
  pub fn hiding_situations(self) -> Self {
    *self.hide_situation.lock().unwrap() = true;
    self
  }

  /// The situation update's light turn is shown and answered from the script, for a test about that turn.
  pub fn showing_situations(self) -> Self {
    *self.hide_situation.lock().unwrap() = false;
    self
  }

  /// The review gate's turn is shown and answered from the script, for a test about that gate.
  pub fn showing_reviews(self) -> Self {
    *self.hide_review.lock().unwrap() = false;
    self
  }

  /// The review gate's turn is shown when `show`, and answers PASS out of the test's list otherwise.
  pub fn hiding_reviews_or(self, show: bool) -> Self {
    *self.hide_review.lock().unwrap() = !show;
    self
  }

  /// Its state names this reason for a changed prefix from now on.
  pub fn prefix_changed(&self, why: &str) {
    *self.prefix_change.lock().unwrap() = Some(why.to_owned());
  }

  /// Its state says it is past every fold share from now on, and whether its program carries its own fold.
  pub fn full(self, folds_itself: bool) -> Self {
    *self.carried.lock().unwrap() = 190_000;
    *self.folds_itself.lock().unwrap() = folds_itself;
    self
  }

  /// Its state names this provider from now on.
  pub fn on_provider(self, provider: &str) -> Self {
    *self.provider.lock().unwrap() = provider.to_owned();
    self
  }

  /// Its state names this generation from now on, or none, on a session already standing.
  pub fn now_at(&self, generation: Option<i64>) {
    *self.generation.lock().unwrap() = generation.map_or(json!(null), |generation| json!(generation));
  }

  /// Its state names this generation from now on.
  pub fn at_generation(self, generation: i64) -> Self {
    *self.generation.lock().unwrap() = json!(generation);
    self
  }

  /// Every turn does this as it works, before it ends.
  pub fn acting(self, act: impl Fn(&Turn) + Send + Sync + 'static) -> Self {
    *self.acting.lock().unwrap() = Some(Box::new(act));
    self
  }

  /// The turns answer these in their order, then `answer`.
  pub fn answering(self, answers: &[&str]) -> Self {
    *self.script.lock().unwrap() = answers.iter().map(|answer| (*answer).to_owned()).collect();
    self
  }

  /// The turn numbered `turn`, counted from 1, stands working once it started, until `let_go`.
  pub fn holding_turn(self, turn: usize) -> Self {
    *self.holding_turn.lock().unwrap() = Some(turn);
    self
  }

  /// Every turn works this long before it ends.
  pub fn working_for(self, long: Duration) -> Self {
    *self.working_for.lock().unwrap() = long;
    self
  }

  /// The first turn stands working once it started, until `let_go`.
  pub fn holding_the_first(self) -> Self {
    *self.holding.lock().unwrap() = 1;
    self
  }

  /// Every turn ends saying `ending` beside its session, in place of its answer.
  pub fn ending_with(self, ending: serde_json::Value) -> Self {
    *self.ending.lock().unwrap() = Some(ending);
    self
  }

  /// How many streams read its events now.
  pub fn listeners(&self) -> usize {
    self.events.receiver_count()
  }

  /// The turn standing working ends.
  pub fn let_go(&self) {
    self.let_go.notify_one();
  }

  /// Whether a turn has started and not yet ended.
  pub fn working(&self) -> bool {
    *self.working.lock().unwrap()
  }

  /// Every turn's prompt, in the order they were carried.
  pub fn prompts(&self) -> Vec<String> {
    self.turns.lock().unwrap().iter().map(|turn| turn.prompt.clone()).collect()
  }

  /// The room each turn it was sent answers, in order; None for a turn naming no room.
  pub fn rooms(&self) -> Vec<Option<i64>> {
    self
      .turns
      .lock()
      .unwrap()
      .iter()
      .map(|turn| turn.room.as_ref().map(|room| room.chat_id))
      .collect()
  }

  /// An event published on its stream.
  pub fn told(&self, kind: &str, data: serde_json::Value) {
    let _ = self.events.send(Event {
      kind: kind.to_owned(),
      data,
      ..Event::default()
    });
  }

  pub fn start(&self, agent: &str, scope: Scope) -> BoxFuture<'_, Answer<Session>> {
    self.starts.lock().unwrap().push(scope);

    let session = Session {
      id: self.id.clone(),
      agent: agent.to_owned(),
      scope,
    };

    Box::pin(async move { Ok(session) })
  }

  pub fn state(&self) -> BoxFuture<'_, Answer<SessionState>> {
    let state = SessionState {
      session: self.id.clone(),
      provider: Some(self.provider.lock().unwrap().clone()),
      model: Some("model-a".to_owned()),
      place: json!(null),
      generation: self.generation.lock().unwrap().clone(),
      context: json!({ "carried": *self.carried.lock().unwrap(), "window": 200000 }),
      fold_ratio: None,
      folds_itself: *self.folds_itself.lock().unwrap(),
      supervisor: None,
      role: None,
      prefix_change_reason: self.prefix_change.lock().unwrap().clone(),
    };

    Box::pin(async move { Ok(state) })
  }

  pub fn events(&self) -> BoxFuture<'_, Answer<Events>> {
    let heard = self.events.subscribe();
    let stream = futures_util::stream::unfold(heard, |mut heard| async move {
      loop {
        match heard.recv().await {
          Ok(event) => return Some((Ok(event), heard)),
          Err(broadcast::error::RecvError::Lagged(_)) => continue,
          Err(broadcast::error::RecvError::Closed) => return None,
        }
      }
    });

    Box::pin(async move { Ok(Box::pin(stream) as Events) })
  }

  pub fn turn<'a>(&'a self, turn: &'a Turn) -> BoxFuture<'a, Answer<Delivery>> {
    Box::pin(async move {
      if *self.busy.lock().unwrap() && *self.working.lock().unwrap() {
        return Err(receiver_ports::PortFailure::Answered {
          url: format!("http://agency.invalid/sessions/{}/turns", self.id),
          status: 409,
          answer: Some(WireError {
            code: "session-busy".to_owned(),
            error: format!("{} is in a turn", self.id),
          }),
        });
      }

      if self
        .refusing
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|words| turn.prompt.contains(words.as_str()))
      {
        return Err(receiver_ports::PortFailure::Answered {
          url: format!("http://agency.invalid/sessions/{}/turns", self.id),
          status: 503,
          answer: None,
        });
      }

      let hidden_situation =
        *self.hide_situation.lock().unwrap() && turn.prompt.contains(&CHAT_ROUND.situation.previous);
      let hidden_review =
        *self.hide_review.lock().unwrap() && turn.prompt.contains(&DRAFTING.review.marker);
      let hidden = hidden_situation || hidden_review;
      let number = {
        let mut turns = self.turns.lock().unwrap();

        if hidden {
          turns.len()
        } else {
          turns.push(turn.clone());
          turns.len()
        }
      };
      let holds = if hidden {
        false
      } else {
        let mut holding = self.holding.lock().unwrap();
        let holds = *holding > 0 || *self.holding_turn.lock().unwrap() == Some(number);

        *holding = holding.saturating_sub(1);
        holds
      };
      let working_for = *self.working_for.lock().unwrap();
      let answer = if hidden_review {
        DRAFTING.review.pass.clone()
      } else if hidden_situation {
        bmo_receiver::chat::sentinels::SKIP.to_owned()
      } else {
        self.script.lock().unwrap().pop_front().unwrap_or_else(|| self.answer.clone())
      };

      *self.working.lock().unwrap() = true;
      self.told(
        "turn.created",
        json!({ "session": self.id, "run": format!("run-{number}"), "turnId": number, "at": 0 }),
      );

      if holds {
        self.let_go.notified().await;
      }

      if !working_for.is_zero() {
        tokio::time::sleep(working_for).await;
      }

      if let Some(act) = self.acting.lock().unwrap().as_ref() {
        act(turn);
      }

      let mut ended = self
        .ending
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| json!({ "outcome": "answered", "answer": answer }));

      ended["session"] = json!(self.id);
      *self.working.lock().unwrap() = false;
      self.told("turn.ended", ended);

      Ok(Delivery::Delivered)
    })
  }

  pub fn run_log(&self) -> BoxFuture<'_, Answer<Vec<RunLine>>> {
    let logged = self.logged.lock().unwrap().clone();

    Box::pin(async move { Ok(logged) })
  }
}

/// The draft process every turn reads, set as the console sets a row of any provider, tier and agent: the
/// questions of an ordinary and a light turn, the light bound and the ceiling, past which `final` is asked at once.
pub async fn drafting(
  db: &sea_orm::DatabaseConnection,
  rounds: &[&str],
  light: &[&str],
  ceiling_seconds: u32,
) {
  drafting_past(db, rounds, light, ceiling_seconds, bmo_receiver::chat::draft::table::PastCeiling::Send)
    .await;
}

/// The draft process every turn reads, with what is done past its ceiling.
pub async fn drafting_past(
  db: &sea_orm::DatabaseConnection,
  rounds: &[&str],
  light: &[&str],
  ceiling_seconds: u32,
  past_ceiling: bmo_receiver::chat::draft::table::PastCeiling,
) {
  use bmo_receiver::chat::draft::table::{ANY, Process, set};

  let named = |list: &[&str]| list.iter().map(|name| (*name).to_owned()).collect();

  set(
    db,
    &Process {
      provider: ANY.to_owned(),
      tier: ANY.to_owned(),
      agent: ANY.to_owned(),
      rounds: named(rounds),
      send_from: String::new(),
      light: named(light),
      off: vec![],
      light_chars: 12,
      ceiling_seconds,
      past_ceiling,
      set: true,
    },
  )
  .await
  .unwrap();
}

/// No question follows a turn: its own answer is the message, as a test of something else needs it.
pub async fn no_drafting(db: &sea_orm::DatabaseConnection) {
  drafting(db, &[], &[], 150).await;
}

/// The prompt documents a room's block is composed from, stood under `data` so a turn can name its room.
pub fn prompt_documents(data: &Path) {
  let chat = data.join("prompts").join("chat");

  std::fs::create_dir_all(&chat).unwrap();
  std::fs::write(chat.join(&CHAT_LINE.room.owner_document), "The owner's own chat.").unwrap();
  std::fs::write(chat.join(&CHAT_LINE.room.group_document), "A group room.").unwrap();
  std::fs::write(chat.join(&CHAT_LINE.capabilities.document), "What the agent can do.").unwrap();
}

/// What a session with a file tool does with the handoff instruction: `note` written to the file it names.
pub fn writing_handoff(note: &'static str) -> impl Fn(&Turn) + Send + Sync + 'static {
  move |turn: &Turn| {
    if let Some(path) = handoff_path(&turn.prompt) {
      std::fs::write(path, note).unwrap();
    }
  }
}

/// The file a handoff instruction names, where the prompt is one.
pub fn handoff_path(prompt: &str) -> Option<String> {
  prompt
    .lines()
    .filter(|line| line.contains("/handoffs/") && line.trim_end().ends_with(".md"))
    .find_map(|line| line.find('/').map(|at| line[at..].trim_end().to_owned()))
}

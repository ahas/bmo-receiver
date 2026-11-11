//! The chat receiver (docs/receiver.md): what becomes of a message whichever chat service handed it over. A
//! service hands one over with what it knows of the room, and this decides — is the room listened to, is it the
//! owner setting something, is it the agent's own — keeps it, and hands it to the wake, which decides whom it
//! wakes. Nothing here reads which service a message came from in order to decide anything.

pub mod commands;
pub mod default_answerer;
pub mod figures;
pub mod files;
pub mod handover;
pub mod hearing;
pub mod labels;
pub mod left_board;
pub mod metadata;
pub mod notices;
pub mod pictures;
pub mod pinned;
pub mod platform;
pub mod reads;
pub mod rows;
pub mod speech;
pub mod wake;

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use axum::{body::Body, response::Response};
use futures_util::future::BoxFuture;
use receiver_codes::{RoomRemovalRefusal, SettingFact};
use receiver_policy::kept::KEPT;
use receiver_ports::agency::{Agency, Agents};
use receiver_ports::brain::BrainUnit;
use receiver_ports::gate::Gate;
use receiver_ports::hearing::Hearing;
use receiver_ports::http::{Answer, PortFailure};
use receiver_ports::job::Job;
use receiver_ports::voice::VoiceEngine;
use receiver_ports::warmer::Warmer;
use receiver_ports::web::Web;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::Fault;
use crate::chat::conversation::{Talking, Touching, Typing};
use crate::chat::talk::{Standing, TalkState};
use crate::data_root::DataRoot;
use crate::kept::Kept;
use crate::now_ms;
use crate::roads::{Road, Roads};
use crate::room_record::{Facing, RoomHolding, Says};
use commands::{CommandKind, OwnerCommand, parse_owner_command};
use speech::{NotSaid, Speech};

/// A message as the receiver keeps it and hands it on: the row the database keeps, the road it came in on and
/// the account that read it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Inbound {
  pub chat_id: i64,
  pub message_id: i64,
  pub sender_id: Option<i64>,
  pub sender_name: String,
  pub sent_at: i64,
  pub text: String,
  pub image_path: Option<String>,
  /// `stored` or `failed` for a picture the message carried, where its keeping was tried; none for no picture.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub image_state: Option<String>,
  /// What bmo-ear heard in a voice note, once the recording stands.
  pub transcript: Option<String>,
  /// How long the recording ran, where bmo-ear heard it.
  pub voice_seconds: Option<f64>,
  /// `heard` or `failed`, for a voice note whose recording was heard; none for a message that carries no voice.
  pub voice_state: Option<String>,
  pub reply_to_message_id: Option<i64>,
  /// The key of the road that took it in. Kept and handed on; never compared to a literal.
  pub via: String,
  /// The read mode the road read it by.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub mode: Option<String>,
  /// Whether it was read through somebody else's account, once the wake has judged it.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub via_reader: Option<bool>,
  /// The send of several it went out in, named by the number of the send's first message; none for a message
  /// sent alone.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub album: Option<i64>,
}

/// What the platform said of a message beyond the row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Context {
  /// What the platform calls the room now.
  pub room_title: Option<String>,
  pub sender_username: Option<String>,
  /// Who wrote the message this one answers: whom an owner's mute aimed by answering names.
  pub reply_to_sender_id: Option<i64>,
  /// The words the person typed, where the row carries a placeholder for what they sent.
  pub spoken_text: Option<String>,
  pub carries_file: bool,
  /// Whether the chat service is still gathering the message's file, and hands it over later.
  pub gathering: bool,
}

/// What the road a message came in on says of its room.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Room {
  /// Whether the room is the owner's private chat, which is listened to without being allowed.
  pub owner_room: bool,
  /// Who the agent is in the room, where the road knows.
  pub identity: Option<i64>,
  /// Whose one-to-one with the agent the room is, by the person's sender number, where the road says it is one.
  pub private_to: Option<i64>,
}

/// Why a message could not be received.
#[derive(Debug, thiserror::Error)]
pub enum NotReceived {
  #[error(transparent)]
  Store(#[from] Fault),
  #[error(transparent)]
  Said(#[from] NotSaid),
  #[error(transparent)]
  Unit(#[from] PortFailure),
}

impl From<sea_orm::DbErr> for NotReceived {
  fn from(error: sea_orm::DbErr) -> Self {
    Self::Store(error.into())
  }
}

/// Where the room's screens hear the receiver's news, and where its live board reads what the process runs
/// (docs/receiver.md, 「The event stream」). The stream itself is the events module's; a hearer that stands
/// for it in a test answers the rest as nothing.
pub trait News: Send + Sync {
  fn message(&self, chat_id: i64, message_id: i64);

  fn window(&self, _chat_id: i64) {}

  fn window_closed(&self, _chat_id: i64, _agent: &str, _at: i64) {}

  fn room_removed(&self, _chat_id: i64) {}

  fn publish(&self, _event: &Value) {}

  fn queue_message(&self, _chat_id: i64, _message_id: i64) {}

  fn drop_queued(&self, _chat_id: i64) {}

  fn begin_fold(&self, _chat_id: i64) {}

  fn end_fold(&self, _chat_id: i64) {}

  fn forget_room(&self, _chat_id: i64) {}

  fn begin_turn(&self, _turn: &Value) -> u64 {
    0
  }

  fn move_turn(&self, _id: u64, _message_ids: &[i64]) {}

  fn end_turn(&self, _id: u64) {}

  fn picture(&self) -> Value {
    json!({ "at": now_ms() / 1000, "turns": [], "compacting": [], "queued": [] })
  }

  fn held(&self) -> Vec<Value> {
    vec![]
  }

  fn started(&self) -> i64 {
    0
  }

  fn listen(&self, _after: i64, _run: Option<&str>) -> Response {
    Response::new(Body::empty())
  }
}

/// News nobody hears, where nothing stands to hear it.
pub struct Unheard;

impl News for Unheard {
  fn message(&self, _chat_id: i64, _message_id: i64) {}
}

/// How a message came to the wake: words a room said, or an answer a process beside the receiver filed under an
/// agent's own number, which wakes no agent by its own words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum How {
  Words,
  Filed,
}

/// Whom a kept message wakes, by the one rule (docs/wake.md).
pub trait Wake: Send + Sync {
  /// A message's words, with whether the owner said them.
  fn words<'a>(
    &'a self,
    receiver: &'a Receiver,
    inbound: &'a Inbound,
    room: Room,
    owner: bool,
    how: How,
  ) -> BoxFuture<'a, Result<(), NotReceived>>;

  /// A kept message's file, once its bytes stand, with the caption it came under.
  fn file<'a>(
    &'a self,
    receiver: &'a Receiver,
    inbound: &'a Inbound,
    room: Room,
    owner: bool,
    caption: &'a str,
  ) -> BoxFuture<'a, Result<(), NotReceived>>;
}

/// The wake where none stands: a kept message wakes nobody.
pub struct Nobody;

impl Wake for Nobody {
  fn words<'a>(
    &'a self,
    _receiver: &'a Receiver,
    _inbound: &'a Inbound,
    _room: Room,
    _owner: bool,
    _how: How,
  ) -> BoxFuture<'a, Result<(), NotReceived>> {
    Box::pin(async { Ok(()) })
  }

  fn file<'a>(
    &'a self,
    _receiver: &'a Receiver,
    _inbound: &'a Inbound,
    _room: Room,
    _owner: bool,
    _caption: &'a str,
  ) -> BoxFuture<'a, Result<(), NotReceived>> {
    Box::pin(async { Ok(()) })
  }
}

/// The instruction pass: asked to hand a row over at once rather than at its next tick, and asked for the rows
/// of one agent riding a room's next round as that round starts.
pub trait Pass: Send + Sync {
  fn kick(&self);

  fn riders<'a>(&'a self, agent: &'a str, chat_id: i64) -> BoxFuture<'a, ()>;
}

/// Where a woken agent's turn goes: its conversations, which take the files leading up to the message ahead of
/// it and the message itself into the room's queue, opening the room's window where its ground does and marking
/// the message as admitted by the open window alone where that was its only ground; whether
/// anything stood to take it. And everything the agent keeps of a room, forgotten as the room's registration is
/// removed.
/// The first launches a receiver has given: the rooms launched for, and the moment the last of them was given, in
/// epoch milliseconds.
#[derive(Debug, Default)]
pub struct Launches {
  pub rooms: std::collections::HashSet<i64>,
  pub last: i64,
}

pub trait Runner: Send + Sync {
  fn hand<'a>(
    &'a self,
    agent: &'a str,
    files: &'a [Inbound],
    message: &'a Inbound,
    room: Room,
    opens_window: bool,
    window_only: bool,
  ) -> BoxFuture<'a, bool>;

  fn forget(&self, chat_id: i64) -> BoxFuture<'_, ()>;

  /// A line of the agent's in a room written as a spoken script by the agent's own session, in one light turn
  /// held behind any round of its; none where no conversations stand or the session wrote none.
  fn spoken<'a>(&'a self, _agent: &'a str, _chat_id: i64, _line: &'a str) -> BoxFuture<'a, Option<String>> {
    Box::pin(async { None })
  }

  /// The situation notes the agent owes written by its session, each in one light turn that judges no
  /// replacement (docs/drafting.md, 「The situation note」): asked before that session is asked for its handoff.
  /// Nothing is written where no conversations stand.
  fn owed_notes<'a>(&'a self, _agent: &'a str, _session: &'a str) -> BoxFuture<'a, ()> {
    Box::pin(async {})
  }

  /// Whether a round of the agent's is working in the room now, which is the one place a send of the agent's is
  /// carried rather than handed to the room's supervisor (docs/messages.md, 「Words handed to the room's
  /// supervisor」); none works where no conversations stand.
  fn in_round(&self, _agent: &str, _chat_id: i64) -> bool {
    false
  }

  /// Whether a voice briefing turn of the agent's is being carried in the room now, in which no send of the
  /// agent's into the room is carried (docs/voiced-notices.md, 「The summary of a notice turn」); none is where no
  /// conversations stand.
  fn in_briefing(&self, _agent: &str, _chat_id: i64) -> bool {
    false
  }
}

/// The conversations where none stand: no turn is taken.
pub struct NoRunner;

impl Runner for NoRunner {
  fn hand<'a>(
    &'a self,
    _agent: &'a str,
    _files: &'a [Inbound],
    message: &'a Inbound,
    _room: Room,
    _opens_window: bool,
    _window_only: bool,
  ) -> BoxFuture<'a, bool> {
    Box::pin(async {
      eprintln!(
        "chat {}: no conversations stand to take message {}",
        message.chat_id, message.message_id
      );

      false
    })
  }

  fn forget(&self, _chat_id: i64) -> BoxFuture<'_, ()> {
    Box::pin(async {})
  }
}

/// The deployment's names, as the configuration gives them (docs/boundary-api.md, 「Configuration」).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deployment {
  /// The owner, whose commands the receiver takes.
  pub owner_id: i64,
  /// The configured agent's registration name, which the default answerer setting is seeded with.
  pub agent: String,
  /// The name the configured agent's own words are filed under.
  pub sender_name: String,
  /// The words a room calls the configured agent by, left off the front of an owner's command.
  pub addresses: Vec<String>,
  /// The infrastructure room, where the configuration names one.
  pub infra_room: Option<i64>,
  /// The owner's private room, a room on the record alone, where the configuration names one.
  pub owner_room: Option<i64>,
  /// The room the system's notices to the owner go to, where the configuration names one. A briefing never
  /// follows it: a briefing goes to the owner's private room alone.
  pub notices_room: Option<i64>,
  /// The language the rooms are spoken in, which a fixed line is rendered in.
  pub language: String,
  /// The largest file a message may carry to be kept, in bytes; none where no cap is set.
  pub attachment_bytes: Option<u64>,
}

/// The units a receiver reaches: the chat services through their roads, the gate, the web unit, the agency whose
/// sessions carry a woken turn, bmo-job whose questions the owner answers in his chat, and the cache warmer the
/// default answerer's one session prefix is kept warm through.
#[derive(Clone)]
pub struct Units {
  pub roads: Roads,
  pub agency: Arc<dyn Agency>,
  pub gate: Arc<dyn Gate>,
  pub job: Arc<dyn Job>,
  pub web: Arc<dyn Web>,
  pub warmer: Arc<dyn Warmer>,
  /// The voice engine, where the configuration names one: a deployment without it speaks no briefing.
  pub voice: Option<Arc<dyn VoiceEngine>>,
  /// The listening tool, where the tool host stands: a deployment without it hears no voice note.
  pub hearing: Option<Arc<dyn Hearing>>,
  /// The brain, where the deployment names it: a deployment without it tells no turn of a due consolidation and
  /// reports no fold.
  pub brain: Option<Arc<dyn BrainUnit>>,
}

/// The receiver: the database it keeps messages in, the deployment it answers for, the data root it keeps files
/// under, the units it reaches and the agency's list of agents kept for a moment, where its news is heard, whom a
/// message wakes, the instruction pass its acknowledgements are handed over by, and the conversations a woken
/// turn goes into.
#[derive(Clone)]
pub struct Receiver {
  db: DatabaseConnection,
  data_root: DataRoot,
  deployment: Arc<Deployment>,
  default_answerer: default_answerer::DefaultAnswerer,
  units: Units,
  agents: Kept<(), Agents>,
  speech: Speech,
  news: Arc<dyn News>,
  wake: Arc<dyn Wake>,
  pass: Arc<OnceLock<Arc<dyn Pass>>>,
  runner: Arc<OnceLock<Arc<dyn Runner>>>,
  talking: Arc<Standing>,
  typing: Arc<OnceLock<Arc<dyn Typing>>>,
  touching: Arc<OnceLock<Arc<dyn Touching>>>,
  place: Arc<OnceLock<Arc<str>>>,
  // The rooms this receiver has launched for and the moment the last first launch was given, by which a room's
  // first launch is spaced from another room's (docs/bmo-window.md, 「Quiet instruction」).
  launches: Arc<tokio::sync::Mutex<Launches>>,
  // The agents whose one session is being replaced now, each with the rooms told so during this replacement
  // (docs/unified-bmo.md, 「Replacing the session」).
  replacing: Arc<std::sync::Mutex<std::collections::HashMap<String, std::collections::HashSet<i64>>>>,
  // Whether every round with words first updates the room's situation note: on for the service, off where a test
  // stands the receiver without that turn.
  situation_updates: bool,
  // Whether the draft process's review gate stands: on for the service, off where a test stands the receiver
  // without that turn.
  review_gate: bool,
}

// The infrastructure room the configuration names: the board for the machines, which hears none of the agent's own
// news.
fn infra_refusal<'a>(facing: &'a Facing<'a>) -> Says<'a, RoomRemovalRefusal> {
  Box::pin(async move {
    Ok(
      (facing.receiver.deployment().infra_room == Some(facing.room))
        .then_some(RoomRemovalRefusal::Infrastructure),
    )
  })
}

crate::room_holding!(RoomHolding::Refuses {
  what: "the board for the machines",
  refuses: infra_refusal,
});

impl Receiver {
  /// A receiver on its database and data root, for its deployment, reaching its units; its news heard nowhere,
  /// its messages waking nobody and no conversations standing to take a turn until a hearer, a wake and a
  /// runner are given.
  pub fn new(db: DatabaseConnection, data_root: DataRoot, deployment: Deployment, units: Units) -> Self {
    let deployment = Arc::new(deployment);
    let default_answerer = default_answerer::DefaultAnswerer::configured(&deployment.agent);
    let news: Arc<dyn News> = Arc::new(Unheard);

    units.roads.with_store(db.clone());

    units.roads.with_owner_room(deployment.owner_room.unwrap_or(deployment.owner_id));

    Self {
      speech: Speech::new(
        db.clone(),
        deployment.clone(),
        default_answerer.clone(),
        units.clone(),
        news.clone(),
      ),
      db,
      data_root,
      deployment,
      default_answerer,
      units,
      agents: Kept::new(Duration::from_secs(KEPT.agents_seconds)),
      news,
      wake: Arc::new(Nobody),
      pass: Arc::default(),
      runner: Arc::default(),
      talking: Arc::default(),
      typing: Arc::default(),
      touching: Arc::default(),
      place: Arc::default(),
      launches: Arc::default(),
      replacing: Arc::default(),
      situation_updates: true,
      review_gate: true,
    }
  }

  /// The rooms this receiver has launched for and the moment the last first launch was given.
  pub fn launches(&self) -> &tokio::sync::Mutex<Launches> {
    &self.launches
  }

  /// The agent's one session marked as being replaced now: a room whose words arrive meanwhile is told once.
  pub fn replacing_begins(&self, agent: &str) {
    self
      .replacing
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
      .insert(agent.to_owned(), Default::default());
  }

  /// The replacement over: the mark and the rooms told go.
  pub fn replacing_ends(&self, agent: &str) {
    self.replacing.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(agent);
  }

  /// Whether the agent's session is being replaced now and this room has not yet been told; telling it is the
  /// caller's, and the room counts as told from here on.
  pub fn replacing_untold(&self, agent: &str, chat_id: i64) -> bool {
    let mut held = self.replacing.lock().unwrap_or_else(std::sync::PoisonError::into_inner);

    match held.get_mut(agent) {
      Some(told) => told.insert(chat_id),
      None => false,
    }
  }

  /// The same receiver, with the situation update turn on or off.
  pub fn with_situation_updates(mut self, on: bool) -> Self {
    self.situation_updates = on;
    self
  }

  /// Whether a round with words first updates the room's situation note.
  pub fn situation_updates(&self) -> bool {
    self.situation_updates
  }

  /// The same receiver, with the draft process's review gate on or off.
  pub fn with_review_gate(mut self, on: bool) -> Self {
    self.review_gate = on;
    self
  }

  /// Whether the draft process's review gate stands.
  pub fn review_gate(&self) -> bool {
    self.review_gate
  }

  /// The conversations that answer a line typed into the default answerer's program, and that take the warmer's touch
  /// of a quiet room's session; the first given stands, for every clone alike.
  pub fn hold_typing(self, typing: Arc<dyn Typing>) {
    let _ = self.typing.set(typing);
  }

  pub fn hold_touching(self, touching: Arc<dyn Touching>) {
    let _ = self.touching.set(touching);
  }

  /// The conversations that answer the typed-line hook, where any stand.
  pub fn typing(&self) -> Option<Arc<dyn Typing>> {
    self.typing.get().cloned()
  }

  /// The conversations that take the warmer's touch, where any stand.
  pub fn touching(&self) -> Option<Arc<dyn Touching>> {
    self.touching.get().cloned()
  }

  /// Where the receiver's talks stand: no talk begun, and nothing taking their turns until a taker is held.
  pub fn talking(&self) -> &Standing {
    &self.talking
  }

  /// The conversations that take a talk's turns from now on; the first given stands, for every clone alike.
  pub fn hold_talks(self, taker: Arc<dyn Talking>) -> Self {
    self.talking.hold(taker);
    self
  }

  /// What takes a talk's turns, where any stand.
  pub fn talks(&self) -> Option<Arc<dyn Talking>> {
    self.talking.taker()
  }

  /// Where one talk stands: whether a turn is being answered, and why the last went unanswered.
  pub fn talk_state(&self, key: &str) -> TalkState {
    self.talking.state(key)
  }

  /// The receiver telling its news to this hearer, its own lines' too.
  pub fn with_news(mut self, news: Arc<dyn News>) -> Self {
    self.speech = Speech::new(
      self.db.clone(),
      self.deployment.clone(),
      self.default_answerer.clone(),
      self.units.clone(),
      news.clone(),
    );
    self.news = news;
    self
  }

  /// The receiver handing what it kept to this wake.
  pub fn with_wake(mut self, wake: Arc<dyn Wake>) -> Self {
    self.wake = wake;
    self
  }

  /// The receiver kicking this pass as it queues a row; the first given stands, for every clone alike.
  pub fn hold_pass(&self, pass: Arc<dyn Pass>) {
    let _ = self.pass.set(pass);
  }

  /// The receiver handing a woken turn to these conversations; the first given stands, for every clone alike.
  pub fn hold_runner(&self, runner: Arc<dyn Runner>) {
    let _ = self.runner.set(runner);
  }

  /// The conversations a woken turn goes into, where any stand.
  pub fn runner(&self) -> Option<Arc<dyn Runner>> {
    self.runner.get().cloned()
  }

  /// The instruction pass the receiver's acknowledgements are queued for, where one stands.
  pub fn pass(&self) -> Option<Arc<dyn Pass>> {
    self.pass.get().cloned()
  }

  /// The database's place, as the configuration gives it: a room's record is read and taken through the any
  /// driver, which opens a database by place rather than through the pool.
  pub fn hold_place(&self, place: Arc<str>) {
    let _ = self.place.set(place);
  }

  pub fn place(&self) -> Option<Arc<str>> {
    self.place.get().cloned()
  }

  pub fn db(&self) -> &DatabaseConnection {
    &self.db
  }

  pub fn data_root(&self) -> &DataRoot {
    &self.data_root
  }

  pub fn deployment(&self) -> &Deployment {
    &self.deployment
  }

  /// The supervisor the default answerer setting names now: the one that answers every room naming none
  /// (docs/api.md, 「Supervisors」).
  pub fn default_answerer(&self) -> String {
    self.default_answerer.name()
  }

  /// The default answerer setting as the store holds it, read into the process.
  pub async fn load_default_answerer(&self) -> Result<(), Fault> {
    self.default_answerer.load(&self.db).await
  }

  /// The default answerer setting moved to the supervisor named.
  pub async fn set_default_answerer(&self, name: &str) -> Result<(), Fault> {
    self.default_answerer.set(&self.db, name).await
  }

  /// The owner as a sender: his number on each road. A room is never read off it.
  pub fn owner_id(&self) -> i64 {
    self.deployment.owner_id
  }

  /// The owner's private room (docs/receiver.md, 「Identity, and reaching the owner」): the configuration's
  /// `owner_room` where it names one, else the chat service's one-to-one with him, which that service numbers as
  /// his own number.
  pub fn owner_room(&self) -> i64 {
    self.deployment.owner_room.unwrap_or(self.deployment.owner_id)
  }

  /// Whether the configuration names the owner's private room, so a chat service's `kind` of `owner` is not read.
  pub fn owner_room_named(&self) -> bool {
    self.deployment.owner_room.is_some()
  }

  /// The owner's notices room (docs/boundary-api.md, 「Configuration」): the room the system's notices to the
  /// owner go to, the tool host's changes and a rest's and a fallback's among them, but never a briefing's; his
  /// private room where the configuration names none. A briefing goes to [`Self::owner_room`] alone.
  pub fn notices_room(&self) -> i64 {
    self.deployment.notices_room.unwrap_or_else(|| self.owner_room())
  }

  /// The name the owner goes by in a room: the one the room's own service last showed him by there, never one
  /// his console rows carry; else his registered name, the user the account model links his number to; and his
  /// number where neither stands (docs/unified-bmo.md, 「The cases」).
  pub async fn owner_name(&self, chat_id: i64) -> String {
    let owner = self.owner_id();

    if let Some(shown) = rows::shown_name(&self.db, chat_id, owner, Road::Record.key()).await.ok().flatten()
    {
      return shown;
    }

    let registered = self.units.web.user_of_sender(owner).await.ok().flatten();

    registered.filter(|name| !name.trim().is_empty()).unwrap_or_else(|| owner.to_string())
  }

  pub fn roads(&self) -> &Roads {
    &self.units.roads
  }

  pub fn units(&self) -> &Units {
    &self.units
  }

  /// The receiver keeping the agency's list of agents for `span` rather than the span the policy sets; a span of
  /// zero asks the agency on every read.
  pub fn keeping_agents_for(mut self, span: Duration) -> Self {
    self.agents = Kept::new(span);
    self
  }

  /// Every agent the agency registers, awake or not, as the agency last answered within the span, and asked of it
  /// where it has not (docs/receiver.md, 「What is kept for a moment」).
  pub async fn agents(&self) -> Answer<Agents> {
    if let Some(kept) = self.agents.get(&()) {
      return Ok(kept);
    }

    let listed = self.units.agency.supervisors().await?;

    self.agents.keep((), listed.clone());

    Ok(listed)
  }

  /// The kept list of agents let go, so the next read asks the agency: a registration the receiver changed itself.
  pub fn forget_agents(&self) {
    self.agents.forget(|_| true);
  }

  /// The lines the receiver says itself in a room.
  pub fn speech(&self) -> &Speech {
    &self.speech
  }

  /// Where the receiver's news is heard.
  pub fn news(&self) -> &dyn News {
    self.news.as_ref()
  }

  // Where a message is written down and the room's screen told of it, which every road's message passes through
  // alike.
  async fn keep(&self, inbound: &Inbound, context: &Context) -> Result<(), NotReceived> {
    if let Some(title) = &context.room_title {
      rows::set_chat_title(&self.db, inbound.chat_id, title).await?;
    }

    rows::save_message(&self.db, inbound, None, None).await?;
    self.told_kept(inbound);

    Ok(())
  }

  /// The room's screen told of a message kept or written again.
  pub fn told_kept(&self, inbound: &Inbound) {
    self.news.message(inbound.chat_id, inbound.message_id);
  }

  // The agent's own words are kept and go no further, whichever road handed them over: a road that let one
  // through would have the agent woken by itself.
  fn is_agents_own(inbound: &Inbound, room: Room) -> bool {
    inbound.sender_id.is_some() && inbound.sender_id == room.identity
  }

  // Whether the room is listened to: the owner's private chat on every road, and every room the database can
  // number.
  fn listened(chat_id: i64, room: Room) -> bool {
    room.owner_room || rows::is_chat_allowed(chat_id)
  }

  /// Words that arrived in a room.
  pub async fn message(&self, inbound: &Inbound, context: &Context, room: Room) -> Result<(), NotReceived> {
    let chat_id = inbound.chat_id;
    let said = context.spoken_text.as_deref().unwrap_or(&inbound.text);
    let is_owner = inbound.sender_id == Some(self.owner_id());
    // A command is words the owner typed and nothing else; a caption under a file the owner sent is not one.
    let command = if is_owner && !context.carries_file {
      parse_owner_command(said, &self.deployment.addresses)
    } else {
      None
    };

    if command.is_some_and(|command| command.kind == CommandKind::Allow) {
      rows::allow_chat(&self.db, chat_id).await?;

      return self.acknowledge(chat_id, SettingFact::ChatAllowed).await;
    }

    if !Self::listened(chat_id, room) {
      println!("bmo-receiver: a message from the room {chat_id}, which is not allowed, was not kept");

      return Ok(());
    }

    if command.is_some_and(|command| command.kind == CommandKind::Deny) {
      rows::deny_chat(&self.db, chat_id).await?;
      // Said as fixed: the room is being shut, and its session is not woken to say so.
      self.speech.denied(chat_id).await?;

      return Ok(());
    }

    self.keep(inbound, context).await?;

    if Self::is_agents_own(inbound, room) || rows::is_read(&self.db, chat_id, inbound.message_id).await? {
      return Ok(());
    }

    if is_owner && let Some(command) = command {
      return self.command(inbound, context, room, command).await;
    }

    self.wake.words(self, inbound, room, is_owner, How::Words).await
  }

  // One of the owner's setting commands, answered by the room's own session once the setting has changed.
  // Allowing and shutting a room are settled before the room's words are kept.
  async fn command(
    &self,
    inbound: &Inbound,
    context: &Context,
    room: Room,
    command: OwnerCommand,
  ) -> Result<(), NotReceived> {
    let chat_id = inbound.chat_id;
    let db = &self.db;
    let fact = match command.kind {
      CommandKind::Allow | CommandKind::Deny => return Ok(()),
      // Looking at pictures is switched on for one room at a time, because what it costs falls on the room that
      // has the pictures.
      CommandKind::PhotosOn | CommandKind::PhotosOff => {
        let on = command.kind == CommandKind::PhotosOn;

        rows::set_photo_look(db, chat_id, on).await?;
        SettingFact::PhotosSwitched { on }
      }
      CommandKind::NoticeRestartOn | CommandKind::NoticeRestartOff => {
        let on = command.kind == CommandKind::NoticeRestartOn;

        rows::set_notice(db, chat_id, "restart", on).await?;
        SettingFact::RestartNoticeSwitched { on }
      }
      CommandKind::Mute | CommandKind::Unmute => {
        let mute = command.kind == CommandKind::Mute;
        let target = context.reply_to_sender_id.or(command.explicit_target_id);

        match target {
          Some(target) if !mute || (target != self.owner_id() && Some(target) != room.identity) => {
            rows::set_muted(db, chat_id, target, mute).await?;
            SettingFact::MemberMuted { muted: mute }
          }
          _ => SettingFact::MuteTargetMissing,
        }
      }
    };

    self.acknowledge(chat_id, fact).await
  }

  // The setting's new state, told to the room's own session to say in its own voice.
  async fn acknowledge(&self, chat_id: i64, fact: SettingFact) -> Result<(), NotReceived> {
    notices::queue(&self.db, &self.default_answerer(), chat_id, &fact).await?;

    // The acknowledgement goes out at once rather than at the pass's next tick.
    if let Some(pass) = self.pass.get() {
      pass.kick();
    }

    Ok(())
  }

  /// A past message read back from a room, kept for the rooms the receiver listens to and waking nobody.
  pub async fn history(&self, inbound: &Inbound, context: &Context, room: Room) -> Result<(), NotReceived> {
    if !Self::listened(inbound.chat_id, room) {
      return Ok(());
    }

    self.keep(inbound, context).await
  }

  /// A message carrying a file, kept: the files it came with kept beside it and woken on, unless it is the
  /// agent's own; a file the chat service is still gathering is woken on once it is handed over.
  pub async fn attachment(
    &self,
    inbound: &Inbound,
    context: &Context,
    room: Room,
    handed: &[bmo_chat_client::MessageFile],
  ) -> Result<(), NotReceived> {
    if !Self::listened(inbound.chat_id, room) {
      return Ok(());
    }

    self.keep(inbound, context).await?;

    if Self::is_agents_own(inbound, room) {
      return Ok(());
    }

    files::keep(&self.db, &self.data_root, self.deployment.attachment_bytes, inbound, handed).await?;
    hearing::hear(self, inbound, handed).await?;

    // The files and the hearing have written their paths and their transcript onto the row: the turn reads that
    // row, not the hand-over that came before either.
    let kept = rows::message(&self.db, inbound.chat_id, inbound.message_id, &inbound.via)
      .await?
      .unwrap_or_else(|| inbound.clone());

    if context.gathering && handed.is_empty() {
      return Ok(());
    }

    let caption = context.spoken_text.as_deref().unwrap_or_default();

    self
      .wake
      .file(self, &kept, room, kept.sender_id == Some(self.owner_id()), caption)
      .await
  }

  /// A kept message's file gathered: whose file it is decides whether anybody is woken. A message the room does
  /// not hold wakes nobody.
  pub async fn gathered(
    &self,
    chat_id: i64,
    message_id: i64,
    via: &str,
    patch: &bmo_chat_client::GatheredPatch,
  ) -> Result<(), NotReceived> {
    let Some(inbound) = rows::message(&self.db, chat_id, message_id, via).await? else {
      return Ok(());
    };

    if let Some(reason) = &patch.gather_failure {
      eprintln!("bmo-receiver: the file of message {message_id} in room {chat_id} was not kept: {reason}");
    } else {
      files::keep(&self.db, &self.data_root, self.deployment.attachment_bytes, &inbound, &patch.files)
        .await?;
    }

    let room = self.roads().room_facts(chat_id).await;

    if Self::is_agents_own(&inbound, room) {
      return Ok(());
    }

    hearing::hear(self, &inbound, &patch.files).await?;

    // The kept file and its hearing stand on the row: the turn reads that row, not the message as it came.
    let kept = rows::message(&self.db, chat_id, message_id, via)
      .await?
      .unwrap_or_else(|| inbound.clone());
    let owner = kept.sender_id == Some(self.owner_id());

    self.wake.file(self, &kept, room, owner, &kept.text).await
  }

  /// An answer a process beside the receiver filed in a room under an agent's own number: it wakes no agent by its
  /// own words, and any other agent of the room by the rule every message is judged by. A row that names no road
  /// of its own is read as having come in on `via`.
  pub async fn filed(
    &self,
    chat_id: i64,
    message_id: i64,
    via: &str,
    room: Room,
  ) -> Result<(), NotReceived> {
    let Some(filed) = rows::message(&self.db, chat_id, message_id, via).await? else {
      return Ok(());
    };
    let owner = filed.sender_id == Some(self.owner_id());

    self.wake.words(self, &filed, room, owner, How::Filed).await
  }

  /// A message already kept, said again with other words. It wakes nobody; a screen showing the room reads the row
  /// again.
  pub async fn edited(&self, inbound: &Inbound) -> Result<(), NotReceived> {
    if rows::update_message_text(&self.db, inbound.chat_id, inbound.message_id, &inbound.text).await? {
      self.told_kept(inbound);
    }

    Ok(())
  }
}

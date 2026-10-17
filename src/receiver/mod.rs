//! The chat receiver (docs/receiver.md): what becomes of a message whichever chat service handed it over. A
//! service hands one over with what it knows of the room, and this decides — is the room listened to, is it the
//! owner setting something, is it the agent's own — keeps it, and hands it to the wake, which decides whom it
//! wakes. Nothing here reads which service a message came from in order to decide anything.

pub mod commands;
pub mod files;
pub mod handover;
pub mod left_board;
pub mod notices;
pub mod rows;
pub mod speech;

use std::sync::{Arc, OnceLock};

use futures_util::future::BoxFuture;
use receiver_codes::{RoomRemovalRefusal, SettingFact};
use receiver_ports::gate::Gate;
use receiver_ports::web::Web;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};

use crate::Fault;
use crate::data_root::DataRoot;
use crate::roads::Roads;
use crate::room_record::{Facing, RoomHolding, Says};
use commands::{CommandKind, OwnerCommand, parse_mode_command, parse_owner_command};
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
}

impl From<sea_orm::DbErr> for NotReceived {
  fn from(error: sea_orm::DbErr) -> Self {
    Self::Store(error.into())
  }
}

/// Where the room's screens hear the receiver's news: a message it kept or wrote again.
pub trait News: Send + Sync {
  fn message(&self, chat_id: i64, message_id: i64);
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

/// The instruction pass, asked to hand a row over at once rather than at its next tick.
pub trait Pass: Send + Sync {
  fn kick(&self);
}

/// The deployment's names, as the configuration gives them (docs/boundary-api.md, 「Configuration」).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deployment {
  /// The owner, whose commands the receiver takes.
  pub owner_id: i64,
  /// The chat agent's registration name, whose session takes the acknowledgements.
  pub agent: String,
  /// The name the chat agent's own words are filed under.
  pub sender_name: String,
  /// The words a room calls the chat agent by, left off the front of an owner's command.
  pub addresses: Vec<String>,
  /// The delegate's registered label, which a delegation notice names.
  pub delegate: Option<String>,
  /// The infrastructure room, where the configuration names one.
  pub infra_room: Option<i64>,
  /// The language the rooms are spoken in, which a fixed line is rendered in.
  pub language: String,
}

/// The units a receiver reaches: the chat services through their roads, the gate and the web unit.
#[derive(Clone)]
pub struct Units {
  pub roads: Roads,
  pub gate: Arc<dyn Gate>,
  pub web: Arc<dyn Web>,
}

/// The receiver: the database it keeps messages in, the deployment it answers for, the data root it keeps files
/// under, the roads its lines go out on, where its news is heard, whom a message wakes, and the instruction pass
/// its acknowledgements are handed over by.
#[derive(Clone)]
pub struct Receiver {
  db: DatabaseConnection,
  data_root: DataRoot,
  deployment: Arc<Deployment>,
  units: Units,
  speech: Speech,
  news: Arc<dyn News>,
  wake: Arc<dyn Wake>,
  pass: Arc<OnceLock<Arc<dyn Pass>>>,
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
  /// A receiver on its database and data root, for its deployment, reaching its units; its news heard nowhere and
  /// its messages waking nobody until a hearer and a wake are given.
  pub fn new(db: DatabaseConnection, data_root: DataRoot, deployment: Deployment, units: Units) -> Self {
    let deployment = Arc::new(deployment);
    let news: Arc<dyn News> = Arc::new(Unheard);

    Self {
      speech: Speech::new(db.clone(), deployment.clone(), units.clone(), news.clone()),
      db,
      data_root,
      deployment,
      units,
      news,
      wake: Arc::new(Nobody),
      pass: Arc::default(),
    }
  }

  /// The receiver telling its news to this hearer, its own lines' too.
  pub fn with_news(mut self, news: Arc<dyn News>) -> Self {
    self.speech = Speech::new(self.db.clone(), self.deployment.clone(), self.units.clone(), news.clone());
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

  pub fn db(&self) -> &DatabaseConnection {
    &self.db
  }

  pub fn data_root(&self) -> &DataRoot {
    &self.data_root
  }

  pub fn deployment(&self) -> &Deployment {
    &self.deployment
  }

  pub fn owner_id(&self) -> i64 {
    self.deployment.owner_id
  }

  pub fn roads(&self) -> &Roads {
    &self.units.roads
  }

  pub fn units(&self) -> &Units {
    &self.units
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

    rows::save_message(&self.db, inbound).await?;
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

    if is_owner {
      if let Some(command) = command {
        return self.command(inbound, context, room, command).await;
      }

      if let Some(mode) = parse_mode_command(said) {
        rows::set_mode(&self.db, mode).await?;

        return self
          .acknowledge(
            chat_id,
            SettingFact::ModeSet {
              everyone: mode.everyone(),
            },
          )
          .await;
      }
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
      CommandKind::ToolsOn | CommandKind::ToolsOff => {
        let on = command.kind == CommandKind::ToolsOn;

        rows::set_tools_allowed(db, chat_id, on).await?;
        SettingFact::ToolsSwitched { on }
      }
      // Looking at pictures is switched on for one room at a time, because what it costs falls on the room that
      // has the pictures.
      CommandKind::PhotosOn | CommandKind::PhotosOff => {
        let on = command.kind == CommandKind::PhotosOn;

        rows::set_photo_look(db, chat_id, on).await?;
        SettingFact::PhotosSwitched { on }
      }
      CommandKind::DelegateOn | CommandKind::DelegateOff => {
        let on = command.kind == CommandKind::DelegateOn;

        rows::set_delegation(db, chat_id, on).await?;
        SettingFact::DelegationSwitched {
          on,
          delegate: self.deployment.delegate.clone(),
        }
      }
      CommandKind::NoticeRestOn | CommandKind::NoticeRestOff => {
        let on = command.kind == CommandKind::NoticeRestOn;

        // The owner's own chat always hears the rest notice: it is how the owner learns why the agent has gone
        // quiet everywhere else.
        if !on && chat_id == self.owner_id() {
          SettingFact::RestNoticeKept
        } else {
          rows::set_notice(db, chat_id, "rest", on).await?;
          SettingFact::RestNoticeSwitched { on }
        }
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
    notices::queue(&self.db, &self.deployment.agent, chat_id, &fact).await?;

    // The acknowledgement goes out at once rather than at the pass's next tick.
    if let Some(pass) = self.pass.get() {
      pass.kick();
    }

    Ok(())
  }

  /// A message carrying a file, kept: the files it came with kept beside it and woken on, unless it is the
  /// agent's own; a file the chat service is still gathering is woken on once it is handed over.
  pub async fn attachment(
    &self,
    inbound: &Inbound,
    context: &Context,
    room: Room,
    handed: &[bmo_chat_client::FileItem],
  ) -> Result<(), NotReceived> {
    if !Self::listened(inbound.chat_id, room) {
      return Ok(());
    }

    self.keep(inbound, context).await?;

    if Self::is_agents_own(inbound, room) {
      return Ok(());
    }

    files::keep(&self.db, &self.data_root, inbound, handed).await?;

    if context.gathering && handed.is_empty() {
      return Ok(());
    }

    let caption = context.spoken_text.as_deref().unwrap_or_default();

    self
      .wake
      .file(self, inbound, room, inbound.sender_id == Some(self.owner_id()), caption)
      .await
  }

  /// A kept message's file gathered: whose file it is decides whether anybody is woken. A message the room does
  /// not hold wakes nobody.
  pub async fn gathered(
    &self,
    chat_id: i64,
    message_id: i64,
    via: &str,
    outcome: &bmo_chat_client::GatheredOutcome,
  ) -> Result<(), NotReceived> {
    let Some(inbound) = rows::message(&self.db, chat_id, message_id, via).await? else {
      return Ok(());
    };

    match outcome {
      bmo_chat_client::GatheredOutcome::Kept { files } => {
        files::keep(&self.db, &self.data_root, &inbound, files).await?;
      }
      bmo_chat_client::GatheredOutcome::NotKept { reason } => {
        eprintln!(
          "bmo-receiver: the file of message {message_id} in room {chat_id} was not kept: {reason}"
        );
      }
    }

    let room = self.roads().room_facts(chat_id).await;

    if Self::is_agents_own(&inbound, room) {
      return Ok(());
    }

    let owner = inbound.sender_id == Some(self.owner_id());

    self.wake.file(self, &inbound, room, owner, &inbound.text).await
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

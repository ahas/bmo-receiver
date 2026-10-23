//! What a send of the receiver's command line, and a send asked of the running receiver, stands on (docs/messages.md,
//! 「What an agent can send back」): the road the room is answered on, asked before anything is sent — the road
//! its caller's turn is read on where that road answers in the room, the room's own otherwise — the agent whose
//! turn asks it, who sends only into a room it answers and only where its role gives sending, and whether the
//! road performs the verb at all, as its chat service declares. A refusal is the road's one sentence, and nothing
//! is sent. Every send goes through bmo-chat-client to the chat service the room's road names, or is kept in the
//! room's own record where the road is that one.

use bmo_chat_client::{ChatCapability, ChatSend, ChatSendRequest, ChatSent, ClientFailure, MissedPicture};

use receiver_codes::SendRefusal;
use receiver_policy::fill_marks;
use receiver_policy::roads::ROADS;
use receiver_policy::roles::ROLES;

use sea_orm::DatabaseConnection;

use crate::Fault;
use std::sync::Arc;

use crate::receiver::{Receiver, speech, wake};
use crate::roads::providers::{self, Carried, Delivery};
use crate::roads::{Road, added, record};
use crate::verbs::{Launch, NoLaunch, VerbError};

/// The road a send goes out on, and who it goes out as there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Going {
  pub road: Road,
  pub speaker: Option<Speaker>,
}

/// Who a send goes out as where it is not the road's own voice: an agent, by its name and the name its words
/// stand under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Speaker {
  pub agent: String,
  pub name: String,
}

/// What a send came to: the number of the message it went out as, where the road gives one, and the pictures of
/// an album it would not carry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sent {
  pub message_id: Option<i64>,
  pub missed: Vec<MissedPicture>,
}

/// Why a send did not go: the road's own refusal in one sentence, or the road failing while it sent.
#[derive(Debug, thiserror::Error)]
pub enum NotSent {
  #[error("{0}")]
  Refused(String),
  #[error("{0}")]
  Failed(String),
}

/// What sending stands on: the receiver's own lines, and the agent whose turn asks with the road the caller was
/// told the turn is read on.
#[derive(Clone)]
pub struct Sends {
  receiver: Receiver,
  turn_agent: Option<String>,
  via: Option<String>,
  launch: Arc<dyn Launch>,
}

impl Sends {
  pub fn new(receiver: Receiver, turn_agent: Option<String>, via: Option<String>) -> Self {
    Self {
      receiver,
      turn_agent,
      via,
      launch: Arc::new(NoLaunch),
    }
  }

  /// The sends standing on this launcher of a verb's own runs, which opens the row, starts the process and
  /// closes the row; with none standing a verb that says a line of its own is refused.
  pub fn with_launch(mut self, launch: Arc<dyn Launch>) -> Self {
    self.launch = launch;
    self
  }

  pub fn launch(&self) -> Arc<dyn Launch> {
    self.launch.clone()
  }

  pub fn db(&self) -> DatabaseConnection {
    self.receiver.db().clone()
  }

  /// The receiver these sends stand on.
  pub fn receiver(&self) -> Receiver {
    self.receiver.clone()
  }

  /// The receiver's own lines, which a line nobody asked for goes through.
  pub fn speech(&self) -> speech::Speech {
    self.receiver.speech().clone()
  }

  /// The agent whose turn asks the sends, none for a send no agent's turn makes.
  pub fn turn_agent(&self) -> Option<&str> {
    self.turn_agent.as_deref()
  }

  /// The road a room's send of a verb goes out on — the one its turn answers on, given the road the caller
  /// named — speaking as the agent whose turn it is, or the one sentence saying why it cannot go there.
  pub async fn road(&self, chat_id: i64, verb: &str) -> Result<Going, VerbError> {
    let Some(road) = self.sending(chat_id, verb).await? else {
      return Err(VerbError::Refused(no_road(chat_id)));
    };

    Ok(Going {
      road,
      speaker: self.sender(chat_id).await?,
    })
  }

  /// The road a room's send of a verb goes out on in the road's own voice, whoever's turn it is: a line that
  /// stands without any agent's turn, such as one said straight to the owner.
  pub async fn own_road(&self, chat_id: i64, verb: &str) -> Result<Going, VerbError> {
    let Some(road) = self.sending(chat_id, verb).await? else {
      return Err(VerbError::Refused(no_road(chat_id)));
    };

    Ok(Going {
      road,
      speaker: None,
    })
  }

  /// The room's own road for a send of a verb in the road's own voice, whatever road the caller named: where a
  /// line the room's record could not take must still reach the room.
  pub async fn room_road(&self, chat_id: i64, verb: &str) -> Result<Going, VerbError> {
    let Some(road) = self.receiver.roads().answering(chat_id, None).await else {
      return Err(VerbError::Refused(no_road(chat_id)));
    };
    let facts = self.receiver.roads().facts(&road).await;

    if !facts.can(verb) {
      return Err(VerbError::Refused(verb_refusal(&facts.label, verb)));
    }

    Ok(Going {
      road,
      speaker: None,
    })
  }

  /// The verbs the road that answers in a room performs, which says what a document read there may teach.
  pub async fn answering_can(&self, chat_id: i64) -> Result<Vec<String>, VerbError> {
    let Some(road) = self.receiver.roads().sending(chat_id, self.via.as_deref()).await else {
      return Err(VerbError::Refused(no_road(chat_id)));
    };

    Ok(self.receiver.roads().facts(&road).await.can.clone())
  }

  /// One send carried on its road: a chat service's own lines, or kept in the room's record.
  pub async fn send(&self, going: &Going, request: &ChatSendRequest) -> Result<Sent, NotSent> {
    match going.road {
      Road::Record => self.kept(going, request).await,
      Road::Service(ref key) => self.carried(key, going, request).await,
    }
  }

  // The road a send of a verb goes out on, refused where none performs the verb.
  async fn sending(&self, chat_id: i64, verb: &str) -> Result<Option<Road>, VerbError> {
    let road = self.receiver.roads().sending(chat_id, self.via.as_deref()).await;

    let Some(road) = road else { return Ok(None) };
    let facts = self.receiver.roads().facts(&road).await;

    if !facts.can(verb) {
      return Err(VerbError::Refused(verb_refusal(&facts.label, verb)));
    }

    Ok(Some(road))
  }

  // Who a send goes out as, or why it may not go: an agent sends only into a room it answers, and in its own
  // turn only where its role gives sending. The default answerer is the road's own voice and names no speaker.
  async fn sender(&self, chat_id: i64) -> Result<Option<Speaker>, VerbError> {
    let default = wake::agents::default_designated(&self.receiver).await.ok();
    let default = default
      .map(|designated| designated.name)
      .unwrap_or_else(|| self.receiver.deployment().agent.clone());
    let agent = self.turn_agent.clone().unwrap_or_else(|| default.clone());
    let named = wake::agents::named_designated(&self.receiver, chat_id).await.ok();
    let answers = match named.as_deref() {
      Some([]) => agent == default,
      Some(named) => named.iter().any(|one| one.name == agent),
      None => agent == default,
    };
    let name = self.label(&agent).await;

    if !answers {
      return Err(VerbError::Refused(SendRefusal::AnotherAgentsRoom { agent: name }.to_string()));
    }

    if self.turn_agent.is_some() && !ROLES.sends(&self.role(&agent).await) {
      return Err(VerbError::Refused(SendRefusal::RoleWithoutSends { agent: name }.to_string()));
    }

    Ok((agent != default).then_some(Speaker { agent, name }))
  }

  // The line kept in the room's record for a send the record road carries.
  async fn kept(&self, going: &Going, request: &ChatSendRequest) -> Result<Sent, NotSent> {
    let text = kept_line(&request.send);
    let name = match &going.speaker {
      Some(speaker) => speaker.name.clone(),
      None => self.receiver.deployment().sender_name.clone(),
    };
    let sender_id = match &going.speaker {
      Some(speaker) => record::agent_number(self.receiver.db(), Road::Record.key(), &speaker.agent)
        .await
        .ok()
        .flatten(),
      None => self.receiver.roads().identity_in(request.room_id).await,
    };
    let line = record::Kept {
      chat_id: request.room_id,
      text: &text,
      reply_to: request.reply_to_message_id,
      via: Road::Record.key(),
      sender_id,
      name: &name,
    };
    let message_id = record::keep(self.receiver.db(), self.receiver.news(), line).await.map_err(kept)?;
    let delivery = Delivery {
      chat_id: request.room_id,
      message_id,
      reply_to: request.reply_to_message_id,
      owner: false,
      standing: vec![Road::Record.key().to_owned()],
      carried: Carried::Send(request.send.clone()),
    };

    providers::deliver(self.receiver.db(), self.receiver.roads(), &delivery).await;

    Ok(Sent {
      message_id: Some(message_id),
      missed: vec![],
    })
  }

  async fn carried(&self, key: &str, going: &Going, asked: &ChatSendRequest) -> Result<Sent, NotSent> {
    let mode = match asked.mode {
      Some(mode) => Some(mode),
      None => added::voice_in(self.receiver.db(), asked.room_id).await,
    };
    let request = ChatSendRequest {
      room_id: asked.room_id,
      send: asked.send.clone(),
      mode,
      speaker: going.speaker.as_ref().map(|speaker| speaker.agent.clone()),
      tag: asked.tag.clone(),
      prefix: asked.prefix.clone(),
      reply_to_message_id: asked.reply_to_message_id,
    };
    let service = self.receiver.roads().chats().road(key).ok_or_else(|| NotSent::Failed(unlisted(key)));
    let result = service?.send(&request).await;

    match result {
      Ok(ChatSent { message_id, missed }) => Ok(Sent { message_id, missed }),
      Err(ClientFailure::Answered {
        answer: Some(answer),
        ..
      }) => Err(NotSent::Refused(answer.error)),
      Err(why) => Err(NotSent::Failed(failed(why))),
    }
  }

  // The name an agent's words stand under: its registration label, or its name where it gives none.
  async fn label(&self, agent: &str) -> String {
    let listed = self.receiver.agents().await.ok();

    listed
      .as_ref()
      .and_then(|listed| {
        listed.agents.iter().find(|one| one.name == agent).and_then(|one| one.label.clone())
      })
      .unwrap_or(agent.to_string())
  }

  // The role an agent's registration names, empty where it names none.
  async fn role(&self, agent: &str) -> String {
    let listed = self.receiver.agents().await.ok();

    listed
      .as_ref()
      .and_then(|listed| {
        listed
          .agents
          .iter()
          .find(|one| one.name == agent)
          .and_then(|one| one.row.get("role").and_then(|held| held.as_str().map(str::to_owned)))
      })
      .unwrap_or_default()
  }
}

/// Whether an agent answers in a room: a room that names it among its awake answerers, and for the default
/// answerer every room that names nobody.
pub async fn answers(receiver: &Receiver, agent: &str, chat_id: i64) -> bool {
  let default = wake::agents::default_designated(receiver).await.ok();
  let named = wake::agents::named_designated(receiver, chat_id).await.ok();

  match named.as_deref() {
    Some([]) => default.as_ref().is_some_and(|default| default.name == agent),
    Some(named) => named.iter().any(|one| one.name == agent),
    None => default.as_ref().is_some_and(|default| default.name == agent),
  }
}

/// The one sentence a road refuses a verb with, naming both what was asked and the road.
pub fn verb_refusal(label: &str, verb: &str) -> String {
  SendRefusal::VerbRefused {
    road: label.to_string(),
    verb: capability(verb),
  }
  .to_string()
}

/// Pictures of which none went, said with each one's address and why.
pub fn pictures_refusal(label: &str, missed: &[MissedPicture]) -> String {
  std::iter::once(
    SendRefusal::NoPicture {
      road: label.to_string(),
    }
    .to_string(),
  )
  .chain(missed.iter().map(|missed| format!("{}: {}", missed.address, missed.reason)))
  .collect::<Vec<_>>()
  .join("\n")
}

/// A verb as the chat protocol names it, for the sentence refusing one a road does not perform.
fn capability(verb: &str) -> ChatCapability {
  serde_json::from_value(serde_json::json!(verb)).unwrap_or(ChatCapability::Text)
}

/// What a send's words are, as the room's record keeps them: a line as it stands, a picture or an album under
/// its caption, a file, a sound and a voice under the marks the policy keeps them with.
fn kept_line(send: &ChatSend) -> String {
  match send {
    ChatSend::Text { text, .. } => text.clone(),
    ChatSend::Photo { caption, .. } | ChatSend::Album { caption, .. } => {
      caption.clone().unwrap_or_default()
    }
    ChatSend::Document { file_name, .. } => fill_marks(&ROADS.record.document_kept, &[("name", file_name)]),
    ChatSend::Audio { file_name, .. } => fill_marks(&ROADS.record.audio_kept, &[("name", file_name)]),
    ChatSend::Voice { said, .. } => fill_marks(&ROADS.record.voice_kept, &[("said", said)]),
    ChatSend::Sticker { emoji, .. } => emoji.clone(),
    ChatSend::Edit { text, .. } => text.clone(),
    ChatSend::Reaction { .. }
    | ChatSend::Pin { .. }
    | ChatSend::Typing { .. }
    | ChatSend::Delete { .. } => String::new(),
  }
}

// Why a send was asked of a room no road holds.
fn no_road(chat_id: i64) -> String {
  format!("no road holds the room {chat_id}, so nothing was said there")
}

// Why a line kept in the room's record did not stand.
fn kept(why: Fault) -> NotSent {
  NotSent::Failed(why.detail)
}

// A road named that the configured chat services do not hold.
fn unlisted(key: &str) -> String {
  format!("the {key} road is not among the configured chat services")
}

// A chat service that did not carry the send.
fn failed(why: ClientFailure) -> String {
  why.to_string()
}

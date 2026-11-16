//! What a send of the receiver's command line, and a send asked of the running receiver, stands on (docs/messages.md,
//! 「What an agent can send back」): the road the room is answered on, asked before anything is sent — the road
//! its caller's turn is read on where that road answers in the room, the room's own otherwise — the agent whose
//! turn asks it, who sends only into a room it answers and only where its role gives sending, and whether the
//! road performs the verb at all, as its chat service declares. A read-only room takes no send at all
//! (docs/receiver.md, 「A read-only room」). A refusal is the road's one sentence, and nothing is sent. Every send
//! goes through bmo-chat-client to the chat service the room's road names, or is kept in the room's own record
//! where the road is that one.

use bmo_chat_client::{
  AlbumItem, ChatCapability, ChatSend, ChatSendRequest, ChatSent, ChatSticker, ChatVoice, ClientFailure,
  FileBody, MessageFile, MissedPicture,
};

use receiver_codes::SendRefusal;
use receiver_policy::fill_marks;
use receiver_policy::roads::ROADS;
use receiver_policy::roles::ROLES;

use sea_orm::DatabaseConnection;

use crate::Fault;
use std::sync::Arc;
use std::time::Duration;

use crate::receiver::speech::KeptLine;
use crate::receiver::{Receiver, files, rows, speech, wake};
use crate::roads::providers::{self, Carried, Delivery, FileLimit, SaidBy};
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
  road_within: Duration,
}

impl Sends {
  pub fn new(receiver: Receiver, turn_agent: Option<String>, via: Option<String>) -> Self {
    Self {
      receiver,
      turn_agent,
      via,
      launch: Arc::new(NoLaunch),
      road_within: Duration::from_secs(ROADS.say.road_seconds),
    }
  }

  /// The sends standing on this launcher of a verb's own runs, which opens the row, starts the process and
  /// closes the row; with none standing a verb that says a line of its own says it in the receiver itself.
  pub fn with_launch(mut self, launch: Arc<dyn Launch>) -> Self {
    self.launch = launch;
    self
  }

  pub fn launch(&self) -> Arc<dyn Launch> {
    self.launch.clone()
  }

  /// The sends giving a line's road this long to say it takes the line, in place of the policy's bound.
  pub fn with_road_within(mut self, within: Duration) -> Self {
    self.road_within = within;
    self
  }

  /// The road a line goes out on, as `road` reads it, refused where the road has not answered within the bound
  /// (`[say]` of the policy's `roads.toml`), so a call that hands the line on never waits on a road that is silent.
  pub async fn line_road(&self, chat_id: i64) -> Result<Going, VerbError> {
    match tokio::time::timeout(self.road_within, self.road(chat_id, "text")).await {
      Ok(going) => going,
      Err(_) => Err(VerbError::Refused(
        SendRefusal::RoadUnanswered {
          room: chat_id,
          seconds: u64::try_from(self.road_within.as_millis().div_ceil(1000)).unwrap_or(u64::MAX),
        }
        .to_string(),
      )),
    }
  }

  pub fn db(&self) -> DatabaseConnection {
    self.receiver.db().clone()
  }

  /// The largest file a room takes, where a chat service on its list declares one (docs/api.md, 「Sends」).
  pub async fn file_limit(&self, chat_id: i64) -> Result<Option<FileLimit>, VerbError> {
    Ok(providers::file_limit(self.receiver.db(), self.receiver.roads(), chat_id).await?)
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
    self.writable(chat_id).await?;

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

  /// One send carried on its road: a chat service's own lines, or kept in the room's record. A send that puts a
  /// message in the room on a chat service's road is kept in the record too, once the road has answered.
  pub async fn send(&self, going: &Going, request: &ChatSendRequest) -> Result<Sent, NotSent> {
    match going.road {
      Road::Record => self.kept(going, request).await,
      Road::Service(ref key) => {
        let sent = self.carried(key, going, request).await?;

        self.filed(key, going, request, &sent).await;

        Ok(sent)
      }
    }
  }

  // The message a send put in the room on a chat service's road, kept in the room's record as the words of the
  // agent it went out as, under the number the road gave it (docs/messages.md, 「What an agent can send back」), and
  // carried on to the room's other providers. An act on a message already standing puts none there, and a send of
  // which nothing went out keeps nothing. The message is in the room either way, so a record that could not keep it
  // is said in the journal.
  async fn filed(&self, key: &str, going: &Going, request: &ChatSendRequest, sent: &Sent) {
    if !puts_message(&request.send) || (sent.message_id.is_none() && !sent.missed.is_empty()) {
      return;
    }

    let chat_id = request.room_id;
    let agent = going
      .speaker
      .as_ref()
      .map(|speaker| speaker.agent.clone())
      .unwrap_or_else(|| self.receiver.default_answerer());
    let voice = match request.mode {
      Some(mode) => Some(mode),
      None => added::voice_in(self.receiver.db(), self.receiver.roads(), key, chat_id).await,
    };
    let text = kept_line(&request.send);
    let line = KeptLine {
      chat_id,
      text: &text,
      reply_to: request.reply_to_message_id,
      sent: Some((key, sent.message_id)),
      agent: Some(&agent),
      voice,
    };
    let kept = self.receiver.speech().kept_carrying(&line, Carried::Send(request.send.clone())).await;

    match kept {
      Ok(message_id) => {
        if let Err(why) = self.keep_sent_files(key, request, message_id, &sent.missed).await {
          eprintln!("chat {chat_id}: a file sent on {key} was not kept: {}", why.detail);
        }

        if matches!(request.send, ChatSend::Sticker { .. })
          && let Err(why) = record::mark_sticker(self.receiver.db(), chat_id, message_id).await
        {
          eprintln!("chat {chat_id}: the sticker sent on {key} was not marked: {}", why.detail);
        }

        // The line reaches the room's other agents as every message filed in it does.
        let room = self.receiver.roads().room_facts(chat_id).await;

        if let Err(why) = self.receiver.filed(chat_id, message_id, key, room).await {
          eprintln!("chat {chat_id}: a send on {key} was not handed to the receiver: {why}");
        }
      }
      Err(why) => eprintln!("chat {chat_id}: a send on {key} was not kept: {why}"),
    }
  }

  // The files a send handed over as bytes, kept under the record row its message stands as, as the files a
  // message carried are kept (docs/room-files.md, 「Message files」): a picture in the picture store, its path on
  // the row, and any other file in the message-file store. A file the road said did not go is left out. Keeping
  // never fails a send: the file is already in the room by the time it runs.
  async fn keep_sent_files(
    &self,
    via: &str,
    request: &ChatSendRequest,
    message_id: i64,
    missed: &[MissedPicture],
  ) -> Result<(), Fault> {
    let files = sent_files(&request.send, missed);

    if files.is_empty() {
      return Ok(());
    }

    let db = self.receiver.db();
    let Some(kept) = rows::message(db, request.room_id, message_id, via).await? else {
      return Ok(());
    };

    files::keep(db, self.receiver.data_root(), &kept, &files).await
  }

  // A read-only or archived room refused before any road is chosen, so nothing is sent or kept there.
  async fn writable(&self, chat_id: i64) -> Result<(), VerbError> {
    if providers::read_only(self.receiver.db(), self.receiver.roads(), chat_id).await? {
      return Err(VerbError::Refused(SendRefusal::ReadOnlyRoom { room: chat_id }.to_string()));
    }

    if crate::roads::archived::archived(self.receiver.db(), chat_id).await?.is_some() {
      return Err(VerbError::Refused(SendRefusal::ArchivedRoom { room: chat_id }.to_string()));
    }

    Ok(())
  }

  // The road a send of a verb goes out on, refused in a read-only room and where none performs the verb.
  async fn sending(&self, chat_id: i64, verb: &str) -> Result<Option<Road>, VerbError> {
    self.writable(chat_id).await?;

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
      .unwrap_or_else(|| self.receiver.default_answerer());
    let agent = self.turn_agent.clone().unwrap_or_else(|| default.clone());
    let named = wake::agents::named_designated(&self.receiver, chat_id).await.ok();
    let answers = match named.as_deref() {
      Some([]) => {
        agent == default
          || wake::agents::private_answerer(&self.receiver, chat_id)
            .await
            .ok()
            .flatten()
            .is_some_and(|owner| owner.name == agent)
      }
      Some(named) => named.iter().any(|one| one.name == agent),
      None => agent == default,
    };
    let name = self.label(&agent).await;

    if !answers {
      return Err(VerbError::Refused(SendRefusal::AnotherAgentsRoom { agent: name }.to_string()));
    }

    if self.turn_agent.is_some() && !self.sends(&agent).await {
      return Err(VerbError::Refused(SendRefusal::RoleWithoutSends { agent: name }.to_string()));
    }

    Ok((agent != default).then_some(Speaker { agent, name }))
  }

  // The line kept in the room's record for a send the record road carries.
  async fn kept(&self, going: &Going, request: &ChatSendRequest) -> Result<Sent, NotSent> {
    // An act on a message the record keeps is kept on that message, and carried where the message stands.
    let by = going
      .speaker
      .as_ref()
      .map(|speaker| speaker.agent.clone())
      .unwrap_or_else(|| self.receiver.default_answerer());
    let acted = match &request.send {
      ChatSend::Reaction { message_id, emoji } => {
        Some((*message_id, record::Act::Reaction { by: &by, emoji }))
      }
      ChatSend::Pin { message_id, on, .. } => Some((*message_id, record::Act::Pin { on: *on })),
      ChatSend::Edit { message_id, text } => Some((*message_id, record::Act::Edit { text })),
      ChatSend::Delete { message_id } => Some((*message_id, record::Act::Delete)),
      _ => None,
    };

    if let Some((message_id, act)) = acted {
      let db = self.receiver.db();

      if !record::act(db, request.room_id, message_id, act).await.map_err(kept)? {
        return Err(NotSent::Refused(format!(
          "the record of room {} keeps no message {message_id}",
          request.room_id
        )));
      }

      self
        .receiver
        .news()
        .publish(&crate::events::message_changed(request.room_id, message_id));
      // What each provider took the act is said in the log there; the send itself went, since the record kept it.
      let _ =
        providers::carry_act(db, self.receiver.roads(), request.room_id, message_id, &request.send, None)
          .await;

      return Ok(Sent {
        message_id: Some(message_id),
        missed: vec![],
      });
    }

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
    let agent = going
      .speaker
      .as_ref()
      .map(|speaker| speaker.agent.clone())
      .unwrap_or_else(|| self.receiver.default_answerer());
    let line = record::Kept {
      chat_id: request.room_id,
      number: None,
      text: &text,
      reply_to: request.reply_to_message_id,
      via: Road::Record.key(),
      sender_id,
      name: &name,
      agent: Some(&agent),
      voice: None,
    };
    let message_id = record::keep(self.receiver.db(), self.receiver.news(), line).await.map_err(kept)?;

    if let Err(why) = self.keep_sent_files(Road::Record.key(), request, message_id, &[]).await {
      eprintln!("chat {}: a file kept in the record was not kept: {}", request.room_id, why.detail);
    }

    if matches!(request.send, ChatSend::Sticker { .. }) {
      record::mark_sticker(self.receiver.db(), request.room_id, message_id)
        .await
        .map_err(kept)?;
    }

    // The line reaches the room's other agents as every message filed in it does: a call by name or a reply to
    // one of their words wakes them there (docs/wake.md, 「Who is woken」).
    let room = self.receiver.roads().room_facts(request.room_id).await;

    if let Err(why) = self.receiver.filed(request.room_id, message_id, Road::Record.key(), room).await {
      eprintln!("chat {}: a send was not handed to the receiver: {why}", request.room_id);
    }

    let delivery = Delivery {
      chat_id: request.room_id,
      message_id,
      reply_to: request.reply_to_message_id,
      said_by: SaidBy::Room,
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
      None => added::voice_in(self.receiver.db(), self.receiver.roads(), key, asked.room_id).await,
    };

    if mode != Some(ChatVoice::HumanReader) && self.receiver.roads().reads_only(key, asked.room_id).await {
      return Err(NotSent::Refused(
        SendRefusal::ReadOnlyRoom {
          room: asked.room_id,
        }
        .to_string(),
      ));
    }

    let request = ChatSendRequest {
      room_id: asked.room_id,
      send: asked.send.clone(),
      mode,
      speaker: going.speaker.as_ref().map(|speaker| speaker.agent.clone()),
      tag: asked.tag.clone(),
      prefix: asked.prefix.clone(),
      reply_to_message_id: asked.reply_to_message_id,
      account_id: asked.account_id,
    };
    let service = self.receiver.roads().chats().road(key).ok_or_else(|| NotSent::Failed(unlisted(key)))?;
    let result = self.receiver.roads().send(service.as_ref(), &request).await;

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

  // Whether the agent gives sending into a room: every supervisor does, and a role's run where its role's
  // bundles open it; an agent that is neither sends nothing.
  async fn sends(&self, agent: &str) -> bool {
    let listed = self.receiver.agents().await.ok();

    if listed.as_ref().is_some_and(|listed| listed.agents.iter().any(|one| one.name == agent)) {
      return ROLES.supervisor_sends();
    }

    let roles = self.receiver.units().agency.roles().await.unwrap_or_default();

    roles.iter().find(|role| role.name == agent).is_some_and(|role| role.gives(&ROLES.send))
  }
}

/// Whether an agent answers in a room: a room that names it among its awake answerers; for a room that names
/// nobody, the supervisor its private person owns where one stands, and otherwise the default answerer.
pub async fn answers(receiver: &Receiver, agent: &str, chat_id: i64) -> bool {
  let default = wake::agents::default_designated(receiver).await.ok();
  let named = wake::agents::named_designated(receiver, chat_id).await.ok();

  match named.as_deref() {
    Some([]) => {
      wake::agents::private_answerer(receiver, chat_id)
        .await
        .ok()
        .flatten()
        .is_some_and(|owner| owner.name == agent)
        || default.as_ref().is_some_and(|default| default.name == agent)
    }
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

/// Whether a send puts a message of its own in the room, rather than acting on one already there.
fn puts_message(send: &ChatSend) -> bool {
  !matches!(
    send,
    ChatSend::Reaction { .. }
      | ChatSend::Pin { .. }
      | ChatSend::Typing { .. }
      | ChatSend::Edit { .. }
      | ChatSend::Delete { .. }
  )
}

/// What a send's words are, as the room's record keeps them: a line as it stands, a picture or an album under
/// its caption, a file, a sound and a voice under the marks the policy keeps them with, a sticker as its emoji or
/// a platform item under its title, else its key.
fn kept_line(send: &ChatSend) -> String {
  match send {
    ChatSend::Text { text, .. } => text.clone(),
    ChatSend::Photo { caption, .. } | ChatSend::Album { caption, .. } => {
      caption.clone().unwrap_or_default()
    }
    ChatSend::Document { file_name, .. } => fill_marks(&ROADS.record.document_kept, &[("name", file_name)]),
    ChatSend::Audio { file_name, .. } => fill_marks(&ROADS.record.audio_kept, &[("name", file_name)]),
    ChatSend::Voice { said, .. } => fill_marks(&ROADS.record.voice_kept, &[("said", said)]),
    ChatSend::Sticker(ChatSticker::Emoji { emoji }) => emoji.clone(),
    ChatSend::Sticker(ChatSticker::Item { item }) => item.title.clone().unwrap_or_else(|| item.id.clone()),
    ChatSend::Edit { text, .. } => text.clone(),
    ChatSend::Reaction { .. }
    | ChatSend::Pin { .. }
    | ChatSend::Typing { .. }
    | ChatSend::Delete { .. } => String::new(),
  }
}

// The type a voice note goes out as, which names the file it is kept under.
const VOICE_TYPE: &str = "audio/ogg";
// What a voice note is told by among its message's files.
const VOICE_ID: &str = "voice";

/// The files a send hands over as bytes, each as a file of its message: the files of an album the road took, a
/// document, a sound and a voice note. A web address is the road's to fetch, so it is no file here.
pub fn sent_files(send: &ChatSend, missed: &[MissedPicture]) -> Vec<MessageFile> {
  // A file of a send is told from the message's others by its name; a voice note, which has none, by what it is.
  let file = |name: &str, mime: String, bytes: &FileBody| MessageFile {
    id: if name.is_empty() { VOICE_ID.to_owned() } else { name.to_owned() },
    name: name.to_owned(),
    mime,
    bytes: bytes.clone(),
    kind: None,
  };

  match send {
    ChatSend::Album { items, .. } => items
      .iter()
      .filter_map(|item| match item {
        AlbumItem::File(one) if !missed.iter().any(|missed| missed.address == one.name) => {
          Some(file(&one.name, one.mime.clone(), &one.bytes))
        }
        _ => None,
      })
      .collect(),
    ChatSend::Document { bytes, file_name } => vec![file(file_name, type_of(file_name), bytes)],
    ChatSend::Audio { audio, file_name } => {
      // A sound is kept as a sound whatever its name says of it.
      let named = Some(type_of(file_name)).filter(|mime| files::kind_of(mime) == "audio");

      vec![file(file_name, named.unwrap_or_else(|| VOICE_TYPE.to_owned()), audio)]
    }
    ChatSend::Voice { audio, .. } => vec![file("", VOICE_TYPE.to_owned(), audio)],
    _ => vec![],
  }
}

// The type a file's name says it is, a bare file's where the name says nothing.
fn type_of(name: &str) -> String {
  mime_guess::from_path(name)
    .first()
    .map(|kind| kind.essence_str().to_owned())
    .unwrap_or_else(|| "application/octet-stream".to_owned())
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

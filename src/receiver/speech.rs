//! The lines the receiver says itself in a room (docs/receiver.md, 「Where it stands」): a shut room's, and any line
//! said straight to the owner. Each goes where a line nobody asked for goes in its room — the room's record where a
//! road reaching the room shows it, the room's own road elsewhere — its words read by the sanitiser first unless
//! they come already read, and handed to the room's chat service as they stand, for its dialect to carry.

use std::sync::Arc;

use bmo_chat_client::{ChatSend, ChatSendRequest, ChatSendResult, ClientFailure};
use receiver_codes::FixedLine;
use receiver_ports::PortFailure;
use receiver_ports::gate::Sanitise;
use receiver_ports::web::Render;
use sea_orm::DatabaseConnection;
use serde_json::{Map, Value};

use super::{Deployment, News, Units};
use crate::Fault;
use crate::roads::{Road, record};

/// Why a line was not said.
#[derive(Debug, thiserror::Error)]
pub enum NotSaid {
  #[error("no road holds the room {chat_id}, so nothing was said there")]
  NoRoad { chat_id: i64 },
  #[error("the {road} road is not among the configured chat services")]
  Unlisted { road: String },
  #[error("the sanitiser did not read the words: {0}")]
  Sanitiser(PortFailure),
  #[error("the {road} road did not carry the line: {why}")]
  Chat { road: String, why: ClientFailure },
  #[error("the {road} road refused the line: {refusal}")]
  Refused { road: String, refusal: String },
  #[error(transparent)]
  Store(#[from] Fault),
}

/// What saying a line stands on: the database a record keeps it in, the deployment it speaks for, the units its
/// words go through, and where a room's screen hears of a line kept.
#[derive(Clone)]
pub struct Speech {
  db: DatabaseConnection,
  deployment: Arc<Deployment>,
  units: Units,
  news: Arc<dyn News>,
}

// What the sanitiser is told a line the receiver says itself is.
const LABEL: &str = "a line the chat service is about to say in a chat room on its own account";

impl Speech {
  pub fn new(
    db: DatabaseConnection,
    deployment: Arc<Deployment>,
    units: Units,
    news: Arc<dyn News>,
  ) -> Self {
    Self {
      db,
      deployment,
      units,
      news,
    }
  }

  /// A line in a room, where a line nobody asked for goes there: its words as they stand where they come already
  /// read — a fixed line's — and read by the sanitiser otherwise. The number of its first message, where its road
  /// gives one.
  pub async fn say(&self, chat_id: i64, text: &str, cleaned: bool) -> Result<Option<i64>, NotSaid> {
    let road = self.units.roads.unasked(chat_id).await.ok_or(NotSaid::NoRoad { chat_id })?;
    let text = if cleaned { text.to_owned() } else { self.sanitised(chat_id, text).await? };

    match road {
      Road::Record => self.kept(chat_id, &text).await.map(Some),
      Road::Service(key) => self.sent(&key, chat_id, &text).await,
    }
  }

  /// A line said straight to the owner, in the owner's own chat where a line nobody asked for goes there.
  pub async fn say_to_owner(&self, text: &str) -> Result<Option<i64>, NotSaid> {
    self.say(self.deployment.owner_id, text, false).await
  }

  /// A room just shut told so, in the one fixed line.
  pub async fn denied(&self, chat_id: i64) -> Result<Option<i64>, NotSaid> {
    let line = self.rendered(&FixedLine::ChatDenied).await;

    self.say(chat_id, &line, true).await
  }

  /// A fixed line's words in the rooms' language, as the web unit's language packs render it; its code and fields
  /// where the web unit cannot be reached, so the room is never left silent.
  pub async fn rendered(&self, line: &FixedLine) -> String {
    let Ok(Value::Object(mut fields)) = serde_json::to_value(line) else {
      return line.unrendered();
    };
    let code = fields
      .remove("code")
      .and_then(|code| code.as_str().map(str::to_owned))
      .unwrap_or_default();
    let asked = Render {
      code,
      fields: fields.into_iter().collect::<Map<String, Value>>(),
      language: self.deployment.language.clone(),
    };

    match self.units.web.render(&asked).await {
      Ok(text) => text,
      Err(why) => {
        eprintln!("bmo-receiver: the {} line went unrendered: {why}", asked.code);
        line.unrendered()
      }
    }
  }

  async fn sanitised(&self, chat_id: i64, text: &str) -> Result<String, NotSaid> {
    let asked = Sanitise {
      text: text.to_owned(),
      label: LABEL.to_owned(),
      chat: Some(chat_id),
      message: None,
    };

    Ok(self.units.gate.sanitise(&asked).await.map_err(NotSaid::Sanitiser)?.text)
  }

  // The line kept in the room's record as the chat agent's own: under the number the room's road gave the agent
  // where it numbered it, and under who the agent is in the room otherwise.
  pub async fn kept(&self, chat_id: i64, text: &str) -> Result<i64, NotSaid> {
    let roads = &self.units.roads;
    let numbered = match roads.room_road(chat_id).await {
      Some(road) => record::agent_number(&self.db, &road, &self.deployment.agent).await?,
      None => None,
    };
    let sender_id = match numbered {
      Some(number) => Some(number),
      None => roads.identity_in(chat_id).await,
    };
    let kept = record::Kept {
      chat_id,
      text,
      reply_to: None,
      via: Road::Record.key(),
      sender_id,
      name: &self.deployment.sender_name,
    };

    Ok(record::keep(&self.db, self.news.as_ref(), kept).await?)
  }

  async fn sent(&self, key: &str, chat_id: i64, text: &str) -> Result<Option<i64>, NotSaid> {
    let send = ChatSend::Text {
      text: text.to_owned(),
      reply_to: None,
      silent: false,
      fold: false,
    };

    self.act(key, &ChatSendRequest::new(chat_id, send)).await
  }

  /// One send carried on a road: the number of the message it went out as, where the road gives one.
  pub async fn act(&self, key: &str, request: &ChatSendRequest) -> Result<Option<i64>, NotSaid> {
    match self.carried(key, request).await? {
      ChatSendResult::Sent { message_id, .. } => Ok(message_id),
      ChatSendResult::Refused { refusal, .. } => Err(NotSaid::Refused {
        road: key.to_owned(),
        refusal: refusal.to_string(),
      }),
    }
  }

  /// One send carried on a road, its result as the road answers it: what went and its number, or the refusal.
  pub async fn carried(&self, key: &str, request: &ChatSendRequest) -> Result<ChatSendResult, NotSaid> {
    let service = self.units.roads.chats().road(key).ok_or_else(|| NotSaid::Unlisted {
      road: key.to_owned(),
    })?;

    service.send(request).await.map_err(|why| NotSaid::Chat {
      road: key.to_owned(),
      why,
    })
  }
}

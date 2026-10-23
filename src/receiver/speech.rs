//! The lines the receiver says itself in a room (docs/receiver.md, 「Where it stands」): a shut room's, and any line
//! said straight to the owner. Each goes where a line nobody asked for goes in its room — the room's record where a
//! road reaching the room shows it, the room's own road elsewhere — its words read by the sanitiser first unless
//! they come already read, and handed to the room's chat service as they stand, for its dialect to carry. Nothing
//! is said in a read-only room (docs/receiver.md, 「A read-only room」).

use std::sync::Arc;

use bmo_chat_client::{ChatSend, ChatSendRequest, ChatSent, ChatVoice, ClientFailure};
use receiver_codes::FixedLine;
use receiver_ports::PortFailure;
use receiver_ports::gate::Sanitise;
use receiver_ports::web::Render;
use sea_orm::DatabaseConnection;
use serde_json::{Map, Value};

use super::{Deployment, News, Units};
use crate::Fault;
use crate::roads::providers::{self, Carried, Delivery};
use crate::roads::{Road, added, record};

/// Why a line was not said.
#[derive(Debug, thiserror::Error)]
pub enum NotSaid {
  #[error("no road holds the room {chat_id}, so nothing was said there")]
  NoRoad { chat_id: i64 },
  #[error("the room {chat_id} is read-only, read in the owner's account alone, so nothing was said there")]
  ReadOnly { chat_id: i64 },
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
    if providers::read_only(&self.db, &self.units.roads, chat_id).await? {
      return Err(NotSaid::ReadOnly { chat_id });
    }

    let road = self.units.roads.unasked(chat_id).await.ok_or(NotSaid::NoRoad { chat_id })?;
    let text = if cleaned { text.to_owned() } else { self.sanitised(chat_id, text).await? };

    match road {
      Road::Record => self.kept(chat_id, &text, None).await.map(Some),
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

    self.rendered_code(&code, fields).await
  }

  /// A line's words where its code and fields are the caller's, rendered in the rooms' language; its code and
  /// fields where the web unit cannot render it, so the room is never left silent.
  pub async fn rendered_code(&self, code: &str, fields: Map<String, Value>) -> String {
    let asked = Render {
      code: code.to_owned(),
      fields: fields.clone(),
      language: self.deployment.language.clone(),
    };

    match self.units.web.render(&asked).await {
      Ok(text) => text,
      Err(why) => {
        eprintln!("bmo-receiver: the {} line went unrendered: {why}", asked.code);
        let mut held = Map::new();

        held.insert("code".to_owned(), Value::String(code.to_owned()));

        for (name, value) in fields {
          if name != "code" {
            held.insert(name, value);
          }
        }

        Value::Object(held).to_string()
      }
    }
  }

  async fn sanitised(&self, chat_id: i64, text: &str) -> Result<String, NotSaid> {
    let asked = Sanitise {
      text: text.to_owned(),
      label: LABEL.to_owned(),
      room_id: Some(chat_id),
      message_id: None,
    };

    Ok(self.units.gate.sanitise(&asked).await.map_err(NotSaid::Sanitiser)?.text)
  }

  /// The line kept in the room's record as the chat agent's own: under the number the room's road gave the agent
  /// where it numbered it, and under who the agent is in the room otherwise. It goes on to the room's other
  /// providers (docs/receiver.md, 「A room's chat providers」), but for the road it was already `sent` on, under
  /// the number that road gave it.
  pub async fn kept(
    &self,
    chat_id: i64,
    text: &str,
    sent: Option<(&str, Option<i64>)>,
  ) -> Result<i64, NotSaid> {
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

    let message_id = record::keep(&self.db, self.news.as_ref(), kept).await?;
    let mut standing = vec![Road::Record.key().to_owned()];

    if let Some((road, copy)) = sent {
      providers::stood(&self.db, chat_id, message_id, road, copy).await?;
      standing.push(road.to_owned());
    }

    let delivery = Delivery {
      chat_id,
      message_id,
      reply_to: None,
      owner: false,
      standing,
      carried: Carried::words(text),
    };

    providers::deliver(&self.db, roads, &delivery).await;

    Ok(message_id)
  }

  async fn sent(&self, key: &str, chat_id: i64, text: &str) -> Result<Option<i64>, NotSaid> {
    let send = ChatSend::Text {
      text: text.to_owned(),
      silent: false,
      fold: false,
    };

    self.act(key, &ChatSendRequest::new(chat_id, send)).await
  }

  /// One send carried on a road: the number of the message it went out as, where the road gives one.
  pub async fn act(&self, key: &str, request: &ChatSendRequest) -> Result<Option<i64>, NotSaid> {
    self.carried(key, request).await.map(|sent| sent.message_id)
  }

  /// One send carried on a road, its result as the road answers it: what went and its number, or the refusal. A
  /// send that names no voice goes in the one its room answers in (docs/receiver.md, 「Adding a room」); a road
  /// that reads the room in the owner's account alone is sent nothing but in the owner's own voice.
  pub async fn carried(&self, key: &str, request: &ChatSendRequest) -> Result<ChatSent, NotSaid> {
    let service = self.units.roads.chats().road(key).ok_or_else(|| NotSaid::Unlisted {
      road: key.to_owned(),
    })?;
    let mut request = request.clone();

    if request.mode.is_none() {
      request.mode = added::voice_in(&self.db, request.room_id).await;
    }

    if request.mode != Some(ChatVoice::HumanReader)
      && self.units.roads.reads_only(key, request.room_id).await
    {
      return Err(NotSaid::ReadOnly {
        chat_id: request.room_id,
      });
    }

    match service.send(&request).await {
      Ok(sent) => Ok(sent),
      Err(ClientFailure::Answered {
        answer: Some(answer),
        ..
      }) => Err(NotSaid::Refused {
        road: key.to_owned(),
        refusal: answer.error,
      }),
      Err(why) => Err(NotSaid::Chat {
        road: key.to_owned(),
        why,
      }),
    }
  }
}

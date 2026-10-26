//! The lines the receiver says itself in a room (docs/receiver.md, 「Where it stands」): a shut room's, and any line
//! said straight to the owner. Each goes where a line nobody asked for goes in its room — the room's record where a
//! road reaching the room shows it, the room's own road elsewhere — its words handed to the room's chat service as
//! they stand, for its dialect to carry. Nothing
//! is said in a read-only room (docs/receiver.md, 「A read-only room」).

use std::sync::Arc;

use bmo_chat_client::{ChatSend, ChatSendRequest, ChatSent, ChatVoice, ClientFailure};
use receiver_codes::FixedLine;
use receiver_policy::fill_marks;
use receiver_policy::fixed_lines::FIXED_LINES;
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

  /// A line in a room, where a line nobody asked for goes there: the number of its first message, where its road
  /// gives one.
  pub async fn say(&self, chat_id: i64, text: &str) -> Result<Option<i64>, NotSaid> {
    if providers::read_only(&self.db, &self.units.roads, chat_id).await? {
      return Err(NotSaid::ReadOnly { chat_id });
    }

    let road = self.units.roads.unasked(chat_id).await.ok_or(NotSaid::NoRoad { chat_id })?;
    match road {
      Road::Record => self.kept(chat_id, text, None, None, None).await.map(Some),
      Road::Service(key) => self.sent(&key, chat_id, text).await,
    }
  }

  /// A line said straight to the owner, in the owner's own chat where a line nobody asked for goes there.
  pub async fn say_to_owner(&self, text: &str) -> Result<Option<i64>, NotSaid> {
    self.say(self.deployment.owner_room.unwrap_or(self.deployment.owner_id), text).await
  }

  /// A room just shut told so, in the one fixed line; nothing where no sentence can be made of it.
  pub async fn denied(&self, chat_id: i64) -> Result<Option<i64>, NotSaid> {
    let Some(line) = self.rendered(&FixedLine::ChatDenied).await else {
      return Ok(None);
    };

    self.say(chat_id, &line).await
  }

  /// A fixed line's words in the rooms' language, as the web unit's language packs render it, or the policy's plain
  /// sentence for it where they cannot; none where neither can, and the journal says so. A person never reads a
  /// line's code or its fields.
  pub async fn rendered(&self, line: &FixedLine) -> Option<String> {
    let Ok(Value::Object(mut fields)) = serde_json::to_value(line) else {
      eprintln!("bmo-receiver: a fixed line did not read as its fields, so nothing was said");

      return None;
    };
    let code = fields
      .remove("code")
      .and_then(|code| code.as_str().map(str::to_owned))
      .unwrap_or_default();

    self.rendered_code(&code, fields).await
  }

  /// A line's words where its code and fields are the caller's, rendered in the rooms' language, or the policy's
  /// plain sentence for the code where the web unit cannot render it; none where neither can.
  pub async fn rendered_code(&self, code: &str, fields: Map<String, Value>) -> Option<String> {
    let asked = Render {
      code: code.to_owned(),
      fields: fields.clone(),
      language: self.deployment.language.clone(),
    };

    match self.units.web.render(&asked).await {
      Ok(text) if !text.trim().is_empty() => Some(text),
      Ok(_) => plain(&self.deployment.language, code, &fields),
      Err(why) => {
        eprintln!("bmo-receiver: the {code} line went unrendered by the web unit: {why}");

        plain(&self.deployment.language, code, &fields)
      }
    }
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
    agent: Option<&str>,
    voice: Option<ChatVoice>,
  ) -> Result<i64, NotSaid> {
    self.kept_replying(chat_id, text, None, sent, agent, voice).await
  }

  /// The line kept as `kept` keeps it, as a reply to the message it is pinned to where it is pinned to one
  /// (docs/drafting.md, 「The pin stands on the room's record」).
  pub async fn kept_replying(
    &self,
    chat_id: i64,
    text: &str,
    reply_to: Option<i64>,
    sent: Option<(&str, Option<i64>)>,
    agent: Option<&str>,
    voice: Option<ChatVoice>,
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
    let voice = voice.and_then(added::voice_key);
    let kept = record::Kept {
      chat_id,
      text,
      reply_to,
      via: Road::Record.key(),
      sender_id,
      name: &self.deployment.sender_name,
      agent,
      voice: voice.as_deref(),
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
      reply_to,
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

/// A fixed line as the policy's plain sentence for its code in the rooms' language, its fields filled in as words;
/// none, with the journal told, where the policy holds no sentence for it (docs/voiced-notices.md, 「Lines that
/// stay fixed」).
pub fn plain(language: &str, code: &str, fields: &Map<String, Value>) -> Option<String> {
  let Some(sentence) = FIXED_LINES.sentence(language, code) else {
    eprintln!("bmo-receiver: the {code} line has no sentence in {language}, so nothing was said");

    return None;
  };
  // Every mark the sentence names is filled, one whose field the line does not carry with nothing, so no mark
  // stands in the room.
  let mut words: Vec<(String, String)> =
    fields.iter().map(|(name, value)| (name.clone(), worded(value))).collect();

  for named in sentence
    .split('{')
    .skip(1)
    .filter_map(|after| after.split_once('}').map(|(name, _)| name))
  {
    if !named.is_empty()
      && named.chars().all(|character| character.is_ascii_alphanumeric() || character == '_')
      && !words.iter().any(|(name, _)| name == named)
    {
      words.push((named.to_owned(), String::new()));
    }
  }

  let marks: Vec<(&str, &str)> = words.iter().map(|(name, said)| (name.as_str(), said.as_str())).collect();
  let said = fill_marks(sentence, &marks);
  let said = said.split_whitespace().collect::<Vec<_>>().join(" ");

  (!said.is_empty()).then_some(said)
}

// A field as words: text as it stands, a number as its digits, a list as its members, an object as its values
// apart from the shape it names, and nothing for null.
fn worded(value: &Value) -> String {
  match value {
    Value::Null => String::new(),
    Value::String(text) => text.clone(),
    Value::Bool(flag) => flag.to_string(),
    Value::Number(number) => number.to_string(),
    Value::Array(members) => members
      .iter()
      .map(worded)
      .filter(|said| !said.is_empty())
      .collect::<Vec<_>>()
      .join(", "),
    Value::Object(fields) => fields
      .iter()
      .filter(|(name, _)| name.as_str() != "shape")
      .map(|(_, value)| worded(value))
      .filter(|said| !said.is_empty())
      .collect::<Vec<_>>()
      .join(" · "),
  }
}

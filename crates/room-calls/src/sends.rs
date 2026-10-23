//! The sends the receiver's tools and the `bmo` commands doing the same sends ask the running receiver for
//! (docs/chat-mcp.md, 「How a call runs」): a line said in a room or in the consult room, a picture or an album,
//! a reaction, a pin, a sticker, a fair draw, a file or an album of files, a voice note and a fixed line, each
//! the arguments its tool takes, asked with who asks it — the room, the road the turn names for it, the agent
//! whose turn it is, the chat that set it off and whether a job asks — since only the running receiver carries
//! a send on its own lines (docs/messages.md, 「What an agent can send back」).

use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::bot::Service;
use crate::mode::Mode;
use crate::turn::{Room, Turn};

/// The route every send but a line in the consult room is asked of.
pub const SENDS_PATH: &str = "/sends";

/// The route a line in the consult room is asked of.
pub const CONSULT_PATH: &str = "/consult-lines";

// Long enough for an album to be drawn on and uploaded, short of the tool host's limit on a call.
const SEND_WAIT: Duration = Duration::from_secs(170);

fn off(flag: &bool) -> bool {
  !*flag
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SayInput {
  /// The room, by chat id, where it is not this turn's own.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub chat: Option<i64>,
  /// The road to send on, where the room block names the room's own road for this.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub via: Option<String>,
  /// The words.
  pub text: String,
  /// An answer is wanted back.
  #[serde(default, skip_serializing_if = "off")]
  pub reply: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConsultInput {
  /// The words.
  pub text: String,
  /// The consult room's message these words answer, by its number.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub reply: Option<i64>,
}

/// One picture of an album.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Picture {
  /// A web address or a kept file.
  pub url: String,
  /// A word drawn into that picture.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub label: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PhotoInput {
  /// The room, by chat id, where it is not this turn's own.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub chat: Option<i64>,
  /// The road to send on, where the room block names the room's own road for this.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub via: Option<String>,
  /// The pictures, in order.
  pub pictures: Vec<Picture>,
  /// The caption.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub caption: Option<String>,
}

/// One file of a file send.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FileToSend {
  /// The path of a file on this machine.
  pub path: String,
  /// The name the room reads it under, its own name where none is given.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FileInput {
  /// The room, by chat id, where it is not this turn's own.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub chat: Option<i64>,
  /// The road to send on, where the room block names the room's own road for this.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub via: Option<String>,
  /// The files, in order: one goes as a document, several as one album.
  pub files: Vec<FileToSend>,
  /// The caption.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub caption: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VoiceInput {
  /// The room, by chat id, where it is not this turn's own.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub chat: Option<i64>,
  /// The road to send on, where the room block names the room's own road for this.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub via: Option<String>,
  /// The path of an OGG/Opus file on this machine.
  pub path: String,
  /// The caption the room reads under the note.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub caption: Option<String>,
  /// What the note says, where the caption is not it.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub said: Option<String>,
  /// What was actually read out, where that differs from the caption.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub transcript: Option<String>,
}

/// A fixed line asked of a room: its code, and the fields the code carries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FixedLineInput {
  /// The room, by chat id, where it is not this turn's own.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub chat: Option<i64>,
  /// The road to send on, where the room block names the room's own road for this.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub via: Option<String>,
  /// The line's code, as the web unit's language packs name it.
  pub code: String,
  /// The fields the code carries.
  #[serde(default, skip_serializing_if = "Map::is_empty")]
  pub fields: Map<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReactInput {
  /// The room, by chat id, where it is not this turn's own.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub chat: Option<i64>,
  /// The road to send on, where the room block names the room's own road for this.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub via: Option<String>,
  /// The message.
  pub message: i64,
  /// The emoji, or empty to clear.
  pub emoji: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PinInput {
  /// The room, by chat id, where it is not this turn's own.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub chat: Option<i64>,
  /// The road to send on, where the room block names the room's own road for this.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub via: Option<String>,
  /// The message.
  pub message: i64,
  /// Unpin instead.
  #[serde(default, skip_serializing_if = "off")]
  pub unpin: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StickerInput {
  /// The room, by chat id, where it is not this turn's own.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub chat: Option<i64>,
  /// The road to send on, where the room block names the room's own road for this.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub via: Option<String>,
  /// The sticker's emoji.
  pub emoji: String,
}

/// What a draw draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
#[schemars(inline)]
pub enum DrawShape {
  Dice,
  Range,
  Pick,
  Shuffle,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RandomDrawInput {
  /// The room, by chat id, where it is not this turn's own.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub chat: Option<i64>,
  /// The road to send on, where the room block names the room's own road for this.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub via: Option<String>,
  /// sync: the result comes back in this call. async: it wakes the next turn.
  pub mode: Mode,
  /// dice: count dice of sides faces plus modifier. range: one integer in min..max. pick: take of items. shuffle: items in a fair order.
  pub shape: DrawShape,
  /// dice: how many dice; one where not given.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub count: Option<i64>,
  /// dice: faces on each die.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub sides: Option<i64>,
  /// dice: added to the total.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub modifier: Option<i64>,
  /// range: the least value, included.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub min: Option<i64>,
  /// range: the greatest value, included.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub max: Option<i64>,
  /// pick: how many items; one where not given.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub take: Option<i64>,
  /// pick, shuffle: the entries.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub items: Option<Vec<String>>,
  /// What the draw decides.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub label: Option<String>,
}

impl RandomDrawInput {
  /// The draw's own fields as the draw reads them, the room, the road and the mode left out.
  pub fn request(&self) -> Map<String, Value> {
    let mut fields = match serde_json::to_value(self) {
      Ok(Value::Object(fields)) => fields,
      _ => Map::new(),
    };

    for gone in ["chat", "via", "mode"] {
      fields.remove(gone);
    }

    fields
  }
}

/// One send asked of `POST /sends`, by its tool's name, with that tool's own arguments.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tool", content = "input", rename_all = "snake_case")]
pub enum Act {
  Say(SayInput),
  Photo(PhotoInput),
  File(FileInput),
  Voice(VoiceInput),
  React(ReactInput),
  Pin(PinInput),
  Sticker(StickerInput),
  RandomDraw(RandomDrawInput),
  FixedLine(FixedLineInput),
}

/// Who asks a send: the room it goes into and the road the turn names for it, the agent whose turn asks — none
/// for a send no agent's turn makes — the chat that set it off, and whether a job asks.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Asker {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub room: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub via: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub agent_name: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub by: Option<String>,
  #[serde(default, skip_serializing_if = "off")]
  pub job: bool,
}

impl Asker {
  /// The asker of a call made in a turn, in the room it acts in: the turn's agent, the chat it answers as the
  /// one that set the send off, and whether a job runs it.
  pub fn of_turn(turn: &Turn, room: Option<&Room>) -> Self {
    Self {
      room: room.map(|room| room.chat),
      via: room.and_then(|room| room.via.clone()),
      agent_name: turn.agent.clone(),
      by: turn.chat.or(room.map(|room| room.chat)).map(|chat| format!("chat:{chat}")),
      job: turn.job,
    }
  }
}

/// One send asked of the receiver: the act, and who asks it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asked {
  pub asker: Asker,
  #[serde(flatten)]
  pub act: Act,
}

/// Why something asked of the receiver — a send, or a run started — did not go all the way.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unsent {
  /// Nothing answered for it: no token, nothing listening, or an answer that says nothing of it.
  Unreached(String),
  /// The receiver turned it away under this status and gave no reason.
  Untaken(u16),
  /// It was refused, after the lines the receiver said of what it did first.
  Refused { lines: Vec<String>, refusal: String },
}

impl Unsent {
  /// Everything said of it, the refusal last.
  pub fn lines(self) -> Vec<String> {
    match self {
      Self::Unreached(why) => vec![why],
      Self::Untaken(status) => vec![format!("the receiver turned it away ({status}) and gave no reason")],
      Self::Refused { mut lines, refusal } => {
        lines.push(refusal);
        lines
      }
    }
  }
}

/// One send asked of the running receiver, answering the lines it said of it, or why it did not go.
pub async fn ask(receiver: &Service, asked: &Asked) -> Result<Vec<String>, Unsent> {
  let body = serde_json::to_value(asked)
    .map_err(|why| Unsent::Unreached(format!("the send could not be written: {why}")))?;
  let (status, said) =
    receiver.post_within(SENDS_PATH, &body, SEND_WAIT).await.map_err(Unsent::Unreached)?;

  answered(SENDS_PATH, status, &said)
}

/// One line said in the consult room under the agent's own name, answering the lines the receiver said of it — each
/// agent's window there — or why it did not go; a line no agent's turn asks for is refused here.
pub async fn ask_consult(
  receiver: &Service,
  agent: Option<&str>,
  input: &ConsultInput,
) -> Result<Vec<String>, Unsent> {
  let Some(agent) = agent else {
    return Err(Unsent::Refused {
      lines: Vec::new(),
      refusal: "this call was made outside any agent's turn, so there is no agent to speak as".to_owned(),
    });
  };
  let body = json!({ "agent": agent, "text": input.text, "replyTo": input.reply });
  let (status, said) =
    receiver.post_within(CONSULT_PATH, &body, SEND_WAIT).await.map_err(Unsent::Unreached)?;

  answered(CONSULT_PATH, status, &said)
}

/// What the receiver's answer to a send says of it: the lines it said, and the refusal it ended in.
pub fn answered(path: &str, status: u16, said: &Value) -> Result<Vec<String>, Unsent> {
  let text = |value: &Value| value.as_str().map(str::to_owned);
  let lines: Vec<String> = said
    .get("lines")
    .and_then(Value::as_array)
    .map(|lines| lines.iter().filter_map(text).collect())
    .unwrap_or_default();

  if !(200..300).contains(&status) {
    return Err(match said.get("error").and_then(text) {
      Some(refusal) => Unsent::Refused { lines, refusal },
      None => Unsent::Untaken(status),
    });
  }

  match said.get("refusal").and_then(text) {
    Some(refusal) => Err(Unsent::Refused { lines, refusal }),
    None if said.get("lines").is_none() => Err(Unsent::Unreached(format!(
      "the receiver's answer to {path} says nothing of what it did"
    ))),
    None => Ok(lines),
  }
}

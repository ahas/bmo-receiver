//! The sends the receiver's tools and the `bmo` commands doing the same sends ask the running receiver for
//! (docs/chat-mcp.md, 「How a call runs」): a line for a room, a spoken talk, a picture or an album, a reaction, a
//! pin, a sticker, a fair draw, a file or an album of files and a voice note, each the arguments its tool takes,
//! asked with who asks it — the room, the road the turn names for it, the agent whose turn it is, the chat that
//! set it off and whether a job asks — since only the running receiver carries a send on its own lines
//! (docs/messages.md, 「What an agent can send back」), only in the asking agent's own round and only where the
//! act holds no words: words go into a room only as a round's message, so every other send it refuses or hands to
//! the room's supervisor (docs/messages.md, 「Words handed to the room's supervisor」).

use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::bot::Service;
use crate::turn::{Room, Turn};

/// The route every send is asked of.
pub const SENDS_PATH: &str = "/sends";

/// How long the receiver is given to answer a call that hands words on or refuses them and sends nothing in the
/// call itself: a line, a spoken talk.
pub const HANDED_WITHIN: Duration = Duration::from_secs(60);

/// How long the receiver is given to answer an act it carries to the road within the call.
pub const CARRIED_WITHIN: Duration = Duration::from_secs(180);

/// How long the receiver is given to answer a send of files, whose bytes go to the road within the call.
pub const FILES_WITHIN: Duration = Duration::from_secs(600);

fn off(flag: &bool) -> bool {
  !*flag
}

// Whether a caption says anything: one of nothing but white space is no words.
fn worded(caption: Option<&str>) -> bool {
  caption.is_some_and(|caption| !caption.trim().is_empty())
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
  /// The message this send replies to, by its number in the room.
  #[serde(rename = "replyTo", default, skip_serializing_if = "Option::is_none")]
  pub reply_to: Option<i64>,
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
  /// The message this send replies to, by its number in the room.
  #[serde(rename = "replyTo", default, skip_serializing_if = "Option::is_none")]
  pub reply_to: Option<i64>,
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
  /// The message this send replies to, by its number in the room.
  #[serde(rename = "replyTo", default, skip_serializing_if = "Option::is_none")]
  pub reply_to: Option<i64>,
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
  /// The message this send replies to, by its number in the room.
  #[serde(rename = "replyTo", default, skip_serializing_if = "Option::is_none")]
  pub reply_to: Option<i64>,
}

/// A spoken talk asked of a room by its words, ahead of any synthesis: the voice it is spoken in, the written words
/// the room reads and the spoken words the voice says, where they are given.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpeakInput {
  /// The room, by chat id, where it is not this turn's own.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub chat: Option<i64>,
  /// The road to send on, where the room block names the room's own road for this.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub via: Option<String>,
  /// The trained voice the line is spoken in.
  pub voice: String,
  /// The written words: what the room reads of the note.
  pub text: String,
  /// The spoken words the voice says in their place, where they are given.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub spoken: Option<String>,
  /// The message this send replies to, by its number in the room.
  #[serde(rename = "replyTo", default, skip_serializing_if = "Option::is_none")]
  pub reply_to: Option<i64>,
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
  /// The message this send replies to, by its number in the room.
  #[serde(rename = "replyTo", default, skip_serializing_if = "Option::is_none")]
  pub reply_to: Option<i64>,
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
  /// The draw's own fields as the draw reads them, the room and the road left out.
  pub fn request(&self) -> Map<String, Value> {
    let mut fields = match serde_json::to_value(self) {
      Ok(Value::Object(fields)) => fields,
      _ => Map::new(),
    };

    for gone in ["chat", "via"] {
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
  /// A spoken talk asked by its words, which no tool entry asks: words, so it is never carried.
  Speak(SpeakInput),
}

impl Act {
  /// How long the receiver is given to answer the act before the call stops waiting: a line or a spoken talk is
  /// handed on and answered at once, the other acts go to the road within the call, and files take the longest.
  pub fn answered_within(&self) -> Duration {
    match self {
      Self::Say(_) | Self::Speak(_) => HANDED_WITHIN,
      Self::Photo(_) | Self::File(_) | Self::Voice(_) => FILES_WITHIN,
      Self::React(_) | Self::Pin(_) | Self::Sticker(_) | Self::RandomDraw(_) => CARRIED_WITHIN,
    }
  }

  /// Whether the act puts words of the asker's in the room: a line, a spoken talk, a voice note, or a picture or a
  /// file with a caption. Such an act is never carried, since words go into a room only as a round's message.
  pub fn holds_words(&self) -> bool {
    match self {
      Self::Say(_) | Self::Speak(_) | Self::Voice(_) => true,
      Self::Photo(input) => worded(input.caption.as_deref()),
      Self::File(input) => worded(input.caption.as_deref()),
      Self::React(_) | Self::Pin(_) | Self::Sticker(_) | Self::RandomDraw(_) => false,
    }
  }

  /// The tool's name the act is asked under.
  pub fn tool(&self) -> &'static str {
    match self {
      Self::Say(_) => "say",
      Self::Photo(_) => "photo",
      Self::File(_) => "file",
      Self::Voice(_) => "voice",
      Self::React(_) => "react",
      Self::Pin(_) => "pin",
      Self::Sticker(_) => "sticker",
      Self::RandomDraw(_) => "random_draw",
      Self::Speak(_) => "speak",
    }
  }
}

/// Who asks a send: the room it goes into and the road the turn names for it, the agent whose turn asks — none
/// for a send no agent's turn makes — the chat that set it off, whether a job asks, and the command or unit the
/// words come through, which words handed to the room's supervisor name.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Asker {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub room: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub via: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub agent_name: Option<String>,
  /// The moment the asking turn took its words, in epoch milliseconds, which an agent's reset point is
  /// judged by.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub began: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub by: Option<String>,
  #[serde(default, skip_serializing_if = "off")]
  pub job: bool,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub from: Option<String>,
}

impl Asker {
  /// The asker of a call made in a turn, in the room it acts in: the turn's agent and the moment it began,
  /// the chat it answers as the one that set the send off, and whether a job runs it.
  pub fn of_turn(turn: &Turn, room: Option<&Room>) -> Self {
    Self {
      room: room.map(|room| room.chat),
      via: room.and_then(|room| room.via.clone()),
      agent_name: turn.agent.clone(),
      began: turn.began,
      by: turn.chat.or(room.map(|room| room.chat)).map(|chat| format!("chat:{chat}")),
      job: turn.job,
      from: None,
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

/// One send asked of the running receiver, answering the lines it said of it, or why it did not go, within the
/// bound its act gives the receiver.
pub async fn ask(receiver: &Service, asked: &Asked) -> Result<Vec<String>, Unsent> {
  ask_within(receiver, asked, asked.act.answered_within()).await
}

/// One send asked of the running receiver, answering the lines it said of it, or why it did not go, the
/// receiver given `within` to answer.
pub async fn ask_within(
  receiver: &Service,
  asked: &Asked,
  within: Duration,
) -> Result<Vec<String>, Unsent> {
  let body = serde_json::to_value(asked)
    .map_err(|why| Unsent::Unreached(format!("the send could not be written: {why}")))?;
  let (status, said) =
    receiver.post_answered(SENDS_PATH, &body, within).await.map_err(Unsent::Unreached)?;

  answered(SENDS_PATH, status, &said)
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

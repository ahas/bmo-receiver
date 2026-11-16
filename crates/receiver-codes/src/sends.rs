//! A send refused before anything went (docs/messages.md, 「What an agent can send back」; docs/chat-mcp.md).

use bmo_chat_client::ChatCapability;
use http::StatusCode;
use strum::{IntoStaticStr, VariantNames};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error, IntoStaticStr, VariantNames)]
#[strum(serialize_all = "kebab-case")]
pub enum SendRefusal {
  /// The road the room is answered on does not perform the verb, as its chat service declares.
  #[error("the road {road} does not perform `{}`", verb_word(.verb))]
  VerbRefused { road: String, verb: ChatCapability },
  /// Another agent answers the room, and the asker does not.
  #[error("another agent answers this room, so {agent} cannot send into it")]
  AnotherAgentsRoom { agent: String },
  /// The room is read-only: it is read in the owner's account alone, and no agent sends into it.
  #[error("the room {room} is read-only, read in the owner's account alone, so nothing is sent into it")]
  ReadOnlyRoom { room: i64 },
  /// The room is archived and locked: its record stands and nothing more is said in it.
  #[error("the room {room} is archived and locked, so nothing is sent into it")]
  ArchivedRoom { room: i64 },
  /// The asker's role gives no sending.
  #[error("the role of {agent} gives no sending")]
  RoleWithoutSends { agent: String },
  /// The road refused a sticker; the list says which there are.
  #[error("{refusal} (--list shows the stickers there are)")]
  Sticker { refusal: String },
  /// Not one picture of a send went.
  #[error("{road}: not one picture went")]
  NoPicture { road: String },
  /// The road cannot send under the asking agent's name.
  #[error("{road} cannot send under the name of {agent}")]
  SpeakerRefused { road: String, agent: String },
  /// The road gave the asking agent no number, so it cannot send under that name.
  #[error("{road} gave {agent} no number, so it cannot send under that name")]
  Unnumbered { road: String, agent: String },
  /// A picture named by neither a web address nor a file of the data root.
  #[error("{url} is neither a web address nor a file of the data root")]
  NotAnAddress { url: String },
  /// A file named outside the asker's data root.
  #[error("{path} is outside the data root of {agent}, so it is not sent")]
  OutsideDataRoot { path: String, agent: String },
  /// A file of the data root that is not there.
  #[error("{path} is not a file on this machine")]
  MissingFile { path: String },
  /// A picture named by a web address that could not be fetched.
  #[error("the picture at {url} could not be fetched")]
  PictureUnfetched { url: String },
  /// A file the send went with could not be kept beside its message.
  #[error("the file {name} could not be kept")]
  FileNotKept { name: String },
  /// A file of another room's folder, which belongs to that room.
  #[error("the file belongs to the room {room}, so it is not sent here")]
  OtherRoomsFile { room: i64 },
  /// A job asked to send a room's file, which a check never takes.
  #[error("a job never sends a file a room keeps; make the material for the check yourself")]
  JobSendsRoomFile,
  /// The agent's session asked a send into a room while the receiver carries its voice briefing turn there.
  #[error(
    "this is a voice briefing turn of {agent} in the room {room}: its answer is the script itself, and nothing is \
     sent into the room from it; answer with the script alone, with no talk, send or voice note"
  )]
  BriefingTurn { agent: String, room: i64 },
  /// The road a line goes out on did not say within the bound whether it takes the line.
  #[error(
    "the road of the room {room} did not answer within {seconds} seconds, so nothing was sent; try again later"
  )]
  RoadUnanswered { room: i64, seconds: u64 },
}

impl SendRefusal {
  fn answers(&self) -> StatusCode {
    match self {
      Self::NotAnAddress { .. } | Self::MissingFile { .. } => StatusCode::BAD_REQUEST,
      Self::OutsideDataRoot { .. } => StatusCode::FORBIDDEN,
      Self::PictureUnfetched { .. } | Self::FileNotKept { .. } => StatusCode::BAD_GATEWAY,
      Self::AnotherAgentsRoom { .. }
      | Self::ReadOnlyRoom { .. }
      | Self::RoleWithoutSends { .. }
      | Self::OtherRoomsFile { .. }
      | Self::JobSendsRoomFile => StatusCode::FORBIDDEN,
      Self::VerbRefused { .. }
      | Self::Sticker { .. }
      | Self::NoPicture { .. }
      | Self::SpeakerRefused { .. }
      | Self::Unnumbered { .. }
      | Self::ArchivedRoom { .. }
      | Self::BriefingTurn { .. } => StatusCode::CONFLICT,
      Self::RoadUnanswered { .. } => StatusCode::GATEWAY_TIMEOUT,
    }
  }
}

coded!(SendRefusal);
refused!(SendRefusal);

// A capability as the chat protocol names it on the wire.
fn verb_word(verb: &ChatCapability) -> String {
  serde_json::to_value(verb)
    .ok()
    .and_then(|word| word.as_str().map(str::to_owned))
    .unwrap_or_default()
}

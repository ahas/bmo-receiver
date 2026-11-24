//! Every body and member of the receiver's fixed API (`docs/api.md`), in the common wire format: camelCase keys,
//! epoch milliseconds, signed 64-bit ids, one page of items and more. A route's request and its answer are each a
//! type here, so the OpenAPI document and the Rust client are both written from these.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// `GET /health`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Health {
  pub ready: bool,
  pub reason: Option<String>,
  pub version: String,
}

/// `GET /owner`: the deployment's owner as a sender, and the owner's private room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Owner {
  pub sender_id: i64,
  pub room: i64,
}

/// One room, as the room list and one room's read answer it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Room {
  pub id: i64,
  pub title: Option<String>,
  pub message_count: i64,
  /// Epoch milliseconds; null where the room holds no message.
  pub last_message_at: Option<i64>,
  pub pinned: bool,
  /// The people that have stood as a sender in the room.
  pub senders: Vec<i64>,
  /// The supervisors the room names to answer it, in name order; empty where it names none and the default
  /// answerer answers it.
  pub supervisors: Vec<String>,
  pub owner_room: bool,
  pub registered: bool,
  /// Whether the room stands on the listen list, the rooms the owner allowed with his allow command and has not
  /// denied since; always true for the owner's private room.
  pub listened: bool,
  /// The room where work filed from this one is told.
  pub work_room: i64,
  /// The mode the room was added in; null for a room never added.
  pub mode: Option<RoomMode>,
  /// The account whose private room this is: a BMO's own room on the record, or the owner's private room; null for
  /// every other room, a group room every other member has left among them.
  pub private_of: Option<i64>,
  /// Whether the room is a private room whose account is not known yet, so no account writes in it: the owner's
  /// private room before an account is linked to his sender.
  pub sealed: bool,
}

/// What a room added is to the agents: answered as the user, or the user alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum RoomMode {
  /// The room's agents answer it, in the account of the person the provider reads.
  Avatar,
  /// The user alone: the room is kept and no agent is woken in it.
  RecordOnly,
}

impl RoomMode {
  /// The mode as the wire names it.
  pub fn as_str(&self) -> &'static str {
    match self {
      Self::Avatar => "avatar",
      Self::RecordOnly => "record-only",
    }
  }
}

/// `POST /rooms`: a room to add, on a chat service by its key, as that service numbers it, in a mode, under a title
/// where one is named.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddRoom {
  pub provider: String,
  pub room_id: i64,
  pub mode: RoomMode,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub title: Option<String>,
}

/// `POST /rooms`: a conversation a chat service starts, a one-to-one with `contact_id` or a group of
/// `contact_ids` under `title`, in the account `account_id` names, added in a mode as the room it comes to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StartRoom {
  pub provider: String,
  pub mode: RoomMode,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub contact_id: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub contact_ids: Option<Vec<i64>>,
  /// The room's title; a group's, which a group needs.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub title: Option<String>,
  /// The account the conversation is started in; unnamed, the service's own.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub account_id: Option<i64>,
}

/// `POST /rooms` with no chat service: an agent room on the record alone, answered by `supervisors`, its first
/// member the account `opened_by` names where one opens it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentRoom {
  pub title: String,
  pub supervisors: Vec<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub opened_by: Option<i64>,
}

/// The body of `POST /rooms`: a room of a chat service's account, a conversation the service starts, or an agent
/// room on the record alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum RoomAddition {
  Room(AddRoom),
  Start(StartRoom),
  Agent(AgentRoom),
}

/// A room a provider's account is in, as the provider answers it, with whether the receiver holds it and the mode
/// the receiver added it in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountRoom {
  pub id: i64,
  pub title: Option<String>,
  /// `owner` or `group`.
  pub kind: String,
  /// The one person of a one-to-one with the account.
  pub private_to: Option<i64>,
  /// Whether the receiver holds the room: its own, one it keeps a title or a message of, or one added.
  #[serde(default)]
  pub held: bool,
  /// The mode the room was added in; null for a room not added.
  pub mode: Option<RoomMode>,
}

/// A contact of a provider's account, as the provider answers it, with whether the receiver holds the one-to-one
/// the account has with it and the mode that room was added in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountContact {
  pub id: i64,
  pub name: String,
  /// `person` or `bot`.
  pub kind: String,
  /// The one-to-one the account already has with the contact.
  pub room_id: Option<i64>,
  pub held: bool,
  pub mode: Option<RoomMode>,
}

/// A room's name alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoomTitle {
  pub title: Option<String>,
}

/// One agent answering a room.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Answerer {
  pub agent_name: String,
  /// Whether it is the supervisor the receiver's default answerer setting names.
  pub default_answerer: bool,
}

/// `GET /rooms/{roomId}/answerers`: the room's default answerer, its voice, and every agent answering it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Answerers {
  pub default: String,
  pub voice: String,
  pub items: Vec<Answerer>,
  pub more: bool,
}

/// Every room with its agents answering it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoomAnswerers {
  pub room: i64,
  pub agents: Vec<String>,
}

/// A file a message carried, as the record kept it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KeptFile {
  pub kind: String,
  pub source: String,
  pub stored_path: Option<String>,
  pub summary: Option<String>,
  /// Epoch milliseconds.
  pub collected_at: i64,
}

/// One message, wherever it is answered.
/// A reaction the signed-in person puts on a message, by its emoji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Reacting {
  pub emoji: String,
  /// The account the act is for where the console acts for one; refused in another account's private room.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub account: Option<i64>,
}

/// A pin the signed-in person sets on a message, or takes off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Pinning {
  pub on: bool,
  /// The account the act is for where the console acts for one; refused in another account's private room.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub account: Option<i64>,
}

/// The new words of the signed-in person's own message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Editing {
  pub text: String,
  /// The account the act is for where the console acts for one; refused in another account's private room.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub account: Option<i64>,
}

/// `PUT /rooms/{roomId}/messages/{messageId}/speaker`: the agent a kept line standing under its own number in the
/// room is to be filed as.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChangeSpeaker {
  pub agent_name: String,
}

/// A sticker of the record's set, by its emoji, as the set lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Sticker {
  pub emoji: String,
}

/// A sticker of the record's set the signed-in person sends, by its emoji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StickerSend {
  pub emoji: String,
  /// The account the act is for where the console acts for one; refused in another account's private room.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub account: Option<i64>,
}

/// The stickers the record keeps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Stickers {
  pub items: Vec<Sticker>,
}

/// A room's pinned messages, newest pin first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Pins {
  pub items: Vec<Message>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Message {
  pub id: i64,
  pub room: i64,
  pub sender_id: Option<i64>,
  pub sender_name: String,
  /// Epoch milliseconds.
  pub sent_at: i64,
  pub text: String,
  pub image_path: Option<String>,
  pub reply_to_message_id: Option<i64>,
  pub answer_seconds: Option<i64>,
  pub round: Option<String>,
  pub album: Option<i64>,
  pub via: Option<String>,
  /// The registration name of the agent whose words these are, set on a send the receiver archived itself and
  /// null on a message that came in from a provider.
  pub agent_name: Option<String>,
  /// The voice the send went out in (`bot-account`, `human-avatar`, `human-reader`), set beside `agentName` and
  /// null on a message that came in from a provider.
  pub voice: Option<String>,
  /// `sticker` for a sticker kept on the record, `thinking` for a thinking block a record holds as a message,
  /// `fixed-line` for a fixed line the receiver said into the room itself and for a line a chat service handed
  /// over, null for any other message.
  #[serde(default)]
  pub kind: Option<String>,
  /// On a message of kind `fixed-line`, the line it was said from; null on any other.
  #[serde(default)]
  pub line: Option<KeptFixedLine>,
  /// When the record kept an edit of the message, in epoch milliseconds.
  #[serde(default)]
  pub edited_at: Option<i64>,
  /// When the record kept the message's delete, its words cleared, in epoch milliseconds.
  #[serde(default)]
  pub deleted_at: Option<i64>,
  /// When the record kept the message pinned, in epoch milliseconds; null where it stands unpinned.
  #[serde(default)]
  pub pinned_at: Option<i64>,
  /// The reactions the record keeps on the message, in the order they were put there.
  #[serde(default)]
  pub reactions: Vec<Reaction>,
  /// Where a copy the owner forwarded came from: the room, the message and who said it; null on any other message.
  #[serde(default)]
  pub forwarded_from: Option<ForwardedFrom>,
  /// The room's situation note as the round that judged this message read it; null on a message no round judged
  /// under one.
  #[serde(default)]
  pub situation: Option<Situation>,
  /// The words heard in a voice note; null on any other message and on one whose recording could not be heard.
  #[serde(default)]
  pub transcript: Option<String>,
  /// How long a voice note's recording ran, in seconds; null on any other message.
  #[serde(default)]
  pub voice_seconds: Option<f64>,
  /// Whether a voice note was heard (`heard`) or could not be (`failed`); null on any other message.
  #[serde(default)]
  pub voice_state: Option<String>,
  pub files: Vec<KeptFile>,
  /// The chat services that did not carry an act on this message, each a `NotCarried`: it stands on the answer of
  /// an act alone, and where there is nothing to name the field is left out.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub not_carried: Option<Vec<NotCarried>>,
}

/// A fixed line as a message keeps it: its code and the fields it carries, as the receiver's `docs/codes.md`,
/// 「Fixed lines」, names them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct KeptFixedLine {
  pub code: String,
  pub fields: serde_json::Map<String, Value>,
}

/// One chat service that did not carry an act on a message: its key, the `reason` the act did not go there
/// (`no-copy` where it holds no number for the message, `no-service` where the room's list names a provider no
/// chat service stands under, `refused` where it turned the act down in every voice it has for the room), and the
/// service's own `detail` sentence where it gave one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NotCarried {
  pub provider: String,
  pub reason: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub detail: Option<String>,
}

/// A room's situation note as one round read it: its text and whether that round wrote it afresh.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Situation {
  pub text: String,
  pub changed: bool,
}

/// A room's situation note as it stands, with when it last changed in epoch milliseconds; both null where the room
/// has no note yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoomSituation {
  pub text: Option<String>,
  pub updated_at: Option<i64>,
}

/// The message a forwarded copy stands for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ForwardedFrom {
  pub room: i64,
  pub message_id: i64,
  pub sender_name: String,
}

/// A message forwarded into another room: the room it goes into, and the account that forwards it with the name it
/// is shown by, where the console forwards for one; the owner's where it names none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Forward {
  pub to: i64,
  /// The forwarding account, whose sender the copy stands under; refused into another account's private room.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub account: Option<i64>,
  /// The forwarding account's shown name, else the name its identity last went by.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub name: Option<String>,
}

/// A reaction the record keeps on a message: its emoji and who put it there, an agent's name or a person's number.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Reaction {
  pub emoji: String,
  pub by: String,
}

/// What a chat service handed over beside a message, opaque to the receiver.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MessageMetadata {
  pub metadata: Option<Value>,
}

/// One of a message's files, by its place among them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
  pub place: usize,
  pub kind: String,
  pub name: String,
  #[serde(rename = "type")]
  pub content_type: Option<String>,
  pub size: Option<u64>,
  pub kept: bool,
}

/// `POST /rooms/{roomId}/file-fills`: a pass over a room's placeholder rows, asking its chat service for their
/// files again.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileFill {
  /// How many rows the pass may ask; null for the policy's default, and never more than its ceiling.
  #[serde(default)]
  pub rows: Option<u32>,
  /// List the rows the pass would ask, and ask nothing.
  #[serde(default)]
  pub dry_run: bool,
}

/// What the chat service answered for one row a pass asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum FillOutcome {
  /// Files came back and were kept.
  Filled,
  /// The service answered no file.
  None,
  /// The platform would not give the files.
  Unavailable,
  /// The service refused the ask.
  Refused,
}

impl FillOutcome {
  /// The outcome as the store and the wire name it.
  pub fn as_str(self) -> &'static str {
    match self {
      Self::Filled => "filled",
      Self::None => "none",
      Self::Unavailable => "unavailable",
      Self::Refused => "refused",
    }
  }
}

/// One file a filled row kept: its kind and the bytes the store holds of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FilledFile {
  /// `image` for a picture, else the kind its kept row names.
  pub kind: String,
  pub bytes: u64,
}

/// One row a pass reached.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FilledRow {
  pub message: i64,
  /// Epoch milliseconds.
  pub sent_at: i64,
  /// What the service answered; null on a dry run.
  pub outcome: Option<FillOutcome>,
  /// The service's own sentence, where it gave one.
  pub reason: Option<String>,
  pub files: Vec<FilledFile>,
}

/// `POST /rooms/{roomId}/file-fills`: the rows a pass reached, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileFills {
  pub room: i64,
  pub rows: Vec<FilledRow>,
  /// Why the room's road could not be reached, where that ended the pass; null otherwise.
  pub unreached: Option<String>,
}

/// One picture of a room.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Picture {
  pub id: i64,
  pub album: Option<i64>,
  pub sender_name: String,
  /// Epoch milliseconds.
  pub sent_at: i64,
  pub text: String,
  pub file: Option<PictureFile>,
}

/// The file behind a picture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PictureFile {
  pub name: Option<String>,
  pub bytes: u64,
  #[serde(rename = "type")]
  pub content_type: Option<String>,
}

/// A room's pictures, oldest first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Pictures {
  pub items: Vec<Picture>,
  pub more: bool,
  /// How many of the room's pictures stand before the first one answered.
  pub before: i64,
  pub total: i64,
}

/// One profile picture the archive keeps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Avatar {
  /// `room` or `person`.
  pub kind: String,
  pub id: i64,
  /// Epoch milliseconds.
  pub taken_at: i64,
}

/// The profile pictures kept, oldest taken first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Avatars {
  pub items: Vec<Avatar>,
  pub more: bool,
}

/// The page of pictures asked for.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AvatarQuery {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub after: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub limit: Option<i64>,
}

/// One agent's figures in a room.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AgentFigure {
  pub agent_name: String,
  pub messages: i64,
  pub last_answer_at: Option<i64>,
  pub session: Option<String>,
  pub label: Option<String>,
  pub last_mode: Option<String>,
  pub last_model: Option<String>,
  pub last_effort: Option<Value>,
  pub session_mode: Option<String>,
  pub model: Option<String>,
  pub context_tokens: Option<Value>,
  pub runs: Option<i64>,
  pub last_run_at: Option<i64>,
  pub week_tokens: Option<i64>,
  pub total_tokens: Option<i64>,
}

/// One speaker among the room's most talkative people.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Speaker {
  pub name: String,
  pub messages: i64,
}

/// A room's figures.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Figures {
  pub room: i64,
  pub messages: i64,
  pub from_people: i64,
  pub speakers: i64,
  pub first_at: Option<i64>,
  pub last_at: Option<i64>,
  pub last_answer_at: Option<i64>,
  pub top: Vec<Speaker>,
  pub by_hour: Vec<i64>,
  pub agents: Vec<AgentFigure>,
}

/// How long the default answerer's window in a room has left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Window {
  pub open_for_ms: i64,
  /// Epoch milliseconds; null where the room never opened one.
  pub closes_at: Option<i64>,
}

/// A chat provider a room may be on: its key, what it is called, the verbs it performs, whether it lists the rooms
/// of its account, and the modes a room added on it may take, as it declares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
  pub key: String,
  pub label: String,
  /// Every chat verb it declares it performs, by bmo-chat's names: one provider's own, whatever room it is on.
  #[serde(default)]
  pub can: Vec<String>,
  pub lists_rooms: bool,
  #[serde(default)]
  pub lists_contacts: bool,
  /// Whether it starts a one-to-one with a contact.
  #[serde(default)]
  pub starts_chats: bool,
  /// Whether it starts a group of contacts under a title.
  #[serde(default)]
  pub starts_groups: bool,
  pub room_modes: Vec<RoomMode>,
  /// The largest file it takes, in bytes, as it declares; null where it declares none.
  #[serde(default)]
  pub file_bytes: Option<u64>,
}

/// The chat providers a room is on, by key in order, whether its list was set rather than read off what the room
/// holds, and the largest file the room takes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoomProviders {
  pub providers: Vec<String>,
  pub chosen: bool,
  /// The smallest limit any chat service on the list declares; null where none declares one and the room takes a
  /// file of any size.
  #[serde(default)]
  pub file_limit: Option<FileLimit>,
}

/// The largest file a room takes, and the chat service that declares it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileLimit {
  pub provider: String,
  pub bytes: u64,
}

/// One person in a room, as the room's own chat service lists them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoomMember {
  /// The sender number the room's messages carry for the person; null where the service gave the person none.
  pub person: Option<i64>,
  /// What the room calls the person.
  pub name: String,
  /// The person's status line, or null where the service gave none.
  pub status_message: Option<String>,
}

/// The people in a room, as the room's own chat service lists them, whole.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RoomMembers {
  pub members: Vec<RoomMember>,
}

/// A member of a room by account: who let it in and when, both null for a private room's own account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoomAccount {
  pub account: i64,
  pub added_by: Option<i64>,
  pub added_at: Option<i64>,
}

/// `GET /rooms/{roomId}/accounts`: a room's members by account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RoomAccounts {
  pub accounts: Vec<RoomAccount>,
}

/// Whom a room's reference may reach for a supervisor (`docs/receiver.md`, 「Whether someone outside the company
/// sits in a room」): `internal` where everyone in the room stands in the company of the supervisor's owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Audience {
  Internal,
  External,
}

/// `GET /rooms/{roomId}/audience`: a room's audience for a supervisor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoomAudience {
  pub room: i64,
  pub agent: String,
  pub audience: Audience,
}

/// `PUT /rooms/{roomId}/accounts/{account}`: the account letting another in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LetIn {
  pub by: i64,
}

/// `POST /rooms/{roomId}/accounts`: the accounts let in, at least one and each once, and the account letting them in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LetInSeveral {
  pub accounts: Vec<i64>,
  pub by: i64,
}

/// `POST /rooms/{roomId}/accounts`: each account asked as a member as it now stands, and those the call let in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LetInAccounts {
  pub accounts: Vec<RoomAccount>,
  pub let_in: Vec<i64>,
}

/// `POST /rooms/{roomId}/unit-accounts`: the organisation unit whose people are let in, by its number in the web
/// unit, and the account letting them in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LetInUnit {
  pub unit: i64,
  pub by: i64,
}

/// `POST /rooms/{roomId}/unit-accounts`: the unit as the web unit named it then, each of its people as a member of
/// the room as it now stands, and those the call let in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LetInUnitAccounts {
  pub unit: i64,
  pub unit_name: String,
  pub accounts: Vec<RoomAccount>,
  pub let_in: Vec<i64>,
}

/// `PUT /accounts/{account}/identity`'s body: the name the account's record identity goes by from then, the same
/// field an account's words carry; none names nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AccountNaming {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub name: Option<String>,
}

/// `PUT /accounts/{account}/identity`: an account's record identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountIdentity {
  pub account: i64,
  pub sender: i64,
}

/// `GET /accounts/{account}/rooms`: the rooms an account is a member of in the console.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AccountMemberships {
  pub rooms: Vec<i64>,
}

/// `POST /rooms/{roomId}/words`: words an account said in the console, with the name it is shown by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountWords {
  pub account: i64,
  pub name: String,
  pub text: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub reply_to: Option<i64>,
}

/// Whether a person is reachable now: online, busy or not; away, signed in but idle; offline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum PresenceState {
  Online,
  Away,
  Offline,
}

/// `PUT /accounts/{account}/presence`: an account's presence as the console draws it from its tabs; a null state
/// takes the console's word back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountPresence {
  pub state: Option<PresenceState>,
  /// Epoch milliseconds: the last moment a visible tab of the account reported.
  pub last_seen_at: Option<i64>,
}

/// `POST /rooms/{roomId}/typing`: an account writing in a room's box in the console, or no longer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountTyping {
  pub account: i64,
  pub on: bool,
}

/// A room's chat providers set whole: at least one, each once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChooseProviders {
  pub providers: Vec<String>,
}

/// `GET /rooms/{roomId}/registration`: what the receiver holds of a room's registration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoomRegistration {
  pub room: i64,
  /// The mode the room was added in; null for a room never added.
  pub mode: Option<RoomMode>,
  /// The chat providers the room is on, in order.
  pub providers: Vec<String>,
  /// Whether the list was set rather than read off what the room holds.
  pub chosen: bool,
  /// The agents answering the room.
  pub answerers: Answerers,
  /// Whether every chat service on the room's list reads it in the owner's account alone.
  pub read_only: bool,
  /// The chat services on the list that read the room in the owner's account alone, in list order.
  pub read_only_by: Vec<String>,
}

/// `GET /rooms/{roomId}/leaders`: the accounts that lead a room, owning a supervisor it names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoomLeaders {
  pub room: i64,
  pub leaders: Vec<i64>,
}

/// `GET /rooms/{roomId}/archive`: whether a room is archived and locked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoomArchive {
  pub room: i64,
  /// When the room was archived, in milliseconds; null while it is open.
  pub archived_at: Option<i64>,
  /// Who archived it; null while it is open.
  pub archived_by: Option<String>,
}

/// `PUT /rooms/{roomId}/archive`: whether a room is to stand archived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRoomArchive {
  pub archived: bool,
}

/// `GET /rooms/{roomId}/tone`: the tone layer a room is set to, and the layers the deployment offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoomTone {
  pub room: i64,
  /// The layer's name; null where the room is set to none.
  pub tone: Option<String>,
  /// The layers a room may be set to, as the deployment's configuration names them.
  pub tones: Vec<String>,
}

/// `PUT /rooms/{roomId}/tone`: the tone layer a room is to stand in, one the deployment offers, or null for none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRoomTone {
  pub tone: Option<String>,
}

/// `PUT /rooms/{roomId}/title`: the name a room on the record alone is to stand under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRoomTitle {
  pub title: String,
}

/// `PUT /rooms/{roomId}/registration`: the mode a room's registration is to stand in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChangeRoomMode {
  pub mode: RoomMode,
}

/// A room's road.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Road {
  /// `owner` or `group`.
  pub kind: String,
  pub one_to_one: bool,
}

/// The sender an agent's own words stand under in a room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OwnNumber {
  pub number: Option<i64>,
}

/// Whether a person spoke in a room in a span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Spoke {
  pub spoke: bool,
}

/// Whether the person is the owner, and whether the owner spoke in the span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OwnerSpoke {
  pub owner: bool,
  pub spoke: bool,
}

/// Whether a message's sender is the owner in that room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SenderOwner {
  pub owner: bool,
}

/// One person.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Person {
  pub id: i64,
  pub name: Option<String>,
}

/// How many of a room's messages an agent has marked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadMarkCount {
  pub count: i64,
}

/// A room and how many of its messages an agent has not marked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoomReadCount {
  pub room: i64,
  pub count: i64,
}

/// What a delete of an agent's marks let go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Forgotten {
  pub forgotten: i64,
}

/// One notice a room was voiced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
  pub id: i64,
  pub kind: Option<String>,
  pub summary: Option<String>,
  pub text: String,
  /// Epoch milliseconds.
  pub created_at: i64,
}

/// The verbs of the road a send naming `via` goes out on in a room; one provider's own are its `can`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SendVerbs {
  pub items: Vec<String>,
  pub more: bool,
}

/// Whether a text waits for something to happen first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PromiseReading {
  pub waits: bool,
}

/// What an instruction hands a session: a fact, or the words of an order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Said {
  #[serde(default)]
  pub fact: Option<Value>,
  #[serde(default)]
  pub order: Option<String>,
}

/// An instruction queued for an agent's session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InstructionRequest {
  #[serde(default)]
  pub agent_name: Option<String>,
  #[serde(default)]
  pub room: Option<i64>,
  pub said: Said,
  #[serde(default)]
  pub kind: Option<String>,
  pub sort: String,
  #[serde(default)]
  pub summary: Option<String>,
  /// Whether somebody waits on it: a room reply, the default, or housekeeping.
  #[serde(default)]
  pub cause: Option<InstructionCause>,
  #[serde(default)]
  pub events: Vec<String>,
  /// A record the agent reads and no room hears: a round carrying only such rows sends no message.
  #[serde(default)]
  pub agent_only: bool,
  #[serde(default)]
  pub rides_until: Option<i64>,
  /// Whether it rides its room's rounds for good, opening no turn of its own.
  #[serde(default)]
  pub rides_for_good: bool,
  #[serde(default)]
  pub via: Option<String>,
  #[serde(default)]
  pub run: Option<i64>,
  /// The unit or source that published the row, kept with it and named in every turn it rides; a row naming none
  /// is read by the first word of its kind.
  #[serde(default)]
  pub publisher: Option<String>,
  /// A fixed line the room hears at once as the service's own as the row is written, its `code` and fields
  /// (docs/codes.md, 「Fixed lines」): one a unit may ask for alone, `supply-closed`.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub line: Option<Value>,
}

/// Whether somebody waits on an instruction (docs/wake.md, 「Quiet instructions」).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InstructionCause {
  /// It answers something a room or the owner asked or set going, and opens the agent's turn by itself, on a
  /// session past its cache lifetime too.
  Reply,
  /// A record for the agent alone that nobody waits on, which waits for a round something else opens where the
  /// session is past its cache lifetime.
  Housekeeping,
}

/// How far an instruction has gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InstructionState {
  Pending,
  Handed,
  Delivered,
}

/// An instruction and how far it has gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Instruction {
  pub id: i64,
  pub state: InstructionState,
}

/// The id a created member answers with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Created {
  pub id: i64,
}

/// A unit declaring what it holds of a room.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoomHolder {
  pub address: String,
  #[serde(default)]
  pub holdings: Vec<Holding>,
}

/// One thing a unit holds of a room.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Holding {
  pub name: String,
  /// `kept` where the rows stay, `warns` where the unit shows what it answers before the removal.
  pub effect: String,
}

/// A send asked of the receiver.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SendRequest {
  pub asker: Asker,
  /// The tool's name: `say`, `photo`, `file`, `voice`, `react`, `pin`, `sticker` or `random_draw`, or `speak`, a
  /// spoken talk asked by its words.
  pub tool: String,
  pub input: Value,
}

/// Who asks a send.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Asker {
  #[serde(default)]
  pub room: Option<i64>,
  #[serde(default)]
  pub via: Option<String>,
  #[serde(default)]
  pub agent_name: Option<String>,
  /// The moment the asking turn took its words, in epoch milliseconds: a turn begun at or before its
  /// agent's reset point is refused.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub began: Option<i64>,
  #[serde(default)]
  pub by: Option<String>,
  #[serde(default)]
  pub job: bool,
  /// The command or unit the words come through, which words handed to the room's supervisor name.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub from: Option<String>,
}

/// What a send came to: carried, refused, or handed to the room's supervisor as the order `handed` numbers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SendAnswer {
  pub lines: Vec<String>,
  pub refusal: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub handed: Option<i64>,
}

/// The owner's words typed on a screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OwnerMessage {
  pub chat_id: i64,
  pub text: String,
  #[serde(default)]
  pub reply_to: Option<i64>,
}

/// Words the owner typed for a room's session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OwnerInstruction {
  pub chat_id: i64,
  pub text: String,
}

/// A turn of a talk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TalkTurn {
  pub owner: bool,
  pub text: String,
}

/// A handoff the agency asks for ahead of a fresh session it opens for an agent that had one: why, and the room
/// whose session is asked where the agent keeps one in each room, and the room the line is told in; the cause that
/// line names, a `HandoverCause` kind, `other` where none is named, and the account key of the person who asked,
/// where one did. `clean` asks the session to consolidate and close with no handoff, so the fresh session opens on
/// no note; `ask` false asks no session at all and tells the room's line alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HandoffAsked {
  pub reason: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub room: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub cause: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub by: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub clean: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub ask: Option<bool>,
}

/// The handoff file a standing session wrote: where it stands and how long it is, in bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HandoffWritten {
  pub path: String,
  pub bytes: u64,
}

/// A handover asked for an agent's one session: why, in the asker's words, for the receiver's journal and the room's
/// line; the room that line goes to, and the account key of the person who asked, where the asking names them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HandoverAsked {
  pub reason: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub room: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub by: Option<i64>,
}

/// A handover written for an agent's one session, which its next turn carries out: the agent, and the reason the
/// replacement goes by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HandoverPending {
  pub agent_name: String,
  pub session_reason: String,
}

/// The voice briefing's two switches: the whole briefing, and whether a summary plays by itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VoiceBriefing {
  pub on: bool,
  pub autoplay: bool,
}

/// The owner's private chat's two notice switches: the restart's notices, and the rest's line and its lifting's
/// notice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NoticeSwitches {
  pub restart: bool,
  pub rest: bool,
}

/// One notice switch moved, the kind named by the path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NoticeSwitch {
  pub on: bool,
}

/// One of the voice briefing's switches moved: exactly one of the two, as a boolean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VoiceBriefingSwitch {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub on: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub autoplay: Option<bool>,
}

/// A line's full reading: `making` while its script is written and the voice engine speaks it, `ready` once it
/// can be heard, and the engine's briefing, whose sound the engine serves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LineReading {
  pub state: String,
  /// Null while the line's agent is still writing its script.
  pub briefing_id: Option<i64>,
}

/// A talk's topic and what the owner says.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Talk {
  /// What the talk is about: `kind` names it, `heading` is its head paragraph and `fields` are the values its
  /// head carries; a caller's own kind needs no code in the receiver.
  pub topic: Value,
  #[serde(default)]
  pub turns: Vec<TalkTurn>,
  pub said: String,
}

/// What a talk's turn answered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TalkAnswer {
  pub answer: Option<String>,
}

/// A supervisor added with a room of the console's own, on its one road: `provider` and `model`, with `effort`,
/// `thinking` and `answerTokens` where it names them, each the provider's own where it does not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SupervisorRequest {
  pub name: String,
  pub label: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub provider: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub model: Option<String>,
  /// The repository its items go to, one the agency holds.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub repository: Option<String>,
  pub room: String,
  /// A level of the reasoning policy the model takes.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub effort: Option<String>,
  /// `on`, `off`, or none for the provider's own.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub thinking: Option<String>,
  /// The answer ceiling, on the registry's provider.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub answer_tokens: Option<i64>,
  /// The account the supervisor belongs to, whose private room its own room is kept as.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub owner_account: Option<i64>,
}

/// The console room the receiver stood up for a supervisor it added: whether it took back one already standing
/// under the name, and what the owner is told of that.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddedRoom {
  pub id: i64,
  pub taken_back: bool,
  pub said: Option<Value>,
}

/// `POST /supervisors`: the agency's read of the supervisor, what became of its brain, and its room.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SupervisorAdded {
  pub supervisor: Value,
  pub brain: Value,
  pub room: AddedRoom,
}

/// A reset asked of a supervisor: its ranges, whether it only counts, and who asks.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SupervisorReset {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub conversation: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub person: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub dry_run: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub by: Option<String>,
}

/// One room a supervisor answers: whether the room names it, or names none and it is the default answerer, and
/// whether it answers there alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SupervisorRoom {
  pub id: i64,
  pub title: Option<String>,
  pub named: bool,
  pub alone: bool,
}

/// The default answerer setting: the supervisor that answers every room naming none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DefaultAnswerer {
  pub supervisor: String,
}

/// A room's whole list of supervisors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoomSupervisors {
  pub supervisors: Vec<String>,
}

/// The pinned order as it now stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PinnedRooms {
  pub room_ids: Vec<i64>,
}

/// The pinned order set whole.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PinRooms {
  pub room_ids: Vec<i64>,
  /// The account pinning; refused where the order pins or unpins another account's private room.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub account: Option<i64>,
}

/// One room pinned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PinRoom {
  pub room: i64,
  /// The account the act is for where the console acts for one; refused in another account's private room.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub account: Option<i64>,
}

/// What a round settled, without the instructions' text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoundOpening {
  pub round: String,
  pub instructions: Vec<Opening>,
}

/// One quiet instruction a round settled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Opening {
  pub id: i64,
  pub sort: Option<String>,
  pub summary: String,
  /// Epoch milliseconds.
  pub created_at: i64,
}

/// One quiet instruction's whole text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OpeningText {
  pub id: i64,
  pub text: String,
}

/// What a round's turns wrote in the round log, row by row (docs/instruction.md, 「The round log」).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoundPipeline {
  pub round: String,
  pub rows: Vec<PipelineRound>,
}

/// One turn of a round, or its end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PipelineRound {
  pub id: i64,
  pub agent_name: String,
  /// From 1, the round's own turn first and then the check of the words that arrived meanwhile, the audit and each
  /// stage in the order asked; 0 for the agent's own judgement before a round its open window alone let in.
  pub number: i64,
  /// The row's name: the stage it ran, `arrived` for the closing check, `message` for the round's own turn.
  pub question: String,
  /// `supervisor` (the agent's own judgement before a round), `turn` (the round's own turn), `arrived` (the closing
  /// check), `audit` (the audit of the people the message answers) or `stage` (an instruction stage), or `end`.
  pub kind: String,
  pub answer_chars: i64,
  pub changed: bool,
  /// `answered`, `send`, `no-verdict`, `skip` or `failed` for a turn or check, and `pass` for a check whose
  /// empty answer kept the message; `pass`, `revised` or `failed` for the audit and a stage; `message` or `skip`
  /// for the end.
  pub outcome: String,
  pub detail: Option<String>,
  pub answer: Option<String>,
  pub ms: i64,
  /// Epoch milliseconds.
  pub at: i64,
  /// On the end, what the round read from the brain; empty on every other row.
  #[serde(default)]
  pub reads: Vec<RoundRead>,
  /// On the end, the agency session the round's turn ran on, null where it ran on none; null on every other row.
  #[serde(default)]
  pub session: Option<String>,
  /// On the end, how many pieces the room's earlier rounds of the same session read in full (`attached` or
  /// `read`), 0 for a session's first round in the room or a round on no session; null on every other row.
  #[serde(default)]
  pub loaded: Option<i64>,
  /// On a turn's row whose turn spanned more than one turn of the agent's program, each of those turns' answers in
  /// order, the row's answer built from them; empty on every other row.
  #[serde(default)]
  pub versions: Vec<RoundVersion>,
}

/// One answer of the program's turns a turn of the process spanned (docs/instruction.md, 「The round log」).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoundVersion {
  /// That turn's answer as it wrote it.
  pub text: String,
  /// What opened that turn: `handed`, words handed over (the turn's own prompt or words pushed into it), or
  /// `program`, the program on its own (a background task's notice).
  pub opened: String,
}

/// One thing a round read from the brain (docs/instruction.md, 「The round log」).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoundRead {
  pub path: String,
  /// What it holds: the index's words for it where the gate's index lists it, else the name it went in with.
  pub holds: Option<String>,
  /// The brain memory it is, where it is one.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub memory_id: Option<i64>,
  /// `attached` (a piece of the turn's core that went in full), `held` (one that went as a mark, the session
  /// holding that version, or a piece the agent read again through the index at a version the session already
  /// held) or `read` (a piece the agent read itself through the index during the round at a version new to it).
  pub how: String,
}

/// A row of a round's pipeline (docs/instruction.md, 「The round log」).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InstructionChain {
  /// The supervisor's name, or `*` for the policy's default chain every supervisor follows until it sets one.
  pub agent_name: String,
  /// The chain's stage names, in the order they are asked once the message stands; empty runs no stage.
  pub stages: Vec<String>,
  /// `set` or `default`.
  pub source: String,
}

/// A supervisor's chain set by the console.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InstructionChainSet {
  pub stages: Vec<String>,
}

/// A stage of the catalogue: its name, its text and whether the console set the text; the policy's text of it where
/// the console set one for a supervisor and the policy holds it too, null for a stage the console added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InstructionText {
  pub name: String,
  pub text: String,
  /// `set` or `default`.
  pub source: String,
  #[serde(default)]
  pub policy_text: Option<String>,
}

/// A question's text set by the console.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InstructionTextSet {
  pub text: String,
}

/// The live board: the process's turns, folds and each room's queued message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Live {
  /// Epoch milliseconds.
  pub at: i64,
  pub turns: Vec<Value>,
  pub compacting: Vec<Compacting>,
  pub queued: Vec<QueuedMessage>,
}

/// A room whose session is folding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Compacting {
  pub room: i64,
  /// Epoch milliseconds.
  pub since: i64,
}

/// The message a room queued for a turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QueuedMessage {
  pub room: i64,
  pub message_id: i64,
  /// Epoch milliseconds.
  pub since: i64,
}

/// The event a process beside the receiver publishes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublishedEvent {
  /// `message.created`, `briefing.created` or `avatars.changed`.
  pub kind: String,
  #[serde(default)]
  pub data: Value,
}

/// What the agency reads from a hook the receiver serves.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HookAnswer {
  pub answer: Value,
}

/// What a room's removal would do, answered by `DELETE /rooms/{roomId}?dryRun=true`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RemovalPreview {
  pub room: i64,
  pub refused: Option<String>,
  pub warnings: Value,
  pub warns: Value,
  pub goes: Value,
  pub stays: Value,
  pub pictures: Value,
  pub settings: Value,
  pub folders: Value,
  pub units: Value,
}

/// What a room's removal took.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Removal {
  pub room: i64,
  pub taken: Value,
  pub cleared: Value,
  pub pictures: Value,
  pub settings: Value,
  pub folders: Value,
  pub holders: Value,
  pub units: Value,
  pub could_not: Value,
}

/// The query of a supervisor's read of a group room's messages.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemberRead {
  /// The cursor message the reader holds: the page ends just before it.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub before: Option<i64>,
  /// The cursor message the reader holds: the page begins just past it.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub after: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub limit: Option<i64>,
}

/// The query of a room's message page.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MessageQuery {
  /// The cursor message the reader holds: the page ends just before it.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub before: Option<i64>,
  /// The cursor message the reader holds: the page begins just past it.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub after: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub limit: Option<i64>,
  /// The agent whose read marks are asked of.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub agent_name: Option<String>,
  /// A moment in epoch milliseconds: only messages sent at or after it.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub since: Option<i64>,
  /// The messages an agent has marked, after `after`.
  #[serde(default)]
  pub marked: bool,
  /// The messages an agent has not marked, oldest first.
  #[serde(default)]
  pub unread: bool,
  /// `person`, `agent` or `any`.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub by: Option<String>,
  /// `media`, `file`, `music`, `voice` or `link`: only the messages holding that kind, walked by `before`.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub holding: Option<String>,
}

/// The query of a room's picture page.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PictureQuery {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub at: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub before: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub after: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub limit: Option<i64>,
}

/// The query of a room's notices.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NoticeQuery {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub before: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub after: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub limit: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub kind: Option<String>,
  /// Epoch milliseconds.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub from: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub to: Option<i64>,
}

/// The query of the event stream.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EventsQuery {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub after: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub run: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub kinds: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub room_id: Option<i64>,
}

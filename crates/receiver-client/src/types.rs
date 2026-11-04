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
  /// The room where work filed from this one is told.
  pub work_room: i64,
  /// The mode the room was added in; null for a room never added.
  pub mode: Option<RoomMode>,
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
/// `contact_ids` under `title`, added in a mode as the room it comes to.
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
}

/// The body of `POST /rooms`: a room of a chat service's account, or a conversation the service starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum RoomAddition {
  Room(AddRoom),
  Start(StartRoom),
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
}

/// A pin the signed-in person sets on a message, or takes off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Pinning {
  pub on: bool,
}

/// The new words of the signed-in person's own message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Editing {
  pub text: String,
}

/// A sticker of the record's set, by its emoji: one the set lists, or one the signed-in person sends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Sticker {
  pub emoji: String,
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
  /// `sticker` for a sticker kept on the record, null for any other message.
  #[serde(default)]
  pub kind: Option<String>,
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

/// A message forwarded into another room: the room it goes into.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Forward {
  pub to: i64,
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

/// A chat provider a room may be on: its key, what it is called, whether it lists the rooms of its account, and the
/// modes a room added on it may take, as it declares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
  pub key: String,
  pub label: String,
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
}

/// The chat providers a room is on, by key in order, and whether its list was set rather than read off what the
/// room holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoomProviders {
  pub providers: Vec<String>,
  pub chosen: bool,
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

/// The verbs the road a room is answered on performs.
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
  #[serde(default)]
  pub via: Option<String>,
  #[serde(default)]
  pub run: Option<i64>,
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
  /// The tool's name: `say`, `photo`, `file`, `voice`, `fixed_line`, `react`, `pin`, `sticker` or `random_draw`,
  /// or `speak`, a spoken talk asked by its words.
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

/// What a send came to: carried, or handed to the room's supervisor as the order `handed` numbers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SendAnswer {
  pub lines: Vec<String>,
  pub refusal: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub handed: Option<i64>,
  /// A spoken talk asked in the asker's own round, which the asker makes and sends as a voice note.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub own: Option<bool>,
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

/// An item filed with a notice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OwnerNotice {
  pub code: String,
  #[serde(flatten)]
  pub params: serde_json::Map<String, Value>,
}

/// A line said in the consult room.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConsultLine {
  pub agent: String,
  pub text: String,
  #[serde(default)]
  pub reply_to: Option<i64>,
  /// The moment the asking turn took its words, in epoch milliseconds: a turn begun at or before its
  /// agent's reset point is refused.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub began: Option<i64>,
  /// The chat that set the line off, `chat:<room>`.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub by: Option<String>,
  #[serde(default)]
  pub job: bool,
  /// The command or unit the words come through.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub from: Option<String>,
}

/// What a consult line came to: said, or handed to the agent as the order `handed` numbers, with no message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConsultSaid {
  pub room: i64,
  pub id: Option<i64>,
  pub lines: Vec<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub handed: Option<i64>,
}

/// What an agent is shown of the other agents' work.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConsultBlock {
  pub block: Option<Value>,
}

/// A turn of a talk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TalkTurn {
  pub owner: bool,
  pub text: String,
}

/// A handoff the agency asks for ahead of a fresh session it opens for an agent that had one: why, and the room
/// whose session is asked where the agent keeps one in each room.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HandoffAsked {
  pub reason: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub room: Option<i64>,
}

/// The handoff file a standing session wrote: where it stands and how long it is, in bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HandoffWritten {
  pub path: String,
  pub bytes: u64,
}

/// The voice briefing's two switches: the whole briefing, and whether a summary plays by itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VoiceBriefing {
  pub on: bool,
  pub autoplay: bool,
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

/// A supervisor added with a room of the console's own. Its order is `order` whole with `quotaShare`, or
/// `provider` and `model` alone, or both, the order's first entry then being that provider and model.
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
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub order: Option<Vec<OrderEntry>>,
  /// The remaining quota, a fraction from 0 to 1, at or below which a provider of its order is passed over.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub quota_share: Option<f64>,
}

/// One entry of an agent's provider order, as the agency's order takes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OrderEntry {
  pub provider: String,
  pub model: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub effort: Option<String>,
  /// `on`, `off`, or none for the provider's own.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub thinking: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub answer_tokens: Option<i64>,
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
}

/// One room pinned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PinRoom {
  pub room: i64,
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

/// What a round's draft process wrote, row by row (docs/drafting.md, 「The round log」).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoundDrafts {
  pub round: String,
  pub rows: Vec<DraftRound>,
}

/// One turn of a round's draft process, or its end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DraftRound {
  pub id: i64,
  pub agent_name: String,
  /// From 1, each question, item and check in the order asked, the round's own turn being the first question's
  /// row; 0 for the agent's own judgement before a round its open window alone let in.
  pub number: i64,
  pub question: String,
  /// `supervisor` (the agent's own judgement whether to answer, before the round), `turn` (only where the process
  /// asks no question), `question`, `item`, `check` or `end`.
  pub kind: String,
  pub answer_chars: i64,
  pub changed: bool,
  /// `answered`, `send`, `skip` or `failed` for a turn; `message`, `skip` or `withheld` for the end.
  pub outcome: String,
  pub detail: Option<String>,
  pub answer: Option<String>,
  pub ms: i64,
  /// Epoch milliseconds.
  pub at: i64,
  /// On the end, what the round read from the brain; empty on every other row.
  #[serde(default)]
  pub reads: Vec<DraftRead>,
  /// On the end, the agency session the round's turn ran on, null where it ran on none; null on every other row.
  #[serde(default)]
  pub session: Option<String>,
  /// On the end, how many pieces the room's earlier rounds of the same session read in full (`attached` or
  /// `read`), 0 for a session's first round in the room or a round on no session; null on every other row.
  #[serde(default)]
  pub loaded: Option<i64>,
}

/// One thing a round read from the brain (docs/drafting.md, 「The round log」).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DraftRead {
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

/// A row of the draft process table (docs/drafting.md, 「The process table」).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DraftProcess {
  /// A provider's key, or `*` for any.
  pub provider: String,
  /// A tier, or `*` for any.
  pub tier: String,
  /// An agent, or `*` for any.
  pub agent_name: String,
  pub rounds: Vec<String>,
  /// The question of `rounds` from whose answer on a send is read; empty for the first.
  pub send_from: String,
  pub light: Vec<String>,
  pub light_chars: u32,
  pub ceiling_seconds: u32,
  /// `send` or `skip`.
  pub past_ceiling: String,
  /// `set` or `default`.
  pub source: String,
}

/// A row of the draft process table set by the console.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DraftProcessSet {
  pub rounds: Vec<String>,
  /// The question of `rounds` from whose answer on a send is read; empty, and left out, for the first.
  #[serde(default)]
  pub send_from: String,
  pub light: Vec<String>,
  pub light_chars: u32,
  pub ceiling_seconds: u32,
  /// `send` or `skip`.
  pub past_ceiling: String,
}

/// A question of the draft process's catalogue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DraftQuestion {
  pub name: String,
  pub text: String,
  /// `set` or `default`.
  pub source: String,
}

/// A question's text set by the console.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DraftQuestionSet {
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

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
  /// The companions answering in the room.
  pub companions: Vec<String>,
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

/// A room a provider's account is in, as the provider answers it, with the mode the receiver added it in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountRoom {
  pub id: i64,
  pub title: Option<String>,
  /// `owner` or `group`.
  pub kind: String,
  /// The one person of a one-to-one with the account.
  pub private_to: Option<i64>,
  /// The mode the room was added in; null for a room not added.
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
  /// Whether it answers beside the room's voice rather than as it.
  pub companion: bool,
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
  pub files: Vec<KeptFile>,
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

/// How long the chat agent's window in a room has left.
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
  pub expires_at: Option<i64>,
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
  #[serde(default)]
  pub expires_at: Option<i64>,
  #[serde(default)]
  pub events: Vec<String>,
  #[serde(default)]
  pub rides_until: Option<i64>,
  #[serde(default)]
  pub via: Option<String>,
  #[serde(default)]
  pub run: Option<i64>,
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
  /// The tool's name: `say`, `photo`, `file`, `voice`, `fixed_line`, `react`, `pin`, `sticker` or `random_draw`.
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
  #[serde(default)]
  pub by: Option<String>,
  #[serde(default)]
  pub job: bool,
}

/// What a send came to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SendAnswer {
  pub lines: Vec<String>,
  pub refusal: Option<String>,
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
}

/// What a consult line came to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConsultSaid {
  pub room: i64,
  pub id: i64,
  pub lines: Vec<String>,
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

/// A companion added on the console.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CompanionRequest {
  pub name: String,
  pub label: String,
  pub provider: String,
  pub model: String,
  pub worker: String,
  pub room: String,
}

/// One companion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Companion {
  pub name: String,
  pub label: String,
  /// The room the receiver stood up for it, where it did.
  pub id: Option<i64>,
}

/// A companion and the room it answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CompanionRoom {
  pub id: i64,
  pub taken_back: bool,
  pub said: Option<Value>,
}

/// Every companion as it now stands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Companions {
  pub items: Vec<Companion>,
  pub more: bool,
  pub room: Option<CompanionRoom>,
  pub said: Option<Value>,
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
  /// 0 for the round's own turn, then each question, item and check in the order asked.
  pub number: i64,
  pub question: String,
  /// `turn`, `question`, `item`, `check` or `end`.
  pub kind: String,
  pub answer_chars: i64,
  pub changed: bool,
  /// `answered`, `skip` or `failed` for a turn; `message`, `skip`, `withheld`, `ceiling-message` or
  /// `ceiling-skip` for the end.
  pub outcome: String,
  pub detail: Option<String>,
  pub answer: Option<String>,
  pub ms: i64,
  /// Epoch milliseconds.
  pub at: i64,
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

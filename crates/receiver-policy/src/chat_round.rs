//! What a round of a chat session is handed (docs/bmo-window.md, 「What every turn carries」): every fixed
//! line its prompt is written in.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Header {
  pub chat: String,
  pub chat_titled: String,
  pub mode: String,
  pub kind: String,
  pub privilege: String,
  pub privileged: String,
  pub restricted: String,
  pub owner: String,
  pub tools_on: String,
  pub tools_off: String,
  pub delegation_on: String,
  pub delegation_off: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kinds {
  pub owner: String,
  pub user: String,
  pub group: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Voice {
  pub none: String,
  pub launch: String,
  pub waiting: String,
  pub untitled: String,
  pub titled: String,
  pub failed_kind: String,
  pub switch_intent: String,
  pub switch_kind: String,
  pub switch_ttl_ms: i64,
  pub run_cap_ms: i64,
  pub emotions: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Blocks {
  pub archived: String,
  pub archived_end: String,
  pub waited: String,
  pub waited_end: String,
  pub fresh: String,
  pub fresh_end: String,
  pub nothing: String,
  pub quiet: String,
  pub quiet_end: String,
  pub reference: String,
  pub reference_pushed: String,
  pub reference_end: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reads {
  pub range: String,
  pub one: String,
  pub span: String,
  pub quoted: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Labels {
  pub message: String,
  pub quoted: String,
  pub instructions: String,
  pub reference: String,
  pub summary: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shape {
  pub unknown: String,
  pub owner_tag: String,
  pub album: String,
  pub image_look: String,
  pub image_blind: String,
  pub image_attached_blind: String,
  pub image_attached_gone: String,
  pub image_attached: String,
  pub image_replied_blind: String,
  pub image_replied_gone: String,
  pub image_replied: String,
  pub image_no_reader: String,
  pub image_fetch_failed: String,
  pub file_saved: String,
  pub file_not_stored: String,
  pub quote_limit: usize,
  pub quote_cut: String,
  pub quoted: String,
  pub quoted_empty: String,
  pub quoted_missing: String,
  pub quoted_shown: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Closing {
  pub owed: String,
  /// The language line naming the room's language, `{language}`, where the room speaks one the receiver knows.
  pub language_named: String,
  pub language_own: String,
  pub woken: String,
  pub forced_owed: String,
  pub forced: String,
  pub waiting: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Thread {
  pub setting: String,
  pub days: i64,
  pub lookback: u64,
  pub line_chars: usize,
  pub ceiling_chars: usize,
  pub path: String,
  pub heading: String,
  pub room: String,
  pub line: String,
  pub cut: String,
}

/// What the gate is handed to choose a turn's reference.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
  /// How much of the closing words of the message a turn follows on from, in characters.
  pub answered_chars: usize,
}

/// How the conversations keep time.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Conversation {
  pub queue_ms: u64,
  pub wait_ms: u64,
  pub window_seconds: i64,
}

/// What waits in the agent's queue behind a turn.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueueLine {
  pub nothing: String,
  pub waiting: String,
}

/// The answer one room sent back to another, as a quiet instruction.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relay {
  pub answer: String,
}

/// The paragraph words pushed into a working turn are wrapped in.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Push {
  pub note: String,
  pub messages: String,
  pub instructions: String,
  pub joined: String,
  pub scope: String,
}

/// How often a working turn's picture is published where only its counts changed.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Progress {
  pub gap_ms: u64,
}

/// The instruction pass's clock, its pacing and what a row given back by a restart opens with.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Instructions {
  pub tick_seconds: u64,
  pub watch_ms: u64,
  pub cold_wake_gap_ms: i64,
  pub restart_limit: i64,
  pub restarted: String,
}

/// The words a restart left unread, read back at the start.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Backlog {
  pub max_age_seconds: i64,
  pub settle_ms: u64,
}

/// How often a room being answered is shown typing again.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Typing {
  pub every_ms: u64,
}

/// The owner's words from the console to a room's session, as the quiet instruction they go in as.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Console {
  pub sort: String,
  pub instruction: Vec<String>,
}

/// A line typed into the chat agent's program: the number it is shown under and the lead ahead of its round.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Typed {
  pub number: i64,
  pub nameless: String,
  pub lead: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatRound {
  pub recent_messages: u64,
  pub scope: String,
  pub header: Header,
  pub kinds: Kinds,
  pub voice: Voice,
  pub blocks: Blocks,
  pub reads: Reads,
  pub labels: Labels,
  pub shape: Shape,
  pub closing: Closing,
  pub thread: Thread,
  pub conversation: Conversation,
  pub queue_line: QueueLine,
  pub relay: Relay,
  pub push: Push,
  pub progress: Progress,
  pub typing: Typing,
  pub instructions: Instructions,
  pub backlog: Backlog,
  pub console: Console,
  pub typed: Typed,
  pub reference: Reference,
}

crate::part!(pub CHAT_ROUND: ChatRound = "chat-round.toml");

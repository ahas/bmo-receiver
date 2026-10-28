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
  pub emotions: Vec<String>,
  /// The quiet instruction a notice turn's summary is asked in, `{line}` the line the turn sent.
  pub summary: String,
  /// The quiet instruction a line's full reading is asked in, `{line}` the line.
  pub reading: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Blocks {
  pub archived: String,
  pub archived_end: String,
  pub archived_left_out: String,
  pub waited: String,
  pub waited_end: String,
  pub fresh: String,
  pub fresh_end: String,
  pub nothing: String,
  pub quiet: String,
  pub quiet_end: String,
  pub reference: String,
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
pub struct Shape {
  pub unknown: String,
  pub owner_tag: String,
  pub album: String,
  pub image_look: String,
  pub image_blind: String,
  pub image_attached_blind: String,
  pub image_attached_gone: String,
  pub image_not_stored: String,
  pub image_attached: String,
  pub image_replied_blind: String,
  pub image_replied_gone: String,
  pub image_replied: String,
  pub image_no_reader: String,
  pub image_fetch_failed: String,
  pub voice: String,
  pub voice_empty: String,
  pub voice_failed: String,
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
  /// The language line naming each person the turn answers with their language, `{people}`: one
  /// `language_person` clause a person, `{name}` and `{language}`, joined by `language_joint`.
  pub language: String,
  pub language_person: String,
  pub language_joint: String,
  /// The language line naming the room's language, `{language}`, where the turn answers nobody.
  pub language_room: String,
  /// What the line calls the owner where the page shows no name of theirs.
  pub owner: String,
  pub woken: String,
  pub forced_owed: String,
  pub forced: String,
  pub waiting: String,
  /// The owed, forced and waiting lines of a turn that carries the draft process's first question, where that
  /// question is not its last.
  pub questioned_owed: String,
  pub questioned_forced: String,
  pub questioned_waiting: String,
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
  /// What a round's reads name the thread by (docs/drafting.md, 「The round log」).
  pub holds: String,
  pub heading: String,
  /// The heading of the lines added since the session was last handed the thread.
  pub added: String,
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
  /// What a round's reads name a piece of the turn's core by where neither the index nor the gate names it, by the
  /// gate's word for the piece's sort (docs/drafting.md, 「The round log」).
  pub holds: std::collections::BTreeMap<String, String>,
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

/// How often a working turn is offered the words that came for its room, and how long it is offered none after
/// the agency refused a push.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pushing {
  pub look_ms: u64,
  pub refused_gap_ms: u64,
}

/// How often a working turn's picture is published where only its counts changed.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Progress {
  pub gap_ms: u64,
}

/// The instruction pass's clock, its pacing, what a row given back by a restart opens with, and what a row for a
/// room whose registration is gone opens with in the owner's private chat, `{room}` the room's number.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Instructions {
  pub tick_seconds: u64,
  pub watch_ms: u64,
  pub cold_wake_gap_ms: i64,
  pub restart_limit: i64,
  pub restarted: String,
  pub room_removed: String,
}

/// The words a restart left unread, read back at the start.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Backlog {
  pub settle_ms: u64,
  pub turn_seconds: u64,
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
  pub archived_messages: u64,
  pub scope: String,
  pub header: Header,
  pub kinds: Kinds,
  pub voice: Voice,
  pub blocks: Blocks,
  pub reads: Reads,
  pub shape: Shape,
  pub closing: Closing,
  pub thread: Thread,
  pub conversation: Conversation,
  pub queue_line: QueueLine,
  pub relay: Relay,
  pub push: Push,
  pub pushing: Pushing,
  pub progress: Progress,
  pub typing: Typing,
  pub instructions: Instructions,
  pub backlog: Backlog,
  pub console: Console,
  pub typed: Typed,
  pub reference: Reference,
  pub language: Language,
  pub situation: Situation,
  pub supervisor: Supervisor,
  pub index: Index,
}

/// The room's situation note, updated by one light turn before the supervisor's judgement.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Situation {
  pub holds: String,
  pub ceiling_chars: usize,
  pub head: String,
  pub previous: String,
  pub none: String,
  pub recent: String,
  pub judged: String,
  pub cut: String,
}

/// The agent's own judgement that opens a round its every message reached on the open window alone.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Supervisor {
  pub head: String,
  pub situation: String,
  pub recent: String,
  pub judged: String,
  pub question: String,
  pub name: String,
}

/// The brain's index a turn carries in place of the pieces, and the walk a new session's first turn carries.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Index {
  pub head: String,
  pub group: String,
  pub group_hint: String,
  pub hint: String,
  pub end: String,
  pub system_documents: String,
  pub room_rules: String,
  pub knowledge: String,
  pub people: String,
  pub room_memories: String,
  pub walk_head: String,
  pub walk_room: String,
  pub walk_unknown: String,
}

/// How a person's language in a room is read: the least letters a message's prose must hold to name it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Language {
  pub prose_letters: usize,
}

crate::part!(pub CHAT_ROUND: ChatRound = "chat-round.toml");

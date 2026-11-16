//! What a round of a chat session is handed (docs/bmo-window.md, 「What every turn carries」): every fixed
//! line its prompt is written in.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Header {
  pub line: String,
  pub line_untitled: String,
  pub privileged: String,
  pub restricted: String,
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
  /// The marker a notice turn's summary carries, the line it reads as data, and the rule the marker stands for
  /// in the room's block, `{line}` the line the turn sent.
  pub summary_marker: String,
  pub summary: String,
  pub summary_rule: String,
  /// The marker a line's full reading carries, the line it reads as data, and the rule the marker stands for in
  /// the room's block, `{line}` the line.
  pub reading_marker: String,
  pub reading: String,
  pub reading_rule: String,
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
  pub people: String,
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
  /// The note beside an owner's message that holds the secret marker.
  pub secret_taken: String,
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
  /// The closing rules, said once in the room's block rather than as a reminder on every turn.
  pub rule: String,
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
  pub words: String,
  pub instructions: String,
  pub rooms: String,
  pub waiting: String,
  /// What the agent may do when words wait behind its turn, said once in the room's block; a turn carries no
  /// queue counts.
  pub rule: String,
}

/// The answer one room sent back to another, as a quiet instruction: its marker, its data and the rule the
/// marker stands for in the room's block.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relay {
  pub marker: String,
  pub answer: String,
  pub rule: String,
}

/// A send asked outside its agent's own round, handed to the room's supervisor as an order: who handed it, each
/// act's words, the order around them and what the asker is answered with.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Handed {
  pub sort: String,
  /// The marker the handed words carry, and the rule it stands for in the room's block.
  pub marker: String,
  pub rule: String,
  pub job: String,
  pub agent: String,
  pub nobody: String,
  pub through: String,
  pub said: String,
  pub spoken: String,
  pub spoken_words: String,
  pub consulted: String,
  pub act: String,
  pub reply_to: String,
  pub answer_wanted: String,
  pub via: String,
  pub instruction: String,
  pub answered: String,
}

/// What words pushed into a working turn carry: the marker that names the push, what arrived as data, and the
/// rule the room's block carries for a turn marked with it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Push {
  pub marker: String,
  pub messages: String,
  pub instructions: String,
  pub joined: String,
  pub scope: String,
  pub rule: String,
}

/// The frame a quiet notice is carried in: the markers that name it, and the rules they stand for in the room's
/// block.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Notice {
  pub marker: String,
  pub optional_marker: String,
  pub rule: String,
  pub optional_rule: String,
}

/// How often a working turn is offered the words that came for its room, and how long it is offered none after
/// the agency refused a push.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pushing {
  pub look_ms: u64,
  pub refused_gap_ms: u64,
}

/// How a turn cut from the round that follows it is re-attached to: the wait between two tries while the agency
/// answers nothing, and how long the round keeps trying before it fails.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attach {
  pub pace_ms: u64,
  pub reattach_ms: u64,
}

/// How a round's message is put into its room: how many times its send is tried before the round fails, and the
/// wait between two tries.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Send {
  pub tries: u64,
  pub gap_ms: u64,
}

/// How often a working turn's picture is published where only its counts changed.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Progress {
  pub gap_ms: u64,
}

/// The thinking block the picture of a turn working in the owner's private chat carries: the most characters of
/// the block carried, an ellipsis marking the cut.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Thinking {
  pub ceiling_chars: usize,
}

/// The instruction pass's clock, its pacing, how many failures the agency answered a row goes through before it
/// is let go, what a row given back opens with, and what a row for a room whose registration is gone opens with in
/// the owner's private chat, `{room}` the room's number.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Instructions {
  pub tick_seconds: u64,
  pub watch_ms: u64,
  pub cold_wake_gap_ms: i64,
  pub failure_limit: i64,
  /// The marker a row given back by a restart carries, its line as data, and the rule the marker stands for in
  /// the room's block.
  pub restarted_marker: String,
  pub restarted: String,
  pub restarted_rule: String,
  pub room_removed: String,
  /// The line a quiet instruction opens on: the unit that published its row, and what the row knows of the agent
  /// and room it is for, joined by `parts`.
  pub source: String,
  pub agent: String,
  pub room: String,
  pub parts: String,
  /// The line naming one item a notice is for, and its count where the same notice arrived for it more than once.
  pub subject: String,
  pub subject_times: String,
  /// The line under a notice that arrived unchanged more than once.
  pub repeat: String,
  /// The publisher a row is shown as where it carries none and its kind names none.
  pub unknown_publisher: String,
}

/// The words a restart left unread, read back at the start.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Backlog {
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
  /// The marker the owner's console words carry, their text as data, and the rule the marker stands for in the
  /// room's block.
  pub marker: String,
  pub instruction: Vec<String>,
  pub rule: String,
}

/// A line typed into the default answerer's program: the number it is shown under and the lead ahead of its round.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Typed {
  pub number: i64,
  pub nameless: String,
  /// The marker a typed line's turn carries, the lead as data, and the rule the marker stands for in the room's
  /// block.
  pub marker: String,
  pub lead: String,
  pub rule: String,
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
  pub handed: Handed,
  pub push: Push,
  pub notice: Notice,
  pub pushing: Pushing,
  pub attach: Attach,
  pub send: Send,
  pub progress: Progress,
  pub thinking: Thinking,
  pub typing: Typing,
  pub instructions: Instructions,
  pub backlog: Backlog,
  pub console: Console,
  pub typed: Typed,
  pub reference: Reference,
  pub situation: Situation,
  pub supervisor: Supervisor,
  pub index: Index,
}

/// The room's situation note, updated by one turn before the supervisor's judgement.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Situation {
  pub ceiling_chars: usize,
  /// The most messages between the note's mark and the newest before the update reads the newest this many alone;
  /// 0 reads every message since the mark.
  pub gap_messages: usize,
  /// What the note turn is asked, said once in the room's block rather than in the turn.
  pub instruction: String,
  /// The short mark the note turn itself carries.
  pub marker: String,
  pub previous: String,
  /// The line the message round reads the note under, ahead of the room's words.
  pub front: String,
  pub none: String,
  /// The line the messages that came in since the note last read the room's record stand under.
  pub since: String,
  /// The one-line marker a turn carries in the note's place where the session already read this version of it.
  pub unchanged: String,
  pub cut: String,
  /// A first-person claim in the room's language that a reply, fix, attachment or send happened.
  pub claim: String,
}

/// The agent's own judgement that opens a round its every message reached on the open window alone.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Supervisor {
  /// The marker the judgement turn carries, and the rule it stands for in the room's block, `{question}` its
  /// catalogue text.
  pub marker: String,
  pub rule: String,
  pub structure: String,
  pub head: String,
  pub recent: String,
  pub judged: String,
  pub question: String,
  pub name: String,
}

/// The brain's index a turn carries in place of the pieces, and the walk a new session's first turn carries.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Index {
  /// The brain's standing rule, said in the room's block; a turn carries no index line.
  pub rule: String,
  /// The rule for the memories entry a turn carries, said in the room's block, `{chat}` the room.
  pub memories: String,
  pub head: String,
  pub group: String,
  pub group_hint: String,
  pub hint: String,
  /// The line the block gains where the turn's agent's brain says a consolidation is due.
  pub due: String,
  pub system_documents: String,
  pub room_rules: String,
  pub knowledge: String,
  pub people: String,
  pub room_memories: String,
  pub walk_head: String,
  pub walk_room: String,
  pub walk_untitled: String,
  pub walk_unknown: String,
}

crate::part!(pub CHAT_ROUND: ChatRound = "chat-round.toml");
